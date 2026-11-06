//! Pointing and drawing on the user's screen (`highlight`, `draw`). Diagrams and explanations go
//! on the Excalidraw canvas instead (`canvas.rs`).
//!
//! Screen coordinates arrive in CG global points. The overlay covers the screen they fall on;
//! the UI converts using the window's *actual* frame. Targets snap to the nearest real control
//! or text line from the last capture.

use crate::macos::{self, Rect};
use crate::{emit, screen, AppState, OVERLAY};
use openclicky_core::Event;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

static GEN: AtomicU64 = AtomicU64::new(0);
static SHOWN: AtomicBool = AtomicBool::new(false);

/// Marks or a highlight are on screen.
pub fn is_shown() -> bool {
    SHOWN.load(Ordering::SeqCst)
}
/// Upper bound for marks that outlive the answer (speech end clears sooner).
const LINGER: Duration = Duration::from_secs(20);

pub fn configure(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        macos::configure_overlay(&w, macos::OVERLAY_LEVEL);
        macos::set_ignores_mouse(&w, true);
    }
}

/// Put the overlay on the screen containing (x, y), show it, and report its real frame.
fn show_at(app: &AppHandle, x: f64, y: f64) {
    let h = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = h.get_webview_window(OVERLAY) {
            let s = macos::screen_containing(x, y);
            macos::set_frame(&w, s.frame);
            macos::order_front_without_focus(&w);
            macos::set_ignores_mouse(&w, true);
            let actual = macos::frame(&w).unwrap_or(s.frame);
            let _ = tx.send((s, actual));
        }
    });
    if let Ok((s, actual)) = rx.recv_timeout(Duration::from_millis(600)) {
        let notch = crate::panel::screen(app);
        SHOWN.store(true, Ordering::SeqCst);
        emit(app, Event::new("overlay.screen", json!({ "screen": s, "window": actual, "origin": notch.frame })));
        emit(app, Event::new("overlay.state", json!({ "active": true })));
    }
    schedule_clear(app, LINGER);
}

/// Fade the marks out after `after` unless something new is drawn first.
pub fn schedule_clear(app: &AppHandle, after: Duration) {
    if !SHOWN.load(Ordering::SeqCst) {
        return;
    }
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(after).await;
        if GEN.load(Ordering::SeqCst) == gen {
            clear(&h);
        }
    });
}

pub fn clear(app: &AppHandle) {
    GEN.fetch_add(1, Ordering::SeqCst);
    if !SHOWN.swap(false, Ordering::SeqCst) {
        return;
    }
    emit(app, Event::new("overlay.clear", json!({})));
    emit(app, Event::new("overlay.state", json!({ "active": false })));
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(450)).await;
        // The buddy keeps the overlay up; only the marks go.
        if SHOWN.load(Ordering::SeqCst) || crate::buddy::holds_overlay(&h) {
            return;
        }
        let h2 = h.clone();
        let _ = h.run_on_main_thread(move || {
            if let Some(w) = h2.get_webview_window(OVERLAY) {
                macos::order_out(&w);
            }
        });
    });
}

fn num(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Where to point: a named target (id or on-screen text) wins; then coordinates snapped to the
/// nearest real item; then, if the coordinates hit nothing, the label as a target. Returns None
/// when nothing on screen backs the request (so the model can try again instead of pointing at
/// empty space).
fn locate(app: &AppHandle, input: &Value, label: &str) -> Result<Rect, String> {
    let cap = app.state::<AppState>().last_capture.lock().unwrap().clone();
    let has_xy = input.get("x").and_then(Value::as_f64).is_some() && input.get("y").and_then(Value::as_f64).is_some();
    let raw = Rect { x: num(input, "x"), y: num(input, "y"), w: num(input, "w").max(8.0), h: num(input, "h").max(8.0) };
    let near = has_xy.then_some((raw.x + raw.w / 2.0, raw.y + raw.h / 2.0));
    let as_rect = |f: screen::Frame| Rect { x: f.x, y: f.y, w: f.w, h: f.h };
    if let Some(c) = &cap {
        if let Some(t) = input.get("target").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            if let Some(f) = screen::find(c, t, near) {
                return Ok(as_rect(f));
            }
        }
        if has_xy {
            if let Some(f) = screen::snap(c, raw.x, raw.y, raw.w, raw.h, label) {
                return Ok(as_rect(f));
            }
        }
        if let Some(f) = screen::find(c, label, near) {
            return Ok(as_rect(f));
        }
    }
    if has_xy && (raw.x != 0.0 || raw.y != 0.0) {
        // Nothing listed there (an image, a canvas): trust the coordinates.
        return Ok(raw);
    }
    Err("I couldn't find that on screen. Give target (an element id like e12 or the exact text from the lists) or the x, y, w, h from the lists.".into())
}

pub fn highlight(app: &AppHandle, input: &Value) -> Result<String, String> {
    let label = input.get("label").and_then(Value::as_str).unwrap_or("");
    let r = locate(app, input, label)?;
    show_at(app, r.x + r.w / 2.0, r.y + r.h / 2.0);
    emit(app, Event::new("overlay.highlight", json!({ "rect": r, "label": label })));
    app.state::<AppState>().play("target-ping");
    Ok(format!("Pointing at {} ({}, {}, {}×{}).", if label.is_empty() { "it" } else { label }, r.x, r.y, r.w, r.h))
}

pub fn draw(app: &AppHandle, input: &Value) -> Result<String, String> {
    let mut ops = input.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
    if ops.is_empty() {
        return Err("draw needs at least one op.".into());
    }
    // Marks about a thing on screen (circle/box/underline) go exactly on that thing.
    for op in ops.iter_mut() {
        let kind = op.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
        if matches!(kind.as_str(), "circle" | "box" | "underline") && (op.get("target").is_some() || op.get("w").is_some()) {
            let s = locate(app, op, "")?;
            op["x"] = json!(s.x);
            op["y"] = json!(s.y);
            op["w"] = json!(s.w);
            op["h"] = json!(s.h);
        } else if op.get("x").is_none() {
            return Err(format!("{kind} needs x and y."));
        }
    }
    let first = ops[0].clone();
    show_at(app, num(&first, "x") + num(&first, "w") / 2.0, num(&first, "y") + num(&first, "h") / 2.0);
    emit(app, Event::new("overlay.draw", json!({ "ops": ops })));
    let sound = match first.get("kind").and_then(Value::as_str).unwrap_or("") {
        "underline" | "box" => "highlighter",
        "check" => "stamp",
        _ => "pen-flick",
    };
    app.state::<AppState>().play(sound);
    Ok(format!("Drew {} mark(s).", ops.len()))
}

