//! Edge job cards + the floating pill (BUILD-SPEC §1): a transparent, click-through window on
//! the right edge of the main screen. Shown while a job runs, needs them, or just finished.

use crate::macos::{self, Rect};
use crate::{emit, hit, AppState, SHELF};
use openclicky_core::model::JobState;
use openclicky_core::Event;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

pub const WIDTH: f64 = 340.0;
static SHOWN: AtomicBool = AtomicBool::new(false);

pub fn configure(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SHELF) {
        macos::configure_overlay(&w, macos::NS_STATUS_WINDOW_LEVEL);
        macos::set_ignores_mouse(&w, true);
    }
}

/// Re-evaluate visibility and push the job list to the shelf.
pub fn refresh(app: &AppHandle) {
    let st = app.state::<AppState>();
    let jobs = st.jobs.list();
    let visible = jobs.iter().any(|j| {
        matches!(j.state, JobState::Running | JobState::WaitingOnUser | JobState::Offered) || j.unread
    });
    emit(app, Event::new("shelf.jobs", json!({ "jobs": jobs, "visible": visible })));
    let was = SHOWN.swap(visible, Ordering::SeqCst);
    if was == visible {
        return;
    }
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(w) = h.get_webview_window(SHELF) else { return };
        if visible {
            let s = macos::screens().first().copied().unwrap_or_default();
            let frame = Rect {
                x: s.frame.x + s.frame.w - WIDTH - 6.0,
                y: s.frame.y + s.top_inset + 6.0,
                w: WIDTH,
                h: (s.frame.h * 0.8).min(760.0),
            };
            macos::set_frame(&w, frame);
            hit::set_frame(&h, SHELF, frame);
            macos::order_front_without_focus(&w);
            hit::set_visible(&h, SHELF, true);
        } else {
            hit::set_visible(&h, SHELF, false);
            macos::order_out(&w);
        }
    });
}
