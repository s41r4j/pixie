//! Pointing and drawing on the user's screen (`highlight`, `draw`), and the **board**: a paper
//! canvas for explaining things visually (`draw` with `board: true`), so the notch text never
//! covers a diagram.
//!
//! Screen coordinates arrive in CG global points. The overlay covers the screen they fall on;
//! the UI converts using the window's *actual* frame. Targets snap to the nearest real control
//! or text line from the last capture.

use crate::macos::{self, Rect};
use crate::{emit, screen, AppState, BOARD, OVERLAY};
use openclicky_core::Event;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};

static GEN: AtomicU64 = AtomicU64::new(0);
static SHOWN: AtomicBool = AtomicBool::new(false);
static BOARD_OPEN: AtomicBool = AtomicBool::new(false);
/// Upper bound for marks that outlive the answer (speech end clears sooner).
const LINGER: Duration = Duration::from_secs(20);

pub fn configure(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        macos::configure_overlay(&w, macos::OVERLAY_LEVEL);
        macos::set_ignores_mouse(&w, true);
    }
    if let Some(w) = app.get_webview_window(BOARD) {
        macos::configure_overlay(&w, macos::OVERLAY_LEVEL);
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
    emit(app, Event::new("overlay.state", json!({ "active": BOARD_OPEN.load(Ordering::SeqCst) })));
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

fn snapped(app: &AppHandle, r: Rect, label: &str) -> Rect {
    let cap = app.state::<AppState>().last_capture.lock().unwrap().clone();
    match cap.and_then(|c| screen::snap(&c, r.x, r.y, r.w, r.h, label)) {
        Some(f) => Rect { x: f.x, y: f.y, w: f.w, h: f.h },
        None => r,
    }
}

pub fn highlight(app: &AppHandle, input: &Value) -> Result<String, String> {
    let label = input.get("label").and_then(Value::as_str).unwrap_or("");
    let raw = Rect { x: num(input, "x"), y: num(input, "y"), w: num(input, "w").max(8.0), h: num(input, "h").max(8.0) };
    if raw.x == 0.0 && raw.y == 0.0 {
        return Err("highlight needs the item's x, y, w and h in screen points.".into());
    }
    let r = snapped(app, raw, label);
    show_at(app, r.x + r.w / 2.0, r.y + r.h / 2.0);
    emit(app, Event::new("overlay.highlight", json!({ "rect": r, "label": label })));
    app.state::<AppState>().play("target-ping");
    Ok(format!("Pointing at {} ({}, {}, {}×{}).", if label.is_empty() { "it" } else { label }, r.x, r.y, r.w, r.h))
}

pub fn draw(app: &AppHandle, input: &Value) -> Result<String, String> {
    if input.get("board").and_then(Value::as_bool).unwrap_or(false) {
        return board(app, input);
    }
    let mut ops = input.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
    if ops.is_empty() {
        return Err("draw needs at least one op.".into());
    }
    // Snap marks that are about a thing on screen (not arrows/free text) to that thing.
    for op in ops.iter_mut() {
        let kind = op.get("kind").and_then(Value::as_str).unwrap_or("");
        if matches!(kind, "circle" | "box" | "underline") && op.get("w").is_some() {
            let r = Rect { x: num(op, "x"), y: num(op, "y"), w: num(op, "w"), h: num(op, "h").max(8.0) };
            let s = snapped(app, r, op.get("text").and_then(Value::as_str).unwrap_or(""));
            op["x"] = json!(s.x);
            op["y"] = json!(s.y);
            op["w"] = json!(s.w);
            op["h"] = json!(s.h);
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

/// Board size in points; ops use a 1000 × 640 canvas.
const BOARD_W: f64 = 960.0;
const BOARD_H: f64 = 680.0;

fn board(app: &AppHandle, input: &Value) -> Result<String, String> {
    let ops = input.get("ops").and_then(Value::as_array).cloned().unwrap_or_default();
    if ops.is_empty() {
        return Err("draw needs at least one op.".into());
    }
    let title = input.get("title").and_then(Value::as_str).unwrap_or("").to_string();
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(w) = h.get_webview_window(BOARD) else { return };
        let s = macos::screen_at_cursor();
        let width = BOARD_W.min(s.frame.w - 80.0);
        let height = BOARD_H.min(s.frame.h - s.top_inset - 120.0);
        // Centre, nudged down so the notch above stays readable.
        let frame = Rect { x: s.frame.x + (s.frame.w - width) / 2.0, y: s.frame.y + s.top_inset + 90.0_f64.max((s.frame.h - height) / 2.0 - 10.0), w: width, h: height };
        macos::set_frame(&w, frame);
        macos::order_front_without_focus(&w);
    });
    BOARD_OPEN.store(true, Ordering::SeqCst);
    emit(app, Event::new("board.show", json!({ "title": title, "ops": ops })));
    emit(app, Event::new("overlay.state", json!({ "active": true })));
    let st = app.state::<AppState>();
    st.play("pencil-paper-bed");
    let secs = (ops.len() as u64 * 450).clamp(900, 6000);
    let h2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(secs)).await;
        if let Ok(mut bus) = h2.state::<AppState>().audio.lock() {
            bus.stop_bed();
        }
    });
    Ok(format!("Drew {} things on the board. It stays open until they close it or ask something else.", ops.len()))
}

pub fn board_close(app: &AppHandle) {
    if !BOARD_OPEN.swap(false, Ordering::SeqCst) {
        return;
    }
    emit(app, Event::new("board.hide", json!({})));
    emit(app, Event::new("overlay.state", json!({ "active": SHOWN.load(Ordering::SeqCst) })));
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(260)).await;
        let h2 = h.clone();
        let _ = h.run_on_main_thread(move || {
            if let Some(w) = h2.get_webview_window(BOARD) {
                macos::order_out(&w);
            }
        });
    });
}
