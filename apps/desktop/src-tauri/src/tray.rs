//! Menu-bar icon (the `:)` mark): left click opens the panel to type; right click for the menu.

use crate::{commands, panel, AppState};
use openclicky_core::ResourceKind;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

static UPDATE_ITEM: std::sync::OnceLock<MenuItem<tauri::Wry>> = std::sync::OnceLock::new();
/// "Restart to Update" once a new version is waiting.
pub fn update_item(_app: &AppHandle) {
    if let Some(item) = UPDATE_ITEM.get() {
        let text = match crate::updates::ready_version() {
            Some(v) => format!("Restart to Update to {v}"),
            None => "Check for Updates…".to_string(),
        };
        let _ = item.set_text(text);
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let hint_text = if cfg!(target_os = "macos") { "Hold ⌃⌥ to talk · fn⌃ to dictate" } else { "Hold Ctrl+Alt to talk · Ctrl+Win to dictate" };
    let hint = MenuItem::with_id(app, "hint", hint_text, false, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", "Check for Updates…", true, None::<&str>)?;
    let _ = UPDATE_ITEM.set(update.clone());
    let ask = MenuItem::with_id(app, "ask", "Type a Question…", true, None::<&str>)?;
    let jobs = MenuItem::with_id(app, "jobs", "Show Jobs", true, None::<&str>)?;
    let canvas = MenuItem::with_id(app, "canvas", "Show Canvas", true, None::<&str>)?;
    let setup = MenuItem::with_id(app, "settings", "Settings…", true, Some("cmd+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit Pixie", true, Some("cmd+q"))?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&hint, &sep, &ask, &jobs, &canvas, &sep3, &setup, &update, &sep2, &quit])?;

    let icon = app
        .state::<AppState>()
        .store
        .resolve(ResourceKind::Icon, "tray-template")
        .and_then(|p| Image::from_path(p).map_err(|e| log::warn!("tray icon: {e}")).ok())
        .or_else(|| app.default_window_icon().cloned());

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Pixie")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "ask" => panel::set_mode(app, panel::PanelMode::Typing),
            "jobs" => {
                if let Some(j) = app.state::<AppState>().jobs.list().first() {
                    crate::windows::open_note(app, &j.id);
                } else {
                    crate::jobs::toast(app, "No jobs yet. Ask for something bigger and I'll make one.", "info", None);
                }
            }
            "canvas" => {
                // The one in use, else the last one, else a blank one.
                let _ = crate::canvas::canvas_open(app.clone(), None, None);
            }
            "settings" => commands::open_permissions_window(app),
            "update" => {
                if crate::updates::ready_version().is_some() {
                    let _ = crate::updates::restart_now(app);
                } else {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let s = crate::updates::check(&app).await.unwrap_or_default();
                        let msg = match s["phase"].as_str().unwrap_or("") {
                            "up_to_date" => format!("Pixie {} is the newest version.", s["current"].as_str().unwrap_or("")),
                            "error" => s["error"].as_str().unwrap_or("Couldn't check for updates.").to_string(),
                            _ => return,
                        };
                        crate::jobs::toast(&app, &msg, "info", None);
                    });
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if panel::mode(app) == panel::PanelMode::Hidden {
                    panel::set_mode(app, panel::PanelMode::Typing);
                } else {
                    crate::session::escape(app);
                }
            }
        });
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}
