//! Pixie updates itself from GitHub Releases. The release job publishes `latest.json` (version,
//! notes, and a signed download per platform); the updater checks it on launch and every 6 hours,
//! downloads a newer build quietly and verifies its signature against the public key in
//! `tauri.conf.json`.
//!
//! When it's in place:
//! - macOS / Linux: the new app replaces the old one on disk right away; Pixie switches to it at
//!   the next restart.
//! - Windows: the installer is kept and runs on restart (it exits Pixie, installs quietly and
//!   opens the new version).
//!
//! Pixie restarts on its own only when nobody's using it: nothing running, the notch idle, no
//! Settings or note open, and no question for 10 minutes. "Restart now" in Settings → About or the
//! tray menu does it at once, and quitting installs a waiting Windows update.

use crate::{emit, panel, AppState};
use openclicky_core::model::JobState;
use openclicky_core::Event;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const EVERY: Duration = Duration::from_secs(6 * 3600);
const FIRST: Duration = Duration::from_secs(25);
const QUIET_FOR: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading,
    /// Downloaded and verified; waiting for a restart.
    Ready,
    UpToDate,
    Error,
}

#[derive(Default)]
struct State {
    phase: Phase,
    version: Option<String>,
    notes: Option<String>,
    error: Option<String>,
    done: u64,
    total: u64,
    checked_at: Option<i64>,
    /// Windows keeps the installer until restart; macOS/Linux have installed already.
    pending: Option<(Update, Vec<u8>)>,
    last_used: Option<Instant>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap();
    f(g.get_or_insert_with(State::default))
}

fn changed(app: &AppHandle) {
    emit(app, Event::new("update.state", status(app)));
    crate::tray::update_item(app);
}

/// The user did something with Pixie (talked, typed, opened a window): not a moment to restart.
pub fn touch() {
    with(|s| s.last_used = Some(Instant::now()));
}

pub fn status(app: &AppHandle) -> Value {
    let auto = app.state::<AppState>().settings.lock().map(|s| s.auto_update).unwrap_or(true);
    with(|s| {
        json!({
            "current": app.package_info().version.to_string(),
            "phase": s.phase,
            "version": s.version,
            "notes": s.notes,
            "error": s.error,
            "done": s.done,
            "total": s.total,
            "checked_at": s.checked_at,
            "auto": auto,
            "supported": cfg!(not(debug_assertions)),
        })
    })
}

pub fn ready_version() -> Option<String> {
    with(|s| (s.phase == Phase::Ready).then(|| s.version.clone()).flatten())
}

/// Check now; downloads and installs (or stages) a newer version when there is one.
pub async fn check(app: &AppHandle) -> Result<Value, String> {
    if with(|s| matches!(s.phase, Phase::Checking | Phase::Downloading | Phase::Ready)) {
        return Ok(status(app));
    }
    with(|s| {
        s.phase = Phase::Checking;
        s.error = None;
    });
    changed(app);
    let r = run(app).await;
    with(|s| {
        s.checked_at = Some(chrono::Utc::now().timestamp());
        if let Err(e) = &r {
            s.phase = Phase::Error;
            s.error = Some(e.clone());
        }
    });
    changed(app);
    if let Err(e) = r {
        log::warn!("update: {e}");
    }
    Ok(status(app))
}

async fn run(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = match updater.check().await {
        Ok(f) => f,
        // No update file on the latest release (one published before auto-update): nothing newer.
        Err(e) if e.to_string().contains("valid release JSON") => {
            log::info!("update: no latest.json on the newest release ({e})");
            None
        }
        Err(e) => return Err(format!("Couldn't reach GitHub: {e}")),
    };
    let Some(update) = found else {
        with(|s| s.phase = Phase::UpToDate);
        return Ok(());
    };
    log::info!("update: {} → {}", update.current_version, update.version);
    with(|s| {
        s.phase = Phase::Downloading;
        s.version = Some(update.version.clone());
        s.notes = update.body.clone();
        s.done = 0;
        s.total = 0;
    });
    changed(app);
    let tick = app.clone();
    let mut last = Instant::now();
    let bytes = update
        .download(
            move |n, total| {
                with(|s| {
                    s.done += n as u64;
                    s.total = total.unwrap_or(0);
                });
                if last.elapsed() > Duration::from_millis(400) {
                    last = Instant::now();
                    emit(&tick, Event::new("update.state", status(&tick)));
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("The download failed: {e}"))?;
    if cfg!(windows) {
        // Running the installer quits Pixie, so it waits for the restart.
        with(|s| s.pending = Some((update, bytes)));
    } else {
        // Verified against the public key, then the app on disk is replaced.
        update.install(&bytes).map_err(|e| format!("Installing failed: {e}"))?;
    }
    with(|s| s.phase = Phase::Ready);
    let v = ready_version().unwrap_or_default();
    crate::jobs::toast(app, &format!("Pixie {v} is ready. It switches over the next time Pixie restarts."), "info", None);
    Ok(())
}

/// Restart into the new version now.
pub fn restart_now(app: &AppHandle) -> Result<(), String> {
    if !with(|s| s.phase == Phase::Ready) {
        return Err("No update is waiting.".into());
    }
    let pending = with(|s| s.pending.take());
    if let Some((update, bytes)) = pending {
        // Windows: exits Pixie, installs quietly and opens the new version.
        update.install(&bytes).map_err(|e| format!("Installing failed: {e}"))?;
    }
    app.restart();
}

/// On quit: a staged Windows update installs instead of waiting for next time.
pub fn on_quit() {
    if let Some((update, bytes)) = with(|s| s.pending.take()) {
        let update = update.restart_after_install(false);
        if let Err(e) = update.install(&bytes) {
            log::warn!("update on quit: {e}");
        }
    }
}

/// Nobody's using Pixie: safe to restart without interrupting.
fn idle(app: &AppHandle) -> bool {
    let st = app.state::<AppState>();
    let busy_job = st.jobs.list().iter().any(|j| matches!(j.state, JobState::Running | JobState::WaitingOnUser));
    let windows_open = [crate::SETTINGS, crate::NOTE, crate::CANVAS]
        .iter()
        .any(|l| app.get_webview_window(l).map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false));
    let recent = with(|s| s.last_used.map(|t| t.elapsed() < QUIET_FOR).unwrap_or(false));
    panel::mode(app) == panel::PanelMode::Hidden && !busy_job && !windows_open && !recent
}

/// Check on launch and every 6 hours; restart into a waiting update when Pixie is idle.
pub fn spawn(app: AppHandle) {
    if cfg!(debug_assertions) {
        return; // dev builds aren't released builds
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST).await;
        let mut next_check = Instant::now();
        loop {
            let auto = app.state::<AppState>().settings.lock().map(|s| s.auto_update).unwrap_or(true);
            if auto && Instant::now() >= next_check {
                let _ = check(&app).await;
                next_check = Instant::now() + EVERY;
            }
            if auto && ready_version().is_some() && idle(&app) {
                log::info!("update: idle, restarting into {}", ready_version().unwrap_or_default());
                let _ = restart_now(&app);
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
