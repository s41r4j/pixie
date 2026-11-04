//! Menu-bar icon: left click toggles the panel; the menu has panel, setup and quit.

use crate::{commands, panel, AppState};
use openclicky_core::ResourceKind;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "panel", "Show / Hide Panel (hold ⌃⌥ to talk)", true, None::<&str>)?;
    let setup = MenuItem::with_id(app, "permissions", "Permissions…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit openclicky", true, Some("cmd+q"))?;
    let menu = Menu::with_items(app, &[&toggle, &setup, &PredefinedMenuItem::separator(app)?, &quit])?;

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
            "panel" => panel::toggle(app),
            "permissions" => commands::open_permissions_window(app),
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
                panel::toggle(tray.app_handle());
            }
        });
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}
