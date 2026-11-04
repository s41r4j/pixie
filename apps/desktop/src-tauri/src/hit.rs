//! Click-through for transparent windows: each window reports the rectangles it actually
//! draws (window-local points); the poll loop turns mouse events on only while the pointer
//! is over one of them.

use crate::macos::{self, Rect};
use crate::AppState;
use std::collections::HashMap;
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

/// Called from the poll thread (~50 Hz).
pub fn tick(app: &AppHandle) {
    let (cx, cy) = macos::cursor();
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
