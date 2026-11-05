//! Push-to-talk: hold **ctrl+option** to talk (BUILD-SPEC §1).
//!
//! Modifier-only chords can't be registered as global shortcuts, and event taps need
//! Accessibility/Input Monitoring. Polling `CGEventSourceFlagsState` needs no permission
//! and costs ~nothing at 50 Hz, so the hotkey works before the wizard is done.
//!
//! Behaviour:
//! - press   → panel opens listening (mic live, screenshot taken in parallel)
//! - release → the turn runs; a quick tap (<250 ms) opens the text box, or closes the panel
//!   if it was already open
//! - Escape  → stop speech + the turn, close
//! - fn+⌃    → dictate into the focused field

use crate::panel::{self, PanelMode};
use crate::session;
use tauri::Manager;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::AppHandle;

// CGEventFlags masks.
pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_OPTION: u64 = 0x0008_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;
pub const FLAG_FN: u64 = 0x0080_0000;

const KEY_ESCAPE: u16 = 53;
const POLL: Duration = Duration::from_millis(20);
pub const TAP: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    TalkDown,
    TalkUp { held: Duration },
    /// fn+control: dictate clean text into the focused field.
    DictateDown,
    DictateUp,
    Escape,
}

/// Pure edge detector over polled keyboard state.
#[derive(Debug, Default)]
pub struct ChordTracker {
    talk_since: Option<Instant>,
    dictate: bool,
    escape: bool,
}

const ALL: u64 = FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND | FLAG_SHIFT | FLAG_FN;
pub const DEFAULT_TALK: u64 = FLAG_CONTROL | FLAG_OPTION;
pub const DEFAULT_DICTATE: u64 = FLAG_FN | FLAG_CONTROL;

/// The chords in use (Settings → Shortcuts can change them while running).
static TALK: AtomicU64 = AtomicU64::new(DEFAULT_TALK);
static DICTATE: AtomicU64 = AtomicU64::new(DEFAULT_DICTATE);

/// "ctrl+option" / "fn+ctrl" / "cmd+shift" → modifier mask. None when empty or unknown.
pub fn parse(s: &str) -> Option<u64> {
    let mut m = 0;
    for part in s.split('+').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()) {
        m |= match part.as_str() {
            "ctrl" | "control" | "⌃" => FLAG_CONTROL,
            "option" | "opt" | "alt" | "⌥" => FLAG_OPTION,
            "cmd" | "command" | "⌘" => FLAG_COMMAND,
            "shift" | "⇧" => FLAG_SHIFT,
            "fn" | "globe" => FLAG_FN,
            _ => return None,
        };
    }
    // At least two keys, or fn alone (a single ⌃ or ⇧ would fire while typing).
    (m != 0 && (m.count_ones() >= 2 || m == FLAG_FN)).then_some(m)
}

/// Apply the shortcuts from settings (falls back to the defaults; never two equal chords).
pub fn configure(talk: &str, dictate: &str) {
    let t = parse(talk).unwrap_or(DEFAULT_TALK);
    let d = parse(dictate).filter(|d| *d != t).unwrap_or(if t == DEFAULT_DICTATE { FLAG_FN | FLAG_OPTION } else { DEFAULT_DICTATE });
    TALK.store(t, Ordering::SeqCst);
    DICTATE.store(d, Ordering::SeqCst);
}

fn talk_chord(flags: u64) -> bool {
    flags & ALL == TALK.load(Ordering::Relaxed)
}

fn dictate_chord(flags: u64) -> bool {
    flags & ALL == DICTATE.load(Ordering::Relaxed)
}

impl ChordTracker {
    pub fn update(&mut self, flags: u64, escape_down: bool, now: Instant) -> Vec<HotkeyEvent> {
        let mut out = Vec::new();
        match (self.talk_since, talk_chord(flags)) {
            (None, true) => {
                self.talk_since = Some(now);
                out.push(HotkeyEvent::TalkDown);
            }
            (Some(since), false) => {
                self.talk_since = None;
                out.push(HotkeyEvent::TalkUp { held: now.saturating_duration_since(since) });
            }
            _ => {}
        }
        let dictate = dictate_chord(flags);
        if dictate != self.dictate {
            self.dictate = dictate;
            out.push(if dictate { HotkeyEvent::DictateDown } else { HotkeyEvent::DictateUp });
        }
        if escape_down && !self.escape {
            out.push(HotkeyEvent::Escape);
        }
        self.escape = escape_down;
        out
    }
}

