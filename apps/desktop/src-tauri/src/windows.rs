//! Normal windows: the note (a job's full page) and Settings. Both may take focus: the user
//! asked for them.

use crate::{emit, CANVAS, NOTE, SETTINGS};
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

/// The Excalidraw canvas window (experimental). It comes up where they are (the current Space,
/// over a full-screen app), in front and focused, and goes away when they click outside Pixie.
pub fn open_canvas(app: &AppHandle, id: &str) {
    let handle = app.clone();
    let id = id.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(CANVAS) {
            emit(&handle, Event::new("canvas.select", json!({ "id": id })));
            let _ = w.show();
            let _ = w.unminimize();
            crate::macos::present(&w);
            return;
        }
        let url = WebviewUrl::App(format!("index.html#canvas={id}").into());
        let builder = WebviewWindowBuilder::new(&handle, CANVAS, url)
            .title("Pixie · Canvas")
            .inner_size(1100.0, 760.0)
            .min_inner_size(560.0, 420.0)
            .visible(true)
            .focused(true);
        #[cfg(target_os = "macos")]
        let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
        match builder.build() {
            Ok(w) => crate::macos::present(&w),
            Err(e) => log::error!("canvas window: {e}"),
        }
    });
}

pub fn canvas_visible(app: &AppHandle) -> bool {
    app.get_webview_window(CANVAS).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

pub fn hide_canvas(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(CANVAS) {
            let _ = w.hide();
            close_canvas_later(&handle);
        }
    });
}

/// A canvas hidden for a couple of minutes is closed for real (it's saved; Excalidraw holds a
/// lot of memory). Opening it again loads it back.
fn close_canvas_later(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if let Some(w) = h.get_webview_window(CANVAS) {
                if !w.is_visible().unwrap_or(true) {
                    let _ = w.close();
                }
            }
        });
    });
}

/// The canvas lost focus: if they clicked outside Pixie (another app, the desktop), it goes
/// away. Focus moving to the notch or a Pixie sheet keeps it.
pub fn canvas_blurred(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let focused = h.get_webview_window(CANVAS).and_then(|w| w.is_focused().ok()).unwrap_or(false);
            if !focused && !crate::macos::app_active_in(&h) {
                if let Some(w) = h.get_webview_window(CANVAS) {
                    let _ = w.hide();
                    close_canvas_later(&h);
                }
            }
        });
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
            return;
        }
        // Made when opened and closed for real when they close it: a hidden Settings would keep
        // its own web process (~40 MB) around all day.
        let query: String = tab.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        let url = WebviewUrl::App(if query.is_empty() { "index.html".into() } else { format!("index.html?tab={query}").into() });
        let builder = WebviewWindowBuilder::new(&handle, SETTINGS, url)
            .title("Pixie")
            .inner_size(860.0, 640.0)
            .min_inner_size(720.0, 520.0)
            .resizable(true)
            .center()
            .visible(true)
            .focused(true);
        #[cfg(target_os = "macos")]
        let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
        match builder.build() {
            Ok(w) => {
                dock(&handle, true);
                let _ = w.set_focus();
            }
            Err(e) => log::error!("settings window: {e}"),
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
