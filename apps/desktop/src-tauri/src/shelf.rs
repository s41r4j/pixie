//! Edge job cards + the floating pill (BUILD-SPEC §1): a transparent, click-through window on
//! the right edge of the main screen. Shown while a job runs, needs them, or just finished.

use crate::macos;
use crate::{emit, hit, AppState, SHELF};
use openclicky_core::Event;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

static SHOWN: AtomicBool = AtomicBool::new(false);

pub fn configure(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SHELF) {
        macos::configure_overlay(&w, macos::NS_STATUS_WINDOW_LEVEL);
        macos::set_ignores_mouse(&w, true);
    }
}

/// Push the job list to the notch (its Agents tab). The edge cards are retired: jobs live in
/// the notch now, so this window stays hidden and never takes screen space.
pub fn refresh(app: &AppHandle) {
    let st = app.state::<AppState>();
    let jobs = st.jobs.list();
    emit(app, Event::new("shelf.jobs", json!({ "jobs": jobs, "visible": false })));
    if SHOWN.swap(false, Ordering::SeqCst) {
        let h = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = h.get_webview_window(SHELF) {
                hit::set_visible(&h, SHELF, false);
                macos::order_out(&w);
            }
        });
    }
}
