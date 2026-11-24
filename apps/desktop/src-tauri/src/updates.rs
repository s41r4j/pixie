//! Pixie updates itself from GitHub Releases. The release job publishes `latest.json` (version,
//! notes, and a signed download per platform); the updater checks it on launch and every 6 hours,
//! downloads a newer build quietly and verifies its signature against the public key in
//! `tauri.conf.json`.
//!
//! Since 0.8.2 nothing is installed without asking. Once the download is verified, the update
//! window (`Update.tsx`) shows what's new and offers, like the Mac's familiar updater:
//! - **Install and Relaunch**: install now and open the new version.
//! - **Install on Quit**: install when Pixie quits (closing the window does the same).
//! - **Skip This Version**: forget this download; the next release is offered as usual.
//!
//! The window waits for a quiet moment (nothing running, the notch idle, nothing asked for two
//! minutes); a check they started themselves shows it at once. "Automatically download and install
//! updates" (`auto_update`) decides whether Pixie checks and downloads by itself; with it off,
//! only Check now (Settings → About, or the tray menu) looks for a new version.

use pixie_core::sync::LockOk;
use crate::{emit, panel, AppState};
use pixie_core::model::JobState;
use pixie_core::Event;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const EVERY: Duration = Duration::from_secs(6 * 3600);
const FIRST: Duration = Duration::from_secs(25);
/// How long nobody has asked Pixie anything before the update window may open by itself.
const QUIET_FOR: Duration = Duration::from_secs(2 * 60);
/// After a check that couldn't fetch latest.json (GitHub's asset servers sometimes answer 504),
/// try again this soon instead of waiting the full six hours.
const RETRY: Duration = Duration::from_secs(15 * 60);
static RETRY_SOON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading,
    /// Downloaded and verified; waiting for them to choose.
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
    /// The verified download, installed when they say so (or at quit).
    pending: Option<(Update, Vec<u8>)>,
    /// The update window has been shown for this download.
    shown: bool,
    /// They chose Install on Quit (or closed the window).
    on_quit: bool,
    last_used: Option<Instant>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock_ok();
    f(g.get_or_insert_with(State::default))
}

fn changed(app: &AppHandle) {
    emit(app, Event::new("update.state", status(app)));
    crate::tray::update_item(app);
}

/// The user did something with Pixie (talked, typed, opened a window): not a moment to interrupt.
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
            "on_quit": s.on_quit,
            "supported": cfg!(not(debug_assertions)),
        })
    })
}

pub fn ready_version() -> Option<String> {
    with(|s| (s.phase == Phase::Ready).then(|| s.version.clone()).flatten())
}

