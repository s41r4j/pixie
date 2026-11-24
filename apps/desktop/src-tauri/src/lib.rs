//! Pixie desktop shell: tray agent, notch panel, hold-to-talk voice loop, screen
//! pointing/drawing, background jobs with notes, settings and onboarding.

mod browser;
mod buddy;
mod chats;
mod commands;
mod config;
mod connectors;
mod google;
mod heal;
mod rename;
mod safety;
mod rollback;
mod watchdog;
mod helper;
mod hit;
mod host;
mod hotkey;
#[cfg(not(target_os = "macos"))]
mod native;
mod canvas;
mod canvas_kit;
mod myself;
mod memory;
mod integrations;
mod updates;
mod jobs;
mod kokoro;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
#[path = "desktop_other.rs"]
mod macos;
mod narrate;
mod opencode;
mod overlay;
mod panel;
mod permissions;
mod resources;
mod scene3d;
mod screen;
mod session;
mod shelf;
mod skills;
mod speech;
mod stt;
mod tray;
mod usage;
mod windows;

use pixie_core::sync::LockOk;
use pixie_core::prompt::Skill;
use pixie_core::{Event, ResourceStore};
use pixie_llm::Provider;
use pixie_voice::{AudioBus, RodioOutput};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

pub const PANEL: &str = "panel";
pub const OVERLAY: &str = "overlay";
pub const SHELF: &str = "shelf";
pub const SETTINGS: &str = "settings";
pub const NOTE: &str = "note";
pub const CANVAS: &str = "canvas";
pub const GUIDE: &str = "guide";
pub const UPDATE: &str = "update";

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
    /// The screen as of the last turn, for snapping pointers to real controls.
    pub last_capture: Mutex<Option<screen::Capture>>,
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
        let mut last = self.quiet.lock_ok();
        if last.map(|t| t.elapsed().as_millis() < 1500).unwrap_or(false) {
            return;
        }
        *last = Some(Instant::now());
        drop(last);
        self.play(slot);
    }

    pub fn provider(&self) -> Option<Arc<dyn Provider>> {
        self.provider.lock_ok().clone()
    }

    pub fn reload_provider(&self) {
        let s = self.settings.lock_ok().clone();
        // opencode on this computer as the model (no API key needed).
        if s.provider == "opencode" && opencode::binary().is_some() {
            let p: Arc<dyn Provider> = Arc::new(opencode::OpencodeProvider);
            log::info!("model: {}", p.label());
            *self.provider.lock_ok() = Some(p);
            return;
        }
        let primary = config::provider_config(&self.data_dir, &s);
        let backup = config::fallback_config(&self.data_dir, &s).filter(|b| primary.as_ref().map(|p| p.kind != b.kind || p.fast_model != b.fast_model).unwrap_or(true));
        let p = match (primary, backup) {
            (Some(p), Some(b)) => Some(Arc::new(pixie_llm::Fallback { primary: usage::Recorded::wrap(p, &self.data_dir), backup: usage::Recorded::wrap(b, &self.data_dir) }) as Arc<dyn Provider>),
            (Some(p), None) => Some(usage::Recorded::wrap(p, &self.data_dir)),
            (None, Some(b)) => Some(usage::Recorded::wrap(b, &self.data_dir)),
            (None, None) => None,
        };
        match &p {
            Some(p) => log::info!("model: {}", p.label()),
            None => log::warn!("model: no key configured (DEEPSEEK / ANTHROPIC_API_KEY)"),
        }
        *self.provider.lock_ok() = p;
    }

    pub fn connected_apps(&self) -> Vec<String> {
        connectors::connected_names()
    }
}

/// Emit a core [`Event`] to every window.
/// The fast events, and the only windows that draw them. Everything else goes to every window.
fn only_for(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        // ~40 a second while the pointer moves.
        "buddy.cursor" => Some(&[OVERLAY]),
        // ~30 a second while the microphone is on.
        "mic.level" => Some(&[PANEL, SETTINGS]),
        "speech.word" | "turn.delta" => Some(&[PANEL]),
        _ => None,
    }
}

