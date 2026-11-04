//! openclicky desktop shell (M1): tray agent, notch panel, hold-to-talk hotkey,
//! permissions wizard, resource slots and the sound bus.

mod commands;
mod hotkey;
#[cfg(target_os = "macos")]
mod macos;
mod panel;
mod permissions;
mod resources;
mod tray;

use openclicky_core::{Event, ResourceStore};
use openclicky_voice::{AudioBus, RodioOutput};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

pub const PANEL: &str = "panel";
pub const PERMISSIONS: &str = "permissions";

pub struct AppState {
    pub store: Arc<ResourceStore>,
    pub pack: resources::PackSource,
    pub theme: serde_json::Value,
    pub audio: Mutex<AudioBus<RodioOutput>>,
    pub panel: Mutex<panel::PanelState>,
}

impl AppState {
    /// Play a sound slot; never fails loudly (sound is decoration, not function).
    pub fn play(&self, slot: &str) {
        if let Ok(mut bus) = self.audio.lock() {
            bus.play(slot);
        }
    }
}

/// Emit a core [`Event`] to every window.
pub fn emit(app: &tauri::AppHandle, event: Event) {
    if let Err(e) = app.emit(openclicky_core::event::EVENT_CHANNEL, &event) {
        log::warn!("emit {} failed: {e}", event.kind);
    }
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = tauri::Builder::default()
        .register_uri_scheme_protocol("slot", resources::slot_protocol)
        .invoke_handler(tauri::generate_handler![
            commands::panel_mode,
            commands::hide_panel,
            commands::play_sound,
            commands::stop_bed,
            commands::show_permissions,
            commands::resources_report,
            commands::theme,
            permissions::permissions_status,
            permissions::permission_request,
            permissions::permission_open_settings,
        ])
        .setup(|app| {
            // Menu-bar agent: no Dock icon (LSUIElement is also set in Info.plist for bundles).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let (store, pack) = resources::load_store(app.handle());
            let theme = resources::load_theme(store.root());
            let store = Arc::new(store);
            let audio = AudioBus::new(store.clone(), RodioOutput::spawn());
            app.manage(AppState {
                store,
                pack,
                theme,
                audio: Mutex::new(audio),
                panel: Mutex::new(panel::PanelState::default()),
            });

            panel::configure(app.handle());
            tray::create(app.handle())?;
            hotkey::spawn(app.handle().clone());

            if !permissions::required_granted() {
                commands::open_permissions_window(app.handle());
            }
            // Dev/smoke-test aid: `openclicky --show-panel` opens the panel at launch.
            if std::env::args().any(|a| a == "--show-panel") {
                panel::set_mode(app.handle(), panel::PanelMode::Idle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Windows hide instead of closing; the app lives in the menu bar.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == PANEL {
                    panel::hide(window.app_handle());
                } else {
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building openclicky");

    app.run(|_app, event| {
        // Closing the last window must not quit a menu-bar app; only Quit (exit code) does.
        if let RunEvent::ExitRequested { code: None, api, .. } = event {
            api.prevent_exit();
        }
    });
}
