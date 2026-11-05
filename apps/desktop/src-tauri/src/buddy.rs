//! The buddy: Pixie's own pointer that rides next to your cursor (on the overlay window),
//! flies to things it points at, and — when allowed — clicks for you.

use crate::macos::{self, Rect};
use crate::{emit, AppState, OVERLAY};
use openclicky_core::Event;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

struct Follow {
    last: (f64, f64),
    at: Instant,
    screen: Rect,
}

static ON_SCREEN: AtomicBool = AtomicBool::new(false);

fn follow() -> &'static Mutex<Follow> {
    static F: std::sync::OnceLock<Mutex<Follow>> = std::sync::OnceLock::new();
    F.get_or_init(|| Mutex::new(Follow { last: (-1.0, -1.0), at: Instant::now(), screen: Rect::default() }))
}

pub fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.lock().map(|s| s.buddy).unwrap_or(true)
}

/// Keep the overlay on the cursor's screen and stream the cursor to it (~40 Hz, on change).
pub fn tick(app: &AppHandle, cx: f64, cy: f64) {
    if !enabled(app) {
        if ON_SCREEN.swap(false, Ordering::SeqCst) {
            emit(app, Event::new("buddy.hide", json!({})));
        }
        return;
    }
    let mut f = follow().lock().unwrap();
    let moved = (f.last.0 - cx).abs() + (f.last.1 - cy).abs() > 0.5;
    if !moved && ON_SCREEN.load(Ordering::SeqCst) {
        return;
    }
    if f.at.elapsed() < Duration::from_millis(22) && ON_SCREEN.load(Ordering::SeqCst) {
        return;
    }
    f.last = (cx, cy);
    f.at = Instant::now();
    let changed_screen = !f.screen.contains(cx, cy) || !ON_SCREEN.load(Ordering::SeqCst);
    if changed_screen {
        let h = app.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = h.get_webview_window(OVERLAY) {
                let s = macos::screen_containing(cx, cy);
                macos::set_frame(&w, s.frame);
                macos::order_front_without_focus(&w);
                macos::set_ignores_mouse(&w, true);
                let actual = macos::frame(&w).unwrap_or(s.frame);
                let _ = tx.send((s, actual));
            }
        });
        drop(f);
        if let Ok((s, actual)) = rx.recv_timeout(Duration::from_millis(300)) {
            follow().lock().unwrap().screen = s.frame;
            ON_SCREEN.store(true, Ordering::SeqCst);
            let notch = crate::panel::screen(app);
            emit(app, Event::new("overlay.screen", json!({ "screen": s, "window": actual, "origin": notch.frame })));
        }
    }
    emit(app, Event::new("buddy.cursor", json!({ "x": cx, "y": cy })));
}

/// Is the overlay window currently kept on screen by the buddy?
pub fn holds_overlay(app: &AppHandle) -> bool {
    enabled(app) && ON_SCREEN.load(Ordering::SeqCst)
}

fn num(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}

/// "x,y" or "x y".
fn parse_point(s: &str) -> Option<(f64, f64)> {
    let mut it = s.split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()).map(str::parse::<f64>);
    match (it.next(), it.next(), it.next()) {
        (Some(Ok(x)), Some(Ok(y)), None) => Some((x, y)),
        _ => None,
    }
}