/// Check now; downloads and verifies a newer version when there is one. `asked`: they started it
/// (Check now), so the update window opens as soon as the download is ready.
pub async fn check(app: &AppHandle, asked: bool) -> Result<Value, String> {
    if with(|s| s.phase == Phase::Ready) {
        if asked {
            show(app);
        }
        return Ok(status(app));
    }
    if with(|s| matches!(s.phase, Phase::Checking | Phase::Downloading)) {
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
    match r {
        Err(e) => log::warn!("update: {e}"),
        Ok(()) if asked && ready_version().is_some() => show(app),
        Ok(()) => {}
    }
    Ok(status(app))
}

async fn run(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    RETRY_SOON.store(false, std::sync::atomic::Ordering::SeqCst);
    let found = match updater.check().await {
        Ok(f) => f,
        // latest.json couldn't be fetched: missing on the release, or GitHub failing for a moment
        // (the updater can't tell which). Treat it as nothing newer and look again soon.
        Err(e) if e.to_string().contains("valid release JSON") => {
            log::info!("update: couldn't fetch latest.json ({e}); trying again in 15 minutes");
            RETRY_SOON.store(true, std::sync::atomic::Ordering::SeqCst);
            None
        }
        Err(e) => {
            RETRY_SOON.store(true, std::sync::atomic::Ordering::SeqCst);
            return Err(format!("Couldn't reach GitHub: {e}"));
        }
    };
    let data = app.state::<AppState>().data_dir.clone();
    // A version that was rolled back or skipped isn't offered again; the next release is.
    let found = found.filter(|u| crate::rollback::skipped(&data).as_deref() != Some(u.version.as_str()));
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
    // Verified against the public key as it's installed; nothing changes on disk until they choose.
    with(|s| {
        s.pending = Some((update, bytes));
        s.phase = Phase::Ready;
        s.shown = false;
        s.on_quit = false;
    });
    Ok(())
}

/// Open the update window (once per download unless they ask for it again).
pub fn show(app: &AppHandle) {
    with(|s| s.shown = true);
    crate::windows::open_update(app);
}

/// Put the new version in place: macOS keeps the running one first, in case the new one won't
/// start; Windows runs the installer, which ends Pixie.
fn install(app: &AppHandle, update: &Update, bytes: &[u8]) -> Result<(), String> {
    if !cfg!(windows) {
        let data = app.state::<AppState>().data_dir.clone();
        crate::rollback::keep_current(&data, &update.current_version, &update.version);
    }
    update.install(bytes).map_err(|e| format!("Installing failed: {e}"))
}

/// Install and Relaunch.
pub fn restart_now(app: &AppHandle) -> Result<(), String> {
    let Some((update, bytes)) = with(|s| s.pending.take()) else {
        return Err("No update is waiting.".into());
    };
    log::info!("update: installing {} and relaunching", update.version);
    if let Err(e) = install(app, &update, &bytes) {
        // Keep it, so they can try again.
        with(|s| s.pending = Some((update, bytes)));
        return Err(e);
    }
    app.restart();
}

/// Install on Quit (also what closing the window means).
pub fn later(app: &AppHandle) {
    with(|s| s.on_quit = true);
    changed(app);
}

/// The window closed: unless they skipped or are installing, that's Install on Quit.
pub fn closed(app: &AppHandle) {
    if with(|s| s.pending.is_some() && !s.on_quit) {
        later(app);
    }
}

/// Skip This Version: forget the download; this version isn't offered again, the next one is.
pub fn skip(app: &AppHandle) {
    let version = with(|s| {
        s.pending = None;
        s.on_quit = false;
        s.phase = Phase::Idle;
        s.version.take()
    });
    if let Some(v) = version {
        log::info!("update: skipping {v}");
        crate::rollback::skip(&app.state::<AppState>().data_dir, &v);
    }
    changed(app);
}

/// On quit: a waiting update installs (they chose Install on Quit, closed the window, or haven't
/// answered yet), without opening Pixie again.
pub fn on_quit(app: &AppHandle) {
    if let Some((update, bytes)) = with(|s| s.pending.take()) {
        let update = update.restart_after_install(false);
        if let Err(e) = install(app, &update, &bytes) {
            log::warn!("update on quit: {e}");
        }
    }
}

/// Nobody's using Pixie: a moment the update window won't interrupt.
fn quiet(app: &AppHandle) -> bool {
    let st = app.state::<AppState>();
    let busy_job = st.jobs.list().iter().any(|j| matches!(j.state, JobState::Running | JobState::WaitingOnUser));
    let recent = with(|s| s.last_used.map(|t| t.elapsed() < QUIET_FOR).unwrap_or(false));
    panel::mode(app) == panel::PanelMode::Hidden && !busy_job && !recent
}

/// Check on launch and every 6 hours; open the update window at a quiet moment once one is ready.
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
                let _ = check(&app, false).await;
                let soon = RETRY_SOON.load(std::sync::atomic::Ordering::SeqCst);
                next_check = Instant::now() + if soon { RETRY } else { EVERY };
            }
            let waiting = with(|s| s.phase == Phase::Ready && !s.shown && !s.on_quit);
            if waiting && quiet(&app) {
                log::info!("update: {} is ready; asking", ready_version().unwrap_or_default());
                show(&app);
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}
