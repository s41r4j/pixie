//! Tauri commands for the panel and resources. Permission commands live in `permissions.rs`.

use crate::panel::{self, PanelMode};
use crate::{AppState, PERMISSIONS};
use openclicky_core::resources::{MissingFile, ResourceKind};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn panel_mode(app: AppHandle) -> PanelMode {
    panel::mode(&app)
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    panel::hide(&app);
}

/// Play a UI sound by slot. Only slots in the manifest can play.
#[tauri::command]
pub fn play_sound(state: State<'_, AppState>, slot: String) -> Result<(), String> {
    if state.store.entry(ResourceKind::Sound, &slot).is_none() {
        return Err(format!("unknown sound slot `{slot}`"));
    }
    state.play(&slot);
    Ok(())
}

#[tauri::command]
pub fn stop_bed(state: State<'_, AppState>) {
    if let Ok(mut bus) = state.audio.lock() {
        bus.stop_bed();
    }
}

#[tauri::command]
pub fn show_permissions(app: AppHandle) {
    open_permissions_window(&app);
}

#[derive(Serialize)]
pub struct ResourcesReport {
    root: String,
    sounds: usize,
    missing: Vec<MissingFile>,
}

#[tauri::command]
pub fn resources_report(state: State<'_, AppState>) -> ResourcesReport {
    ResourcesReport {
        root: state.store.root().display().to_string(),
        sounds: state.store.manifest().sounds.len(),
        missing: state.store.missing(),
    }
}

/// The wizard is a normal window: the user asked for it, so it may take focus.
pub fn open_permissions_window(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(PERMISSIONS) {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    });
}
