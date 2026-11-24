//! Pointing and drawing on the user's screen (`highlight`, `draw`). Diagrams and explanations go
//! on the Excalidraw canvas instead (`canvas.rs`).
//!
//! Screen coordinates arrive in CG global points. The overlay covers the screen they fall on;
//! the UI converts using the window's *actual* frame. Targets snap to the nearest real control
//! or text line from the last capture.

use pixie_core::sync::LockOk;
use crate::macos::{self, Rect};
use crate::{emit, screen, AppState, OVERLAY};
use pixie_core::Event;
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

/// The ways numbers from the model can be read: as screen points (what the lists and tool
/// results give), and, when it saw the screenshot, as that image's pixels (scaled to points).
fn readings(cap: Option<&screen::Capture>, raw: Rect) -> Vec<Rect> {
    let mut v = vec![raw];
    if let Some(c) = cap.filter(|_| crate::narrate::saw_image()) {
        let (k, ox, oy) = c.pixel_scale();
        if (k - 1.0).abs() > 0.01 || ox != 0.0 || oy != 0.0 {
            v.push(Rect { x: ox + raw.x * k, y: oy + raw.y * k, w: raw.w * k, h: raw.h * k });
        }
    }
    v
}

/// Is the middle of `r` on the screen that was captured?
fn on_display(cap: Option<&screen::Capture>, r: &Rect) -> bool {
    let Some(d) = cap.map(|c| &c.display).filter(|d| d.w > 0.0) else { return true };
    let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    x >= d.x && x <= d.x + d.w && y >= d.y && y <= d.y + d.h
}

/// Where to point: a named target (id or on-screen text) wins; then coordinates snapped to the
/// nearest real item, read as points or as screenshot pixels, whichever lands on something
/// (models copy points from the lists but read positions off the image in pixels); then the
/// label as a target. Err when nothing on screen backs the request (so the model can try again
/// instead of pointing at empty space).
fn locate(app: &AppHandle, input: &Value, label: &str) -> Result<Rect, String> {
    let cap = app.state::<AppState>().last_capture.lock_ok().clone();
    let label = if label.is_empty() { input.get("label").and_then(Value::as_str).unwrap_or("") } else { label };
    let has_xy = input.get("x").and_then(Value::as_f64).is_some() && input.get("y").and_then(Value::as_f64).is_some();
    let raw = Rect { x: num(input, "x"), y: num(input, "y"), w: num(input, "w").max(8.0), h: num(input, "h").max(8.0) };
    let reads = if has_xy { readings(cap.as_ref(), raw) } else { vec![] };
    let as_rect = |f: screen::Frame| Rect { x: f.x, y: f.y, w: f.w, h: f.h };
    if let Some(c) = &cap {
        let near = reads.first().map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0));
        if let Some(t) = input.get("target").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
            if let Some(f) = screen::find(c, t, near) {
                return Ok(as_rect(f));
            }
        }
        let best = reads
            .iter()
            .filter(|r| on_display(cap.as_ref(), r))
            .filter_map(|r| screen::snap_scored(c, r.x, r.y, r.w, r.h, label))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, f)) = best {
            return Ok(as_rect(f));
        }
        if let Some(f) = screen::find(c, label, near) {
            return Ok(as_rect(f));
        }
    }
    if has_xy && (raw.x != 0.0 || raw.y != 0.0) {
        // Nothing listed there (an image, a canvas): trust the coordinates, as points unless
        // only the pixel reading is on the screen.
        let pick = reads.iter().find(|r| on_display(cap.as_ref(), r)).copied().unwrap_or(raw);
        return Ok(pick);
    }
    Err("I couldn't find that on screen. Give target (an element id like e12 or the exact text from the lists) or the x, y, w, h from the lists.".into())
}

/// Where a target (id, on-screen text or x, y, w, h) is on screen.
pub fn rect_of(app: &AppHandle, input: &Value) -> Result<Rect, String> {
    locate(app, input, "")
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

