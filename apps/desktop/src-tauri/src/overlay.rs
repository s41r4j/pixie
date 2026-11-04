//! Full-screen, click-through overlay for pointing (`highlight`) and drawing (`draw`).
//! Coordinates arrive in CG global points; the overlay covers the screen they fall on and
//! the UI converts them to window-local positions.

use crate::macos::{self, Rect};
use crate::{emit, AppState, OVERLAY};
use openclicky_core::Event;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

static GEN: AtomicU64 = AtomicU64::new(0);
/// Marks fade on their own; the window goes away a little after the last one.
const LINGER: Duration = Duration::from_secs(9);

pub fn configure(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        macos::configure_overlay(&w, macos::OVERLAY_LEVEL);
        macos::set_ignores_mouse(&w, true);
    }
}

/// Put the overlay on the screen containing (x, y) and show it.
fn show_at(app: &AppHandle, x: f64, y: f64) {
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let h = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = h.get_webview_window(OVERLAY) {
            let s = macos::screen_containing(x, y);
            macos::set_frame(&w, s.frame);
            macos::order_front_without_focus(&w);
            macos::set_ignores_mouse(&w, true);
            let _ = tx.send(s);
        }
    });
    if let Ok(s) = rx.recv_timeout(Duration::from_millis(500)) {
        let notch = crate::panel::screen(app);
        emit(app, Event::new("overlay.screen", json!({ "screen": s, "origin": notch.frame })));
    }
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(LINGER).await;
        if GEN.load(Ordering::SeqCst) == gen {
            hide(&h);
        }
    });
}

fn hide(app: &AppHandle) {
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = h.get_webview_window(OVERLAY) {
            macos::order_out(&w);
        }
    });
}

pub fn clear(app: &AppHandle) {
    GEN.fetch_add(1, Ordering::SeqCst);
    emit(app, Event::new("overlay.clear", json!({})));
    hide(app);
}

fn num(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn highlight(app: &AppHandle, input: &Value) -> Result<String, String> {
    let r = Rect { x: num(input, "x"), y: num(input, "y"), w: num(input, "w").max(8.0), h: num(input, "h").max(8.0) };
    if r.x == 0.0 && r.y == 0.0 {
        return Err("highlight needs the item's x, y, w and h in screen points.".into());
    }
    show_at(app, r.x + r.w / 2.0, r.y + r.h / 2.0);
    let label = input.get("label").and_then(Value::as_str).unwrap_or("");
    emit(app, Event::new("overlay.highlight", json!({ "rect": r, "label": label })));
    app.state::<AppState>().play("target-ping");
    Ok(format!("Pointing at {} ({}, {}).", if label.is_empty() { "it" } else { label }, r.x, r.y))
}

pub fn draw(app: &AppHandle, input: &Value) -> Result<String, String> {
    let ops = input.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
    if ops.is_empty() {
        return Err("draw needs at least one op.".into());
    }
    let first = &ops[0];
    show_at(app, num(first, "x") + num(first, "w") / 2.0, num(first, "y") + num(first, "h") / 2.0);
    emit(app, Event::new("overlay.draw", json!({ "ops": ops })));
    let sound = match first.get("kind").and_then(Value::as_str).unwrap_or("") {
        "underline" | "box" => "highlighter",
        "check" => "stamp",
        "text" => "pen-flick",
        _ => "pen-flick",
    };
    app.state::<AppState>().play(sound);
    Ok(format!("Drew {} mark(s).", ops.len()))
}
