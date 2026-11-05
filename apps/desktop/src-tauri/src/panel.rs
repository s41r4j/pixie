//! The notch panel. A transparent window pinned to the top edge of the screen under the
//! pointer, over the menu bar; the UI draws a black shape that grows out of the notch.
//! Clicks pass through everywhere except the drawn shape (see `hit.rs`). Never steals focus,
//! except in `Typing` mode where the user asked to type. On a Mac with a notch the window rests
//! there invisibly; hovering the notch opens `Peek` (quick actions, last answer, jobs).

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
    /// Hovering the resting notch: quick actions, last answer, jobs.
    Peek,
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

fn same_screen(a: &macos::ScreenInfo, b: &macos::ScreenInfo) -> bool {
    a.frame == b.frame
}

/// Put the window at the top centre of `s`, over the menu bar / notch.
fn place(handle: &AppHandle, win: &WebviewWindow, s: macos::ScreenInfo) {
    let frame = Rect { x: s.frame.x + (s.frame.w - WIDTH) / 2.0, y: s.frame.y, w: WIDTH, h: HEIGHT };
    macos::set_frame(win, frame);
    handle.state::<AppState>().panel.lock().unwrap().screen = s;
    hit::set_frame(handle, PANEL, frame);
    emit(handle, Event::new("panel.screen", json!({ "screen": s })));
}

/// The screen with a camera notch, where the panel rests.
fn notch_screen() -> Option<macos::ScreenInfo> {
    macos::screens().into_iter().find(|s| s.notch_width.is_some())
}

/// Modes that come and go without a sound.
fn quiet(m: PanelMode) -> bool {
    matches!(m, PanelMode::Hidden | PanelMode::Peek | PanelMode::Toast)
}

/// At launch: rest invisibly over the notch so hovering it opens the peek.
pub fn rest(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let (Some(win), Some(ns)) = (window(&handle), notch_screen()) else { return };
        place(&handle, &win, ns);
        macos::order_front_without_focus(&win);
        hit::set_visible(&handle, PANEL, true);
    });
}

/// Set the mode; places/shows the window and plays a sound on real open/close.
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
        let current = st.panel.lock().unwrap().screen;
        if next == PanelMode::Hidden {
            st.session.set_hovered(false);
            if !quiet(prev) {
                st.play("agent-close");
            }
            // Let the collapse animation play, then rest over the notch (or leave the screen).
            let h2 = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(340));
                let h3 = h2.clone();
                let _ = h2.run_on_main_thread(move || {
                    if mode(&h3) != PanelMode::Hidden {
                        return;
                    }
                    let Some(w) = window(&h3) else { return };
                    match notch_screen() {
                        Some(ns) => {
                            let cur = h3.state::<AppState>().panel.lock().unwrap().screen;
                            if !same_screen(&cur, &ns) {
                                place(&h3, &w, ns);
                            }
                            hit::set_visible(&h3, PANEL, true);
                        }
                        None => {
                            hit::set_visible(&h3, PANEL, false);
                            macos::order_out(&w);
                        }
                    }
                });
            });
        } else {
            if prev == PanelMode::Hidden {
                // Peek stays where it rests; everything else opens on the screen you're using.
                let target = if next == PanelMode::Peek { current } else { macos::screen_at_cursor() };
                if !same_screen(&current, &target) || current.frame.w == 0.0 {
                    place(&handle, &win, target);
                }
                macos::order_front_without_focus(&win);
                hit::set_visible(&handle, PANEL, true);
            }
            if quiet(prev) && !quiet(next) {
                st.play("agent-launch");
            }
        }
        if next == PanelMode::Typing {
            macos::make_key(&win);
            let _ = win.set_focus();
        } else if prev == PanelMode::Typing {
            macos::deactivate();
        }
    });
    emit(app, Event::new("panel.state", json!({ "mode": next, "prev": prev })));
}

pub fn hide(app: &AppHandle) {
    set_mode(app, PanelMode::Hidden);
}
