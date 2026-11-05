//! openclicky desktop shell: tray agent, notch panel, hold-to-talk voice loop, screen
//! pointing/drawing, background jobs with notes, settings and onboarding.

mod browser;
mod commands;
mod config;
mod connectors;
mod helper;
mod hit;
mod host;
mod hotkey;
mod jobs;
mod kokoro;
#[cfg(target_os = "macos")]
mod macos;
mod overlay;
mod panel;
mod permissions;
mod resources;
mod screen;
mod session;
mod shelf;
mod skills;
mod speech;
mod tray;
mod windows;

use openclicky_core::prompt::Skill;
use openclicky_core::{Event, ResourceStore};
use openclicky_llm::Provider;
use openclicky_voice::{AudioBus, RodioOutput};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

pub const PANEL: &str = "panel";
pub const OVERLAY: &str = "overlay";
pub const SHELF: &str = "shelf";
pub const SETTINGS: &str = "settings";
pub const NOTE: &str = "note";

pub struct AppState {
    pub store: Arc<ResourceStore>,
    pub pack: resources::PackSource,
    pub theme: serde_json::Value,
    pub audio: Mutex<AudioBus<RodioOutput>>,
    pub panel: Mutex<panel::PanelState>,
    pub helper: Arc<helper::Helper>,
    pub data_dir: PathBuf,
    pub settings: Mutex<config::Settings>,
    provider: Mutex<Option<Arc<dyn Provider>>>,
    pub session: session::Session,
    pub jobs: jobs::Jobs,
    pub hits: hit::Hits,
    pub skills: Vec<Skill>,
    pub os_version: String,
    pub full_name: String,
    quiet: Mutex<Option<Instant>>,
}

impl AppState {
    /// Play a sound slot; never fails loudly (sound is decoration, not function).
    pub fn play(&self, slot: &str) {
        if let Ok(mut bus) = self.audio.lock() {
            bus.play(slot);
        }
    }

    /// Same, but at most once every 1.5 s (pen sounds while a note fills).
    pub fn play_quiet(&self, slot: &str) {
        let mut last = self.quiet.lock().unwrap();
        if last.map(|t| t.elapsed().as_millis() < 1500).unwrap_or(false) {
            return;
        }
        *last = Some(Instant::now());
        drop(last);
        self.play(slot);
    }

    pub fn provider(&self) -> Option<Arc<dyn Provider>> {
        self.provider.lock().unwrap().clone()
    }

    pub fn reload_provider(&self) {
        let s = self.settings.lock().unwrap().clone();
        let p = config::provider_config(&self.data_dir, &s).map(|c| c.build());
        match &p {
            Some(p) => log::info!("model: {}", p.label()),
            None => log::warn!("model: no key configured (DEEPSEEK / ANTHROPIC_API_KEY)"),
        }
        *self.provider.lock().unwrap() = p;
    }

    pub fn connected_apps(&self) -> Vec<String> {
        connectors::connected_names()
    }
}

/// Emit a core [`Event`] to every window.
pub fn emit(app: &tauri::AppHandle, event: Event) {
    if !matches!(event.kind.as_str(), "mic.level" | "speech.word" | "turn.delta") {
        let p = event.payload.to_string();
        log::debug!("event {} {}", event.kind, p.chars().take(240).collect::<String>());
    }
    if let Err(e) = app.emit(openclicky_core::event::EVENT_CHANNEL, &event) {
        log::warn!("emit {} failed: {e}", event.kind);
    }
}

fn command_output(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = tauri::Builder::default()
        // One openclicky at a time: a second launch just opens Settings in the first one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::open_settings(app, "");
        }))
        .register_uri_scheme_protocol("slot", resources::slot_protocol)
        .invoke_handler(tauri::generate_handler![
            commands::panel_mode,
            commands::panel_screen,
            commands::hide_panel,
            commands::hit_rects,
            commands::panel_hover,
            commands::panel_type,
            commands::panel_peek,
            commands::talk_start,
            commands::talk_stop,
            commands::ask,
            commands::session_note,
            commands::play_sound,
            commands::stop_bed,
            commands::show_settings,
            commands::resources_report,
            commands::theme,
            commands::jobs_list,
            commands::job_get,
            commands::job_note,
            commands::job_start,
            commands::job_answer,
            commands::job_cancel,
            commands::job_dismiss,
            commands::job_delete,
            commands::open_note,
            commands::open_target,
            commands::settings_get,
            commands::settings_set,
            commands::provider_test,
            commands::voices,
            commands::voice_test,
            commands::onboarding_done,
            commands::status,
            commands::voice_status,
            commands::voice_install,
            permissions::permissions_status,
            permissions::permission_request,
            permissions::permission_open_settings,
            permissions::relaunch,
        ])
        .setup(|app| {
            // Menu-bar agent: no Dock icon (LSUIElement is also set in Info.plist for bundles).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".openclicky"));
            let _ = std::fs::create_dir_all(&data_dir);
            let settings = config::Settings::load(&data_dir);

            let (store, pack) = resources::load_store(app.handle());
            let theme = resources::load_theme(store.root());
            let store = Arc::new(store);
            let mut audio = AudioBus::new(store.clone(), RodioOutput::spawn());
            audio.set_muted(settings.muted);
            let skills = skills::load(app.handle());
            let state = AppState {
                store,
                pack,
                theme,
                audio: Mutex::new(audio),
                panel: Mutex::new(panel::PanelState::default()),
                helper: helper::Helper::spawn(),
                data_dir: data_dir.clone(),
                settings: Mutex::new(settings.clone()),
                provider: Mutex::new(None),
                session: session::Session::default(),
                jobs: jobs::Jobs::default(),
                hits: hit::Hits::default(),
                skills,
                os_version: format!(
                    "macOS {} ({})",
                    command_output("/usr/bin/sw_vers", &["-productVersion"]),
                    std::env::consts::ARCH
                ),
                full_name: command_output("/usr/bin/id", &["-F"]),
                quiet: Mutex::new(None),
            };
            state.reload_provider();
            state.jobs.load(&data_dir);
            app.manage(state);

            panel::configure(app.handle());
            panel::rest(app.handle());
            overlay::configure(app.handle());
            shelf::configure(app.handle());
            tray::create(app.handle())?;
            hotkey::spawn(app.handle().clone());
            session::spawn_event_pump(app.handle().clone());
            connectors::start(app.handle().clone());
            browser::start(app.handle().clone());
            kokoro::start(app.handle().clone());
            shelf::refresh(app.handle());

            if !settings.onboarded || !permissions::required_granted() {
                windows::open_settings(app.handle(), if settings.onboarded { "setup" } else { "welcome" });
            }
            // Dev/smoke-test aids.
            if std::env::args().any(|a| a == "--show-panel") {
                panel::set_mode(app.handle(), panel::PanelMode::Idle);
            }
            if let Some(q) = std::env::args().skip_while(|a| a != "--ask").nth(1) {
                let delay = std::env::var("OPENCLICKY_ASK_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    session::ask_text(&h, q);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Windows hide instead of closing; the app lives in the menu bar.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == NOTE {
                    return; // the note window is created on demand; let it close
                }
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