pub fn emit(app: &tauri::AppHandle, event: Event) {
    let only = only_for(&event.kind);
    if only.is_none() && log::log_enabled!(log::Level::Debug) {
        let p = event.payload.to_string();
        log::debug!("event {} {}", event.kind, p.chars().take(240).collect::<String>());
    }
    let sent = match only {
        Some(labels) => labels.iter().try_for_each(|l| app.emit_to(*l, pixie_core::event::EVENT_CHANNEL, &event)),
        None => app.emit(pixie_core::event::EVENT_CHANNEL, &event),
    };
    if let Err(e) = sent {
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
    // Logs also go to <app_data>/logs/pixie.log once setup knows where that is.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(heal::Tee)))
        .format_timestamp_millis()
        .init();

    let app = tauri::Builder::default()
        // One Pixie at a time: a second launch just opens Settings in the first one.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::open_settings(app, "");
        }))
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("slot", resources::slot_protocol)
        .invoke_handler(tauri::generate_handler![
            commands::panel_mode,
            commands::panel_screen,
            commands::hide_panel,
            commands::hit_rects,
            commands::panel_hover,
            commands::panel_type,
            commands::panel_peek,
            commands::chat_log,
            commands::chat_clear,
            commands::chats_list,
            commands::chat_get,
            commands::chat_delete,
            commands::open_chats,
            commands::sound_custom,
            commands::mic_test_start,
            commands::mic_test_stop,
            commands::buddy_tip,
            commands::prices_refresh,
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
            commands::models_available,
            commands::usage_summary,
            commands::coders_status,
            commands::guide_show,
            commands::guide_hide,
            commands::voice_status,
            commands::voice_install,
            commands::stt_status,
            commands::stt_install,
            commands::stt_remove,
            commands::apps_status,
            commands::app_set,
            commands::google_sign_in,
            commands::google_sign_out,
            commands::connectors_gallery,
            commands::opencode_status,
            commands::connector_save,
            commands::connector_remove,
            commands::connector_retry,
            commands::connector_add_custom,
            commands::update_status,
            commands::update_check,
            commands::update_restart,
            commands::update_later,
            commands::update_skip,
            commands::update_show,
            memory::memory_list,
            memory::memory_add,
            memory::memory_update,
            memory::memory_delete,
            memory::memory_clear,
            canvas::canvas_list,
            canvas::canvas_load,
            canvas::canvas_save,
            canvas::canvas_delete,
            canvas::canvas_open,
            canvas::canvas_take_ops,
            canvas::canvas_done,
            canvas::canvas_export,
            scene3d::scene3d_get,
            commands::checkup,
            commands::screen_list,
            commands::report_problem,
            commands::show_log,
            commands::show_notices,
            permissions::permissions_status,
            permissions::permission_request,
            permissions::permission_open_settings,
            permissions::relaunch,
            permissions::permission_reset,
        ])
        .setup(|app| {
            // Menu-bar agent: no Dock icon (LSUIElement is also set in Info.plist for bundles).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".pixie"));
            // Up to 0.8.0 the app ID was com.openclicky.app: bring that folder over first.
            let moved = rename::move_data(&data_dir);
            let _ = std::fs::create_dir_all(&data_dir);
            heal::init(&data_dir, &app.package_info().version.to_string());
            match &moved {
                rename::Moved::Nothing => {}
                rename::Moved::Done => {
                    log::info!("rename: moved {} to {}", rename::OLD_ID, data_dir.display());
                    if cfg!(target_os = "macos") {
                        heal::notice("Pixie has a new app ID (pixie.s41r4j.in), so macOS asks for its permissions once more: Settings → Permissions. Your settings, keys, memory and chats came along.");
                    }
                }
                rename::Moved::BothThere(old) => log::warn!("rename: both {} and {} have data; kept the new one", old.display(), data_dir.display()),
                rename::Moved::Failed(e) => {
                    log::error!("rename: couldn't move {}: {e}", rename::OLD_ID);
                    heal::note("rename", "The old data folder couldn't be moved to the new app ID", "Started with a fresh folder", e);
                }
            }
            // A version an update brought that keeps closing at once: the previous one goes back.
            if rollback::on_launch(&data_dir, &app.package_info().version.to_string()) {
                std::process::exit(0);
            }
            // Agents' processes end with Pixie (the helper reads this if Pixie dies first).
            pixie_tools::children::set_file(data_dir.join("agents.pids"));
            let settings = config::Settings::load(&data_dir);

            let (store, pack) = resources::load_store(app.handle());
            let theme = resources::load_theme(store.root());
            let store = Arc::new(store);
            let mut audio = AudioBus::new(store.clone(), RodioOutput::spawn());
            config::apply_audio(&mut audio, &settings);
            hotkey::configure(&settings.shortcut_talk, &settings.shortcut_dictate);
            hotkey::set_dictate_toggle(settings.dictate_mode == "toggle");
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
                last_capture: Mutex::new(None),
                skills,
                os_version: format!(
                    "{} {} ({})",
                    if cfg!(target_os = "macos") { "macOS" } else { std::env::consts::OS },
                    command_output("/usr/bin/sw_vers", &["-productVersion"]),
                    std::env::consts::ARCH
                ),
                full_name: command_output("/usr/bin/id", &["-F"]),
                quiet: Mutex::new(None),
            };
            state.reload_provider();
            state.jobs.load(&data_dir);
            app.manage(state);

            macos::init(app.handle());
            // Tell them once about a crash last time or a file that had to be restored.
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                for n in heal::take_notices() {
                    jobs::toast(&h, &n, "info", None);
                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                }
            });
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
            stt::start(app.handle().clone());
            watchdog::start(app.handle().clone());
            {
                // The speech recognizer, made before the first press so the mic comes on at once.
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let st = h.state::<AppState>();
                    let locale = st.settings.lock().map(|s| s.locale.clone()).unwrap_or_default();
                    let _ = st.helper.call("listen_prewarm", serde_json::json!({ "locale": locale }), std::time::Duration::from_secs(10)).await;
                });
            }
            opencode::init(app.handle());
            updates::spawn(app.handle().clone());
            session::spawn_learner(app.handle().clone());
            shelf::refresh(app.handle());
            {
                // Live model prices for Settings → Usage (cached; refreshed once a day).
                let data = app.state::<AppState>().data_dir.clone();
                let settings = app.state::<AppState>().settings.lock_ok().clone();
                tauri::async_runtime::spawn(async move { usage::warm(&data, &settings).await });
            }

            // The welcome shows once; after that a missing permission is a gentle notch toast,
            // never a window that keeps coming back.
            if !settings.onboarded {
                windows::open_settings(app.handle(), "welcome");
            } else if !permissions::required_granted() {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    jobs::toast(&h, "Some permissions are off · open Settings", "needs", None);
                });
            }
            // Dev/smoke-test aids.
            if std::env::args().any(|a| a == "--show-panel") {
                panel::set_mode(app.handle(), panel::PanelMode::Idle);
            }
            if std::env::args().any(|a| a == "--hold-talk-test") {
                // Speed test: hold ctrl+option for 1.5 s, three times (the log says how soon the
                // microphone came on).
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Three presses: the first after launch, then two warm ones.
                    for _ in 0..3 {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        log::info!("hold talk test: pressing");
                        let r = h.state::<AppState>().helper.call("hold_keys_test", serde_json::json!({ "codes": [59, 58], "ms": 1500 }), std::time::Duration::from_secs(10)).await;
                        log::info!("hold talk test: released {r:?}");
                    }
                });
            }
            if let Some(steps) = std::env::args().skip_while(|a| a != "--live-type-test").nth(1) {
                // Type as you speak without a mic: "first|first words|First words." into the
                // focused field after 3 s (PIXIE_HOLD_CTRL=1 holds control meanwhile).
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let args = serde_json::json!({ "steps": steps.split('|').collect::<Vec<_>>(), "hold_ctrl": std::env::var("PIXIE_HOLD_CTRL").is_ok(), "tap": std::env::var("PIXIE_TAP").unwrap_or_default(), "force_keys": std::env::var("PIXIE_FORCE_KEYS").is_ok() });
                    let r = h.state::<AppState>().helper.call("live_type_test", args, std::time::Duration::from_secs(30)).await;
                    log::info!("live type test: {r:?}");
                });
            }
            if std::env::args().any(|a| a == "--stt-mic-test") {
                // Pixie's speech engine with the real microphone for 5 s, without the shortcut:
                // the log says whether the mic opened, how much it heard and what was recognised.
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let t0 = std::time::Instant::now();
                    let began = stt::begin(&h, false).await;
                    log::info!("stt mic test: began {began:?} in {} ms", t0.elapsed().as_millis());
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    let t1 = std::time::Instant::now();
                    let out = stt::finish(&h).await;
                    log::info!("stt mic test: finished in {} ms: {out:?}", t1.elapsed().as_millis());
                });
            }
            if let Some(q) = std::env::args().skip_while(|a| a != "--ask").nth(1) {
                let delay = std::env::var("PIXIE_ASK_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(300);
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    session::ask_text(&h, q);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                if window.label() == CANVAS {
                    windows::canvas_blurred(window.app_handle());
                }
                if window.label() == scene3d::WINDOW {
                    scene3d::blurred(window.app_handle());
                }
            }
            // Windows hide instead of closing; the app lives in the menu bar.
            if let WindowEvent::Destroyed = event {
                if [NOTE, CANVAS, SETTINGS, UPDATE].contains(&window.label()) {
                    windows::dock_sync(window.app_handle(), window.label());
                }
                // Closing the update window without choosing means Install on Quit.
                if window.label() == UPDATE {
                    updates::closed(window.app_handle());
                }
                if window.label() == scene3d::WINDOW {
                    scene3d::closed();
                }
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Created on demand: let it close and free its memory (for the 3D view, its GPU
                // memory and drawing too; hiding it kept both).
                if [NOTE, CANVAS, SETTINGS, UPDATE, scene3d::WINDOW].contains(&window.label()) {
                    return;
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
        .expect("error while building Pixie");

    app.run(|_app, event| match event {
        // Closing the last window must not quit a menu-bar app; only Quit (exit code) does.
        RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        RunEvent::Exit => {
            helper::QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
            pixie_tools::children::kill_all();
            if let Some(st) = _app.try_state::<AppState>() {
                rollback::on_quit(&st.data_dir, &_app.package_info().version.to_string());
            }
            opencode::on_quit();
            updates::on_quit(_app);
        }
        // Clicking Pixie in the Dock, Launchpad or Finder while it runs brings Settings back.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => windows::open_settings(_app, ""),
        _ => {}
    });
}