/// Spawn the polling thread (macOS only). Also drives click-through hit testing.
pub fn spawn(app: AppHandle) {
    #[cfg(target_os = "macos")]
    std::thread::Builder::new()
        .name("openclicky-hotkey".into())
        .spawn(move || {
            let mut tracker = ChordTracker::default();
            let mut was_open_at_press = false;
            loop {
                let flags = crate::macos::modifier_flags();
                let esc = crate::macos::key_down(KEY_ESCAPE);
                for ev in tracker.update(flags, esc, Instant::now()) {
                    handle(&app, ev, &mut was_open_at_press);
                }
                crate::hit::tick(&app);
                std::thread::sleep(POLL);
            }
        })
        .expect("spawn hotkey thread");
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

fn handle(app: &AppHandle, ev: HotkeyEvent, was_open_at_press: &mut bool) {
    log::debug!("hotkey: {ev:?}");
    match ev {
        HotkeyEvent::TalkDown => {
            *was_open_at_press = panel::mode(app) != PanelMode::Hidden;
            session::talk_down(app);
        }
        HotkeyEvent::TalkUp { held } => session::talk_up(app, held < TAP, *was_open_at_press),
        HotkeyEvent::Escape => {
            if panel::mode(app) != PanelMode::Hidden || app.state::<crate::AppState>().session.is_speaking() {
                session::escape(app);
            }
        }
        HotkeyEvent::DictateDown => session::dictate_down(app),
        HotkeyEvent::DictateUp => session::dictate_up(app),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CO: u64 = FLAG_CONTROL | FLAG_OPTION;

    #[test]
    fn hold_and_release_ctrl_option() {
        let mut t = ChordTracker::default();
        let t0 = Instant::now();
        assert_eq!(t.update(FLAG_CONTROL, false, t0), vec![]);
        assert_eq!(t.update(CO, false, t0), vec![HotkeyEvent::TalkDown]);
        assert_eq!(t.update(CO, false, t0 + Duration::from_millis(100)), vec![]);
        assert_eq!(
            t.update(FLAG_OPTION, false, t0 + Duration::from_millis(900)),
            vec![HotkeyEvent::TalkUp { held: Duration::from_millis(900) }]
        );
        assert_eq!(t.update(0, false, t0 + Duration::from_millis(950)), vec![]);
    }

    #[test]
    fn extra_modifiers_do_not_trigger_talk() {
        let mut t = ChordTracker::default();
        let now = Instant::now();
        assert_eq!(t.update(CO | FLAG_COMMAND, false, now), vec![]);
        assert_eq!(t.update(CO | FLAG_SHIFT, false, now), vec![]);
        // adding cmd mid-hold ends the talk
        assert_eq!(t.update(CO, false, now), vec![HotkeyEvent::TalkDown]);
        assert!(matches!(t.update(CO | FLAG_COMMAND, false, now)[..], [HotkeyEvent::TalkUp { .. }]));
    }

    #[test]
    fn fn_control_is_dictation_not_talk() {
        let mut t = ChordTracker::default();
        let now = Instant::now();
        assert_eq!(t.update(FLAG_FN | FLAG_CONTROL, false, now), vec![HotkeyEvent::DictateDown]);
        assert_eq!(t.update(0, false, now), vec![HotkeyEvent::DictateUp]);
    }

    #[test]
    fn parses_chords() {
        assert_eq!(parse("ctrl+option"), Some(CO));
        assert_eq!(parse("fn"), Some(FLAG_FN));
        assert_eq!(parse("cmd + shift"), Some(FLAG_COMMAND | FLAG_SHIFT));
        assert_eq!(parse("shift"), None);
        assert_eq!(parse("ctrl+banana"), None);
    }

    #[test]
    fn escape_fires_once_per_press() {
        let mut t = ChordTracker::default();
        let now = Instant::now();
        assert_eq!(t.update(0, true, now), vec![HotkeyEvent::Escape]);
        assert_eq!(t.update(0, true, now), vec![]);
        assert_eq!(t.update(0, false, now), vec![]);
        assert_eq!(t.update(0, true, now), vec![HotkeyEvent::Escape]);
    }
}
