//! The buddy: Pixie's own pointer that rides next to your cursor (on the overlay window),
//! flies to things it points at, and, when allowed, clicks for you.

use crate::macos::{self, Rect};
use crate::{emit, AppState, OVERLAY};
use pixie_core::Event;
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
/// A hover tip from one of Pixie's own windows is showing.
static TIP: AtomicBool = AtomicBool::new(false);

fn follow() -> &'static Mutex<Follow> {
    static F: std::sync::OnceLock<Mutex<Follow>> = std::sync::OnceLock::new();
    F.get_or_init(|| Mutex::new(Follow { last: (-1.0, -1.0), at: Instant::now(), screen: Rect::default() }))
}

/// When Pixie last did something (idle mode keeps the buddy up a moment after).
fn last_busy() -> &'static Mutex<Option<Instant>> {
    static L: std::sync::OnceLock<Mutex<Option<Instant>>> = std::sync::OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}

/// Should the buddy be on screen right now?
pub fn enabled(app: &AppHandle) -> bool {
    let st = app.state::<AppState>();
    // Asked ~50 times a second: no copies of the settings.
    let mode = st.settings.lock().map(|s| if !s.buddy || s.buddy_mode == "off" { 0 } else if s.buddy_mode == "idle" { 1 } else { 2 }).unwrap_or(2);
    match mode {
        0 => false,
        1 => {
            let busy = st.session.active() || crate::overlay::is_shown() || crate::jobs::live_running(app) || TIP.load(Ordering::SeqCst);
            let mut l = last_busy().lock().unwrap();
            if busy {
                *l = Some(Instant::now());
            }
            l.is_some_and(|t| t.elapsed() < Duration::from_millis(1500))
        }
        _ => true,
    }
}

/// Is the buddy switched off entirely (Settings → General → Buddy: Off)?
pub fn off(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.lock().map(|s| !s.buddy || s.buddy_mode == "off").unwrap_or(false)
}

/// Show (or clear, with "") a hover tip beside the buddy. Ignored when the buddy is off.
pub fn tip(app: &AppHandle, text: &str) {
    if off(app) {
        return;
    }
    let text: String = text.trim().chars().take(140).collect();
    TIP.store(!text.is_empty(), Ordering::SeqCst);
    if !text.is_empty() {
        touch();
        // Make sure the buddy is up on this screen even if the cursor is resting.
        let (cx, cy) = crate::macos::cursor();
        tick(app, cx, cy);
    }
    emit(app, Event::new("buddy.tip", json!({ "text": text })));
}

