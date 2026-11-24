//! Watchdogs and stuck states (self-healing 1), and permissions turned off while Pixie runs (3).
//!
//! Every 30 s: the helper, the natural voice and each connector server must still answer; one
//! that hangs (not only one that exits) is restarted. Every 5 s: the hotkey poller must still be
//! going round, and the notch mustn't sit in a mode nothing is driving any more (thinking after
//! the answer ended, listening with the keys let go). Three restarts of one part in five minutes
//! and it's left alone: they're told once, with what to do, and it goes in the problems list.
//! No restart loops.

use pixie_core::sync::LockOk;
use crate::panel::{self, PanelMode};
use crate::{hotkey, session, AppState};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const STRIKES: usize = 3;
const STRIKE_WINDOW: Duration = Duration::from_secs(5 * 60);

static APP: OnceLock<AppHandle> = OnceLock::new();

#[derive(Default)]
struct Part {
    times: Vec<Instant>,
    gave_up: bool,
}

fn parts() -> &'static Mutex<BTreeMap<String, Part>> {
    static P: OnceLock<Mutex<BTreeMap<String, Part>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// What to do about one more restart of a part: restart it, or leave it alone (the third in five
/// minutes; already given up).
#[derive(Debug, PartialEq)]
enum Verdict {
    Restart,
    GiveUp,
    LeftAlone,
}

fn judge(p: &mut Part, now: Instant) -> Verdict {
    if p.gave_up {
        return Verdict::LeftAlone;
    }
    p.times.retain(|t| now.duration_since(*t) < STRIKE_WINDOW);
    p.times.push(now);
    if p.times.len() >= STRIKES {
        p.gave_up = true;
        return Verdict::GiveUp;
    }
    Verdict::Restart
}

/// Count a restart of `part`; on giving up, note it and tell them once.
fn strike(app: &AppHandle, part: &str, what: &str, gave_up_text: &str) -> bool {
    let v = judge(parts().lock_ok().entry(part.to_string()).or_default(), Instant::now());
    match v {
        Verdict::Restart => {
            log::warn!("watchdog: {what}; restarting it");
            crate::heal::note("watchdog", what, "Restarted it", "");
            true
        }
        Verdict::GiveUp => {
            log::error!("watchdog: {what}, three times in five minutes; leaving it alone");
            crate::heal::note("watchdog", &format!("{what}, again and again"), "Stopped restarting it", "");
            crate::jobs::toast(app, gave_up_text, "needs", None);
            false
        }
        Verdict::LeftAlone => false,
    }
}

/// They fixed it by hand (Retry): count from zero again.
pub fn forgive(part: &str) {
    parts().lock_ok().remove(part);
}

pub fn start(app: AppHandle) {
    let _ = APP.set(app.clone());
    tauri::async_runtime::spawn(async move {
        let mut round = 0u64;
        let mut stuck: Option<(PanelMode, Instant)> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            round += 1;
            poller(&app);
            unstick(&app, &mut stuck);
            if round.is_multiple_of(6) {
                helper(&app).await;
                voice(&app);
                connectors(&app).await;
            }
        }
    });
}

/// The shortcut and click-through poller stopped going round: start a fresh one.
fn poller(app: &AppHandle) {
    if hotkey::since_tick() > Duration::from_secs(10)
        && strike(app, "hotkey", "The shortcut listener got stuck", "The talk shortcut keeps getting stuck. Restart Pixie from the menu bar; if it keeps happening, Settings → About → Report a problem.")
    {
        hotkey::spawn(app.clone());
    }
}

/// How long a mode may sit with nothing driving it.
fn stuck_after(app: &AppHandle, mode: PanelMode) -> Option<Duration> {
    let st = app.state::<AppState>();
    match mode {
        // The answer finished (or failed) without the notch being told.
        PanelMode::Thinking if !st.session.busy() => Some(Duration::from_secs(15)),
        // The key-up was missed: the keys aren't held any more.
        PanelMode::Listening if !hotkey::talk_held() => Some(Duration::from_secs(10)),
        PanelMode::Dictating if hotkey::dictate_is_toggle() => Some(Duration::from_secs(10 * 60)),
        PanelMode::Dictating if !hotkey::dictate_held() => Some(Duration::from_secs(10)),
        // Written answers stay up to 90 s; hovering keeps them.
        PanelMode::Answer if !st.session.busy() && !st.session.hovered() => Some(Duration::from_secs(150)),
        _ => None,
    }
}

fn unstick(app: &AppHandle, stuck: &mut Option<(PanelMode, Instant)>) {
    let mode = panel::mode(app);
    let Some(limit) = stuck_after(app, mode) else {
        *stuck = None;
        return;
    };
    let since = match stuck {
        Some((m, t)) if *m == mode => *t,
        _ => {
            *stuck = Some((mode, Instant::now()));
            return;
        }
    };
    if since.elapsed() < limit {
        return;
    }
    *stuck = None;
    log::warn!("watchdog: the notch sat in {mode:?} with nothing going on; resetting it");
    crate::heal::note("stuck", &format!("The notch stayed in {mode:?} with nothing going on"), "Put it back to rest", "");
    match mode {
        PanelMode::Listening => session::talk_up(app, false, false),
        PanelMode::Dictating => session::dictate_up(app),
        _ => panel::set_mode(app, PanelMode::Hidden),
    }
}

