//! Menu-bar icon (the `:)` mark): left click opens the panel to type; right click for the menu.

use crate::{commands, panel, AppState};
use openclicky_core::ResourceKind;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let hint = MenuItem::with_id(app, "hint", "Hold ⌃⌥ to talk · fn⌃ to dictate", false, None::<&str>)?;
    let ask = MenuItem::with_id(app, "ask", "Type a Question…", true, None::<&str>)?;
    let jobs = MenuItem::with_id(app, "jobs", "Show Jobs", true, None::<&str>)?;
    let setup = MenuItem::with_id(app, "settings", "Settings…", true, Some("cmd+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit openclicky", true, Some("cmd+q"))?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&hint, &sep, &ask, &jobs, &setup, &sep2, &quit])?;

    let icon = app
        .state::<AppState>()
        .store
        .resolve(ResourceKind::Icon, "tray-template")
        .and_then(|p| Image::from_path(p).map_err(|e| log::warn!("tray icon: {e}")).ok())
        .or_else(|| app.default_window_icon().cloned());

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("openclicky")
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
            "settings" => commands::open_permissions_window(app),
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