/// Mark the buddy busy now (it's about to point, draw or click).
pub fn touch() {
    *last_busy().lock().unwrap() = Some(Instant::now());
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

/// Where a click goes, and what Pixie believes is there.
struct Aim {
    x: f64,
    y: f64,
    label: String,
    /// The app that owns the control (to give focus back after asking).
    pid: Option<i64>,
}

/// Resolve a click target: an element id (e12, to the live control, scrolled into view first),
/// explicit x/y, "x,y", or a label the screen or the helper can find. Never moves a point to a
/// control that isn't under it.
async fn resolve(app: &AppHandle, input: &Value) -> Result<Aim, String> {
    let target = input.get("target").and_then(Value::as_str).unwrap_or("").trim().to_string();
    // An id wins over coordinates given with it: it names the exact control.
    if let Some(hit) = crate::screen::element(&target) {
        let t = hit?;
        let (x, y, label, pid) = live_target(app, &target, &t).await?;
        return Ok(Aim { x, y, label, pid });
    }
    if let (Some(x), Some(y)) = (num(input, "x"), num(input, "y")) {
        return Ok(Aim { x, y, label: target, pid: None });
    }
    if let Some((x, y)) = parse_point(&target) {
        return Ok(Aim { x, y, label: String::new(), pid: None });
    }
    if target.is_empty() {
        return Err("Say where to click: an element id like e12 (best), a label, or x and y.".into());
    }
    let st = app.state::<AppState>();
    let seen = st.last_capture.lock().unwrap().clone();
    if let Some(f) = seen.as_ref().and_then(|c| crate::screen::find(c, &target, None)) {
        return Ok(Aim { x: f.x + f.w / 2.0, y: f.y + f.h / 2.0, label: target, pid: None });
    }
    let v = st.helper.call("mac_step", json!({ "action": "find", "target": target }), Duration::from_secs(5)).await?;
    let f = &v["frame"];
    let (x, y, w, h) = (f["x"].as_f64().unwrap_or(0.0), f["y"].as_f64().unwrap_or(0.0), f["w"].as_f64().unwrap_or(0.0), f["h"].as_f64().unwrap_or(0.0));
    Ok(Aim { x: x + w / 2.0, y: y + h / 2.0, label: v["label"].as_str().unwrap_or(&target).to_string(), pid: None })
}

/// A control id → the point to act on, from the control as it is right now (Mac): its live frame,
/// scrolled into view first when it's out of its list, and refused when something else covers
/// it or it has changed. Elsewhere the latest snapshot's frame, refused when out of view.
pub async fn live_target(app: &AppHandle, id: &str, t: &crate::screen::Target) -> Result<(f64, f64, String, Option<i64>), String> {
    let center = |f: &Value| (f["x"].as_f64().unwrap_or(0.0) + f["w"].as_f64().unwrap_or(0.0) / 2.0, f["y"].as_f64().unwrap_or(0.0) + f["h"].as_f64().unwrap_or(0.0) / 2.0);
    if !cfg!(target_os = "macos") || t.gen.is_none() {
        if t.hidden {
            return Err(format!("{id} \"{}\" is out of view (scrolled out of its list or off the screen). Scroll to it first, then use the id again.", t.label));
        }
        return Ok((t.frame.x + t.frame.w / 2.0, t.frame.y + t.frame.h / 2.0, t.label.clone(), None));
    }
    let st = app.state::<AppState>();
    let args = json!({ "gen": t.gen, "id": t.raw });
    let gone = |e: String| if e.starts_with("gone") { format!("{id} isn't in a recent view any more. Call look and use an id from it.") } else { e };
    let mut v = st.helper.call("element", args.clone(), Duration::from_secs(4)).await.map_err(gone)?;
    if v["exists"] == json!(false) {
        return Err(format!("{id} \"{}\" isn't on screen any more (the window changed). Call look and use an id from it.", t.label));
    }
    if v["hidden"] == json!(true) {
        v = st.helper.call("scroll_to", args, Duration::from_secs(4)).await.map_err(gone)?;
        if v["hidden"] == json!(true) || v["exists"] == json!(false) {
            return Err(format!("{id} \"{}\" is out of view and couldn't be scrolled to. Scroll its list (mac_step scroll with x y over the list), then look again.", t.label));
        }
        log::info!("click: scrolled {id} \"{}\" into view", t.label);
    }
    if v["under"] == json!(false) {
        return Err(format!("Something else is on top of {id} \"{}\" ({}), so I didn't click. A menu, sheet or popup may be open: look again.", t.label, v["covered_by"].as_str().unwrap_or("another window")));
    }
    let live = v["label"].as_str().unwrap_or("").trim().to_string();
    if !live.is_empty() && !t.label.trim().is_empty() && live != t.label.trim() {
        return Err(format!("{id} now says \"{live}\", not \"{}\", so I didn't click. Look again before choosing.", t.label));
    }
    let (x, y) = center(&v["frame"]);
    Ok((x, y, if live.is_empty() { t.label.clone() } else { live }, v["pid"].as_i64()))
}

/// What's really under a point (Mac): the control a click there presses, as "kind \"label\"", its
/// label and the labels to check for risk (the element and its parents up to the first one that
/// takes a press), and its app.
pub struct Under {
    pub text: String,
    pub labels: Vec<String>,
    pub app: String,
    pub pid: Option<i64>,
}

pub async fn under(app: &AppHandle, x: f64, y: f64) -> Option<Under> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let v = app.state::<AppState>().helper.call("hit_test", json!({ "x": x, "y": y }), Duration::from_secs(3)).await.ok()?;
    let chain = v["chain"].as_array()?;
    if chain.is_empty() {
        return None;
    }
    let mut labels = Vec::new();
    let mut text = String::new();
    for e in chain {
        let label = e["label"].as_str().unwrap_or("").trim().to_string();
        if !label.is_empty() {
            labels.push(label.clone());
        }
        let pressable = e["pressable"].as_bool().unwrap_or(false);
        if text.is_empty() && (pressable || !label.is_empty()) {
            let role = e["role"].as_str().unwrap_or("").trim_start_matches("AX").to_lowercase();
            text = format!("{role} \"{}\"", label.chars().take(60).collect::<String>());
        }
        if pressable {
            break;
        }
    }
    Some(Under { text, labels, app: v["app"].as_str().unwrap_or("").to_string(), pid: v["pid"].as_i64() })
}

