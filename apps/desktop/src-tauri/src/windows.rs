//! Normal windows: the note (a job's full page) and Settings. Both may take focus: the user
//! asked for them.

use crate::{emit, NOTE, SETTINGS};
use openclicky_core::Event;
use serde_json::json;
use tauri::{AppHandle, Manager, TitleBarStyle, WebviewUrl, WebviewWindowBuilder};

pub fn open_note(app: &AppHandle, job_id: &str) {
    let handle = app.clone();
    let id = job_id.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(NOTE) {
            emit(&handle, Event::new("note.select", json!({ "id": id })));
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            return;
        }
        let url = WebviewUrl::App(format!("index.html#job={id}").into());
        match WebviewWindowBuilder::new(&handle, NOTE, url)
            .title("openclicky")
            .inner_size(760.0, 880.0)
            .min_inner_size(520.0, 480.0)
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .visible(true)
            .focused(true)
            .build()
        {
            Ok(w) => {
                let _ = w.set_focus();
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
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    });
}
