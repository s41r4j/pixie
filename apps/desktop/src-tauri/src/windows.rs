//! Normal windows: the note (a job's full page) and Settings. Both may take focus: the user
//! asked for them.

use crate::{emit, NOTE, SETTINGS};
use openclicky_core::Event;
use serde_json::json;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_note(app: &AppHandle, job_id: &str) {
    let handle = app.clone();
    let id = job_id.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(NOTE) {
            emit(&handle, Event::new("note.select", json!({ "id": id })));
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            dock(&handle, true);
            return;
        }
        let url = WebviewUrl::App(format!("index.html#job={id}").into());
        let builder = WebviewWindowBuilder::new(&handle, NOTE, url)
            .title("Pixie")
            .inner_size(760.0, 880.0)
            .min_inner_size(520.0, 480.0)
            .visible(true)
            .focused(true);
        #[cfg(target_os = "macos")]
        let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
        match builder.build() {
            Ok(w) => {
                let _ = w.set_focus();
                dock(&handle, true);
            }
            Err(e) => log::error!("note window: {e}"),
        }
    });
}

pub fn open_settings(app: &AppHandle, tab: &str) {
    let handle = app.clone();
    let tab = tab.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(SETTINGS) {
            if !tab.is_empty() {
                emit(&handle, Event::new("settings.tab", json!({ "tab": tab })));
            }
            dock(&handle, true);
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    });
}

/// Pixie lives in the menu bar, but while Settings or a note is open it also gets a Dock icon
/// (and ⌘-Tab), like any window you opened on purpose. Main thread.
pub fn dock(app: &AppHandle, show: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(if show { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, show);
}

/// Drop the Dock icon once no normal window is showing. Main thread.
pub fn dock_sync(app: &AppHandle, closing: &str) {
    let open = [SETTINGS, NOTE]
        .iter()
        .filter(|l| **l != closing)
        .any(|l| app.get_webview_window(l).and_then(|w| w.is_visible().ok()).unwrap_or(false));
    if !open {
        dock(app, false);
    }
}
