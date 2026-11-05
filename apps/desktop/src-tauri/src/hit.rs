//! Click-through for transparent windows: each window reports the rectangles it actually
//! draws (window-local points); the poll loop turns mouse events on only while the pointer
//! is over one of them.

use crate::macos::{self, Rect};
use crate::AppState;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[derive(Debug, Default, Clone)]
pub struct Entry {
    pub frame: Rect,
    pub rects: Vec<Rect>,
    pub visible: bool,
    /// Current `ignoresMouseEvents` value.
    pub ignoring: bool,
}

#[derive(Default)]
pub struct Hits {
    pub map: std::sync::Mutex<HashMap<String, Entry>>,
}

fn with<F: FnOnce(&mut Entry)>(app: &AppHandle, label: &str, f: F) {
    let st = app.state::<AppState>();
    let mut m = st.hits.map.lock().unwrap();
    let e = m.entry(label.to_string()).or_insert_with(|| Entry { ignoring: true, ..Default::default() });
    f(e);
}

pub fn set_frame(app: &AppHandle, label: &str, frame: Rect) {
    with(app, label, |e| e.frame = frame);
}

pub fn set_rects(app: &AppHandle, label: &str, rects: Vec<Rect>) {
    with(app, label, |e| e.rects = rects);
}

pub fn set_visible(app: &AppHandle, label: &str, visible: bool) {
    with(app, label, |e| e.visible = visible);
}

#[derive(Default)]
struct Hover {
    inside_since: Option<Instant>,
    outside_since: Option<Instant>,
    was_inside: bool,
}

fn hover_state() -> &'static std::sync::Mutex<Hover> {
    static H: std::sync::OnceLock<std::sync::Mutex<Hover>> = std::sync::OnceLock::new();
    H.get_or_init(Default::default)
}

/// Native hover for the notch (WebKit doesn't track hover in an inactive app):
/// rest over the notch → peek after 90 ms; leave the peek → close after 350 ms.
fn notch_hover(app: &AppHandle, cx: f64, cy: f64) {
    use crate::panel::{self, PanelMode};
    let st = app.state::<AppState>();
    let mode = panel::mode(app);
    let (frame, rects) = {
        let m = st.hits.map.lock().unwrap();
        match m.get(crate::PANEL) {
            Some(e) if e.visible => (e.frame, e.rects.clone()),
            _ => return,
        }
    };
    let screen = panel::screen(app);
    let inside = if mode == PanelMode::Hidden {
        // The camera housing itself, a little wider, full menu-bar height.
        let w = screen.notch_width.unwrap_or(0.0);
        if w <= 0.0 {
            false
        } else {
            let zone = Rect { x: (frame.w - w) / 2.0 - 10.0, y: 0.0, w: w + 20.0, h: screen.top_inset + 2.0 };
            zone.contains(cx - frame.x, cy - frame.y)
        }
    } else {
        // While the card springs open its reported rect lags; count its final footprint too.
        let w = (screen.notch_width.unwrap_or(0.0) + 300.0).max(500.0);
        let zone = Rect { x: (frame.w - w) / 2.0, y: 0.0, w, h: screen.top_inset + 190.0 };
        let in_zone = mode == PanelMode::Peek && zone.contains(cx - frame.x, cy - frame.y);
        in_zone || rects.iter().any(|r| r.contains(cx - frame.x, cy - frame.y))
    };
    let now = Instant::now();
    let mut h = hover_state().lock().unwrap();
    if inside {
        h.outside_since = None;
        h.inside_since.get_or_insert(now);
    } else {
        h.inside_since = None;
        h.outside_since.get_or_insert(now);
    }
    if inside != h.was_inside {
        h.was_inside = inside;
        st.session.set_hovered(inside);
    }
    let in_for = h.inside_since.map(|t| now - t).unwrap_or_default();
    let out_for = h.outside_since.map(|t| now - t).unwrap_or_default();
    drop(h);
    match mode {
        PanelMode::Hidden if inside && in_for >= Duration::from_millis(90) => panel::set_mode(app, PanelMode::Peek),
        PanelMode::Peek if !inside && out_for >= Duration::from_millis(350) => panel::set_mode(app, PanelMode::Hidden),
        _ => {}
    }
}

/// Called from the poll thread (~50 Hz).
pub fn tick(app: &AppHandle) {
    let (cx, cy) = macos::cursor();
    notch_hover(app, cx, cy);
    crate::buddy::tick(app, cx, cy);
    let changes: Vec<(String, bool)> = {
        let st = app.state::<AppState>();
        let mut m = st.hits.map.lock().unwrap();
        m.iter_mut()
            .filter_map(|(label, e)| {
                let inside = e.visible && e.rects.iter().any(|r| r.contains(cx - e.frame.x, cy - e.frame.y));
                let want_ignore = !inside;
                (want_ignore != e.ignoring).then(|| {
                    e.ignoring = want_ignore;
                    (label.clone(), want_ignore)
                })
            })
            .collect()
    };
    for (label, ignore) in changes {
        let h = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = h.get_webview_window(&label) {
                macos::set_ignores_mouse(&w, ignore);
            }
        });
    }
}