/// The buddy flies to (x, y); resolves when it has landed.
/// (Callers in idle mode get the buddy back first: see `touch`.)
async fn fly(app: &AppHandle, x: f64, y: f64, label: &str, click: bool) {
    emit(app, Event::new("buddy.fly", json!({ "x": x, "y": y, "label": label, "click": click })));
    app.state::<AppState>().play("whoosh");
    tokio::time::sleep(Duration::from_millis(720)).await;
}

/// mac_step click / double_click / right_click / drag, performed by the buddy.
pub async fn act(app: &AppHandle, action: &str, input: &Value) -> Result<String, String> {
    touch();
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
        st.helper.call("drag", json!({ "x": x, "y": y, "to_x": tx, "to_y": ty }), Duration::from_secs(8)).await?;
        let now = crate::screen::after_action(app, Some((tx, ty)), crate::screen::settle_for("drag")).await;
        return Ok(format!("Dragged from ({x:.0}, {y:.0}) to ({tx:.0}, {ty:.0}).\n{now}"));
    }
    let aim = resolve(app, input).await?;
    let (x, y) = (aim.x, aim.y);
    // What a click there really lands on, and whether it needs their OK.
    let st_app = st.last_capture.lock().unwrap().as_ref().map(|c| c.app.clone()).unwrap_or_default();
    let hit = under(app, x, y).await;
    let labels: Vec<String> = match &hit {
        Some(h) if !h.labels.is_empty() => h.labels.clone(),
        _ => vec![aim.label.clone()],
    };
    let owner = hit.as_ref().map(|h| h.app.clone()).filter(|a| !a.is_empty()).unwrap_or(st_app);
    if let Some(l) = labels.iter().find(|l| crate::safety::risky_label(l)) {
        let what = crate::safety::press_text(l, &owner);
        if !crate::safety::ask(app, &what, hit.as_ref().and_then(|h| h.pid).or(aim.pid)).await {
            return Err(crate::safety::declined(&what));
        }
    }
    let label = aim.label.clone();
    fly(app, x, y, &label, true).await;
    let (button, count) = match action {
        "right_click" => ("right", 1),
        "double_click" => ("left", 2),
        _ => ("left", 1),
    };
    emit(app, Event::new("buddy.click", json!({ "x": x, "y": y })));
    st.play("gel-pop");
    st.helper.call("click_at", json!({ "x": x, "y": y, "button": button, "count": count }), Duration::from_secs(8)).await?;
    let now = crate::screen::after_action(app, Some((x, y)), crate::screen::settle_for(action)).await;
    // Say what it actually landed on, so a wrong target is noticed at once.
    let landed = match &hit {
        Some(h) if !h.text.is_empty() => format!("Clicked at ({x:.0}, {y:.0}), on {}.", h.text),
        _ if !label.is_empty() => format!("Clicked \"{label}\" at ({x:.0}, {y:.0})."),
        _ => format!("Clicked at ({x:.0}, {y:.0})."),
    };
    Ok(format!("{landed}\n{now}"))
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
