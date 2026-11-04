//! The notch panel. A transparent window pinned to the top edge of the screen under the
//! pointer, over the menu bar; the UI draws a black shape that grows out of the notch.
//! Clicks pass through everywhere except the drawn shape (see `hit.rs`). Never steals focus,
//! except in `Typing` mode where the user asked to type.

use crate::macos::{self, Rect};
use crate::{emit, hit, AppState, PANEL};
use openclicky_core::Event;
use serde::Serialize;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// Window size (points). The visible shape is drawn inside it.
pub const WIDTH: f64 = 720.0;
pub const HEIGHT: f64 = 460.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PanelMode {
    #[default]
    Hidden,
    /// Hold-to-talk: mic live, transcript streaming.
    Listening,
    /// fn+⌃ dictation.
    Dictating,
    /// Waiting for the first words of the answer.
    Thinking,
    /// The answer (spoken + written).
    Answer,
    /// Opened with a tap: a hint and a text box.
    Idle,
    /// Text box focused.
    Typing,
    /// A one-line notice (job finished, typed it…).
    Toast,
}

#[derive(Debug, Default)]
pub struct PanelState {
    pub mode: PanelMode,
    pub screen: macos::ScreenInfo,
}

fn window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(PANEL)
}

pub fn configure(app: &AppHandle) {
    if let Some(w) = window(app) {
        macos::configure_overlay(&w, macos::PANEL_LEVEL);
        macos::set_ignores_mouse(&w, true);
    }
}

pub fn mode(app: &AppHandle) -> PanelMode {
    app.state::<AppState>().panel.lock().map(|p| p.mode).unwrap_or_default()
}

pub fn screen(app: &AppHandle) -> macos::ScreenInfo {
    app.state::<AppState>().panel.lock().map(|p| p.screen).unwrap_or_default()
}

/// Set the mode; shows/hides the window and plays a sound on open/close.
pub fn set_mode(app: &AppHandle, next: PanelMode) {
    let state = app.state::<AppState>();
    let prev = {
        let Ok(mut p) = state.panel.lock() else { return };
        std::mem::replace(&mut p.mode, next)
    };
    if prev == next {
        return;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(win) = window(&handle) else { return };
        let st = handle.state::<AppState>();
        if prev == PanelMode::Hidden {
            let s = macos::screen_at_cursor();
            let frame = Rect { x: s.frame.x + (s.frame.w - WIDTH) / 2.0, y: s.frame.y, w: WIDTH, h: HEIGHT };
            macos::set_frame(&win, frame);
            st.panel.lock().unwrap().screen = s;
            hit::set_frame(&handle, PANEL, frame);
            macos::order_front_without_focus(&win);
            hit::set_visible(&handle, PANEL, true);
            emit(&handle, Event::new("panel.screen", json!({ "screen": s })));
            if next != PanelMode::Toast {
                st.play("agent-launch");
            }
        }
        if next == PanelMode::Typing {
            macos::make_key(&win);
            let _ = win.set_focus();
        } else if prev == PanelMode::Typing {
            macos::deactivate();
        }
        if next == PanelMode::Hidden {
            hit::set_visible(&handle, PANEL, false);
            if !matches!(prev, PanelMode::Toast) {
                st.play("agent-close");
            }
            // Let the collapse animation play, then take the window off screen.
            let h2 = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(320));
                let h3 = h2.clone();
                let _ = h2.run_on_main_thread(move || {
                    if mode(&h3) == PanelMode::Hidden {
                        if let Some(w) = window(&h3) {
                            macos::order_out(&w);
                        }
                    }
                });
            });
        }
    });
    emit(app, Event::new("panel.state", json!({ "mode": next, "prev": prev })));
}

pub fn hide(app: &AppHandle) {
    set_mode(app, PanelMode::Hidden);
}
