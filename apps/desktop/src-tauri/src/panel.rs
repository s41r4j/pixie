//! The notch panel: top-centre of the screen under the cursor, never steals focus.
//! Sounds play only on state changes (open → `agent-launch`, close → `agent-close`).

use crate::{emit, AppState, PANEL};
use openclicky_core::Event;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Manager, PhysicalPosition, Runtime, WebviewWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PanelMode {
    #[default]
    Hidden,
    /// Hotkey held: mic would be live (M2).
    Listening,
    /// Open, turn finished / waiting.
    Idle,
}

#[derive(Debug, Default)]
pub struct PanelState {
    pub mode: PanelMode,
}

fn window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(PANEL)
}

pub fn configure(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    if let Some(w) = window(app) {
        crate::macos::configure_overlay(&w);
    }
}

/// Place the panel top-centre on the monitor under the cursor, just below the menu bar.
fn position(app: &AppHandle, win: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let (Some(m), Ok(size)) = (monitor, win.outer_size()) else { return };
    let area = m.work_area();
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = area.position.y + (8.0 * m.scale_factor()) as i32;
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

pub fn mode(app: &AppHandle) -> PanelMode {
    app.state::<AppState>().panel.lock().map(|p| p.mode).unwrap_or_default()
}

/// Set the mode; shows/hides the window and plays a sound on open/close transitions.
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
        match (prev, next) {
            (PanelMode::Hidden, _) => {
                position(&handle, &win);
                #[cfg(target_os = "macos")]
                crate::macos::order_front_without_focus(&win);
                #[cfg(not(target_os = "macos"))]
                let _ = win.show();
                handle.state::<AppState>().play("agent-launch");
            }
            (_, PanelMode::Hidden) => {
                #[cfg(target_os = "macos")]
                crate::macos::order_out(&win);
                #[cfg(not(target_os = "macos"))]
                let _ = win.hide();
                handle.state::<AppState>().play("agent-close");
            }
            _ => {}
        }
    });
    emit(app, Event::new("panel.state", json!({ "mode": next })));
}

pub fn hide(app: &AppHandle) {
    set_mode(app, PanelMode::Hidden);
}

pub fn toggle(app: &AppHandle) {
    let next = if mode(app) == PanelMode::Hidden { PanelMode::Idle } else { PanelMode::Hidden };
    set_mode(app, next);
}