/// Resolve a click target: explicit x/y, "x,y", or a label / element id the helper can find.
async fn resolve(app: &AppHandle, input: &Value) -> Result<(f64, f64, String), String> {
    if let (Some(x), Some(y)) = (num(input, "x"), num(input, "y")) {
        let label = input.get("target").and_then(Value::as_str).unwrap_or("").to_string();
        return Ok((x, y, label));
    }
    let target = input.get("target").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if let Some((x, y)) = parse_point(&target) {
        return Ok((x, y, String::new()));
    }
    if target.is_empty() {
        return Err("Say where to click: a label, an element id like e12, or x and y.".into());
    }
    let st = app.state::<AppState>();
    let v = st.helper.call("mac_step", json!({ "action": "find", "target": target }), Duration::from_secs(5)).await?;
    let f = &v["frame"];
    let (x, y, w, h) = (f["x"].as_f64().unwrap_or(0.0), f["y"].as_f64().unwrap_or(0.0), f["w"].as_f64().unwrap_or(0.0), f["h"].as_f64().unwrap_or(0.0));
    Ok((x + w / 2.0, y + h / 2.0, v["label"].as_str().unwrap_or(&target).to_string()))
}

/// The buddy flies to (x, y); resolves when it has landed.
async fn fly(app: &AppHandle, x: f64, y: f64, label: &str, click: bool) {
    emit(app, Event::new("buddy.fly", json!({ "x": x, "y": y, "label": label, "click": click })));
    app.state::<AppState>().play("whoosh");
    tokio::time::sleep(Duration::from_millis(720)).await;
}

/// mac_step click / double_click / right_click / drag, performed by the buddy.
pub async fn act(app: &AppHandle, action: &str, input: &Value) -> Result<String, String> {
    let st = app.state::<AppState>();
    if !st.settings.lock().map(|s| s.can_click).unwrap_or(true) {
        return Err("Clicking for them is switched off (Settings → General → Pointer only). Point with highlight and tell them what to click.".into());
    }
    if action == "drag" {
        let (Some(x), Some(y), Some(tx), Some(ty)) = (num(input, "x"), num(input, "y"), num(input, "to_x"), num(input, "to_y")) else {
            return Err("drag needs x, y, to_x and to_y in screen points.".into());
        };
        fly(app, x, y, "", false).await;
        emit(app, Event::new("buddy.fly", json!({ "x": tx, "y": ty, "label": "", "click": false, "slow": true })));
        let r = st.helper.call("drag", json!({ "x": x, "y": y, "to_x": tx, "to_y": ty }), Duration::from_secs(8)).await?;
        return Ok(r.to_string());
    }
    let (x, y, label) = resolve(app, input).await?;
    // Snap to the real control under the target when we have the screen.
    let (x, y) = {
        let cap = st.last_capture.lock().unwrap().clone();
        match cap.and_then(|c| crate::screen::snap(&c, x - 4.0, y - 4.0, 8.0, 8.0, &label)) {
            Some(f) if f.w < 600.0 => (f.x + f.w / 2.0, f.y + f.h / 2.0),
            _ => (x, y),
        }
    };
    fly(app, x, y, &label, true).await;
    let (button, count) = match action {
        "right_click" => ("right", 1),
        "double_click" => ("left", 2),
        _ => ("left", 1),
    };
    emit(app, Event::new("buddy.click", json!({ "x": x, "y": y })));
    st.play("gel-pop");
    let r = st.helper.call("click_at", json!({ "x": x, "y": y, "button": button, "count": count }), Duration::from_secs(8)).await?;
    let after = &r["after"];
    Ok(format!(
        "Clicked {} at ({:.0}, {:.0}). Now: {} — \"{}\"; focused: {}. Controls now: {}. Check this is what you expected before the next step.",
        if label.is_empty() { "there".to_string() } else { format!("\"{label}\"") },
        x,
        y,
        after["app"].as_str().unwrap_or(""),
        after["window"].as_str().unwrap_or(""),
        after["focused"].as_str().unwrap_or("nothing"),
        after["controls"].as_array().map(|a| a.iter().filter_map(Value::as_str).take(15).collect::<Vec<_>>().join("; ")).unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points() {
        assert_eq!(parse_point("120, 44"), Some((120.0, 44.0)));
        assert_eq!(parse_point("120 44"), Some((120.0, 44.0)));
        assert_eq!(parse_point("Save"), None);
        assert_eq!(parse_point("1,2,3"), None);
    }
}