/// The helper answers a ping (twice 8 s before it counts as hung).
async fn helper(app: &AppHandle) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let st = app.state::<AppState>();
    if !st.helper.available() || matches!(panel::mode(app), PanelMode::Listening | PanelMode::Dictating) {
        return;
    }
    for _ in 0..2 {
        if st.helper.call("ping", json!({}), Duration::from_secs(8)).await.is_ok() {
            return;
        }
    }
    if strike(app, "helper", "The screen and speech helper stopped answering", "The screen and speech helper keeps hanging. Restart Pixie from the menu bar; if it keeps happening, Settings → About → Report a problem.") {
        crate::helper::force_restart(app);
    }
}

fn voice(app: &AppHandle) {
    if !crate::kokoro::hung() {
        return;
    }
    if strike(app, "voice", "The natural voice stopped answering", "The natural voice keeps hanging, so Pixie uses the Mac's voice until it restarts.") {
        crate::kokoro::kill_hung();
    } else {
        crate::kokoro::switch_off();
    }
}

async fn connectors(app: &AppHandle) {
    for (name, client) in crate::connectors::clients() {
        let r = client.request("ping", json!({}), Duration::from_secs(10)).await;
        // An error reply is fine (some servers don't do ping): only silence counts.
        let hung = matches!(&r, Err(e) if e.ends_with("timed out") || e.ends_with("no reply"));
        if !hung {
            continue;
        }
        let part = format!("connector:{name}");
        if strike(app, &part, &format!("The {name} connector stopped answering"), &format!("The {name} connector keeps hanging, so it's paused. Settings → Connectors → Retry when you want it back.")) {
            let _ = crate::connectors::retry(app, &name);
        } else {
            crate::connectors::pause(&name, "Kept hanging, so it's paused. Press Retry to try again.");
        }
    }
}

// ---- permissions turned off while Pixie runs --------------------------------------------------

fn watching() -> &'static Mutex<BTreeSet<&'static str>> {
    static W: OnceLock<Mutex<BTreeSet<&'static str>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(BTreeSet::new()))
}

/// Which permission a helper error is about.
fn permission_in(err: &str) -> Option<&'static str> {
    if err.contains("Accessibility access is off") {
        Some("accessibility")
    } else if err.contains("Screen Recording is off") {
        Some("screen")
    } else if err.contains("Microphone and Speech Recognition") {
        let mic_ok = crate::permissions::check("microphone").map(|(_, ok)| ok).unwrap_or(true);
        Some(if mic_ok { "speech" } else { "microphone" })
    } else {
        None
    }
}

/// A helper call failed: if a permission was turned off, say which, open its page, and say so
/// again when it's back.
pub fn helper_error(err: &str) {
    let (Some(app), Some(id)) = (APP.get(), permission_in(err)) else { return };
    let Some((label, granted)) = crate::permissions::check(id) else { return };
    if granted || !watching().lock_ok().insert(id) {
        return;
    }
    log::warn!("permissions: {label} is off");
    crate::heal::note("permission", &format!("{label} was off for Pixie"), "Opened its page in System Settings", "");
    crate::jobs::toast(app, &format!("{label} is off for Pixie, so that didn't work. Its page in System Settings is open: switch Pixie on there."), "needs", None);
    crate::permissions::open_pane(id);
    let app = app.clone();
    std::thread::spawn(move || {
        // Check again every 2 s for 5 minutes.
        for _ in 0..150 {
            std::thread::sleep(Duration::from_secs(2));
            if crate::permissions::check(id).map(|(_, ok)| ok).unwrap_or(false) {
                log::info!("permissions: {label} is back");
                if id == "screen" {
                    // The helper only sees Screen Recording from a fresh start.
                    crate::helper::force_restart(&app);
                }
                crate::jobs::toast(&app, &format!("{label} is on again."), "info", None);
                break;
            }
        }
        watching().lock_ok().remove(id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_strikes_in_five_minutes() {
        let mut p = Part::default();
        let t = Instant::now();
        assert_eq!(judge(&mut p, t), Verdict::Restart);
        assert_eq!(judge(&mut p, t + Duration::from_secs(60)), Verdict::Restart);
        assert_eq!(judge(&mut p, t + Duration::from_secs(120)), Verdict::GiveUp);
        assert_eq!(judge(&mut p, t + Duration::from_secs(130)), Verdict::LeftAlone);
        // Spread out, they never add up.
        let mut q = Part::default();
        for i in 0..6 {
            assert_eq!(judge(&mut q, t + Duration::from_secs(200 * i)), Verdict::Restart);
        }
    }

    #[test]
    fn permission_errors_are_recognised() {
        assert_eq!(permission_in("Accessibility access is off."), Some("accessibility"));
        assert_eq!(permission_in("Screen Recording is off for Pixie (Settings → Permissions)."), Some("screen"));
        assert_eq!(permission_in("I couldn't find \"Save\"."), None);
    }
}
