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
/// At rest (no key held and the pointer still for [`REST_AFTER`]) the poll halves its rate, so an
/// idle Mac is woken half as often. The first key or pointer movement brings it straight back.
const POLL_RESTING: Duration = Duration::from_millis(40);
const REST_AFTER: Duration = Duration::from_secs(3);

/// How long to sleep before the next round.
fn pace(quiet_for: Duration) -> Duration {
    if quiet_for >= REST_AFTER {
        POLL_RESTING
    } else {
        POLL
    }
}
pub const TAP: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    TalkDown,
    TalkUp { held: Duration },
    /// fn+control: dictate clean text into the focused field.
    DictateDown,
    DictateUp,
    /// fn alone pressed and let go quickly: the Mac's (fn twice starts its own Dictation). Only
    /// used to stop a press-to-start dictation that's running.
    DictateTap,
    Escape,
}

/// Pure edge detector over polled keyboard state.
#[derive(Debug, Default)]
pub struct ChordTracker {
    talk_since: Option<Instant>,
    dictate: bool,
    escape: bool,
    /// fn-alone chords: when the keys went down, and whether the hold has counted yet.
    talk_pressed: Option<Instant>,
    dictate_pressed: Option<Instant>,
}

/// A chord of fn alone is shared with macOS (fn twice starts the Mac's own Dictation; one press
/// can show emoji or switch input source), so it only counts once it has been held this long.
pub const SHARED_HOLD: Duration = Duration::from_millis(300);

fn shared(chord: u64) -> bool {
    chord == FLAG_FN
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

/// Dictation shortcut works as a toggle (press to start, press to stop) instead of hold.
static DICTATE_TOGGLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_dictate_toggle(on: bool) {
    DICTATE_TOGGLE.store(on, Ordering::SeqCst);
}

/// Apply the shortcuts from settings (falls back to the defaults; never two equal chords).
pub fn configure(talk: &str, dictate: &str) {
    let t = parse(talk).unwrap_or(DEFAULT_TALK);
    let d = parse(dictate).filter(|d| *d != t).unwrap_or(if t == DEFAULT_DICTATE { FLAG_FN | FLAG_OPTION } else { DEFAULT_DICTATE });
    // Windows and Linux can't see fn: a chord with it becomes Ctrl+Win.
    let (t, d) = if cfg!(target_os = "macos") {
        (t, d)
    } else {
        let no_fn = |m: u64| if m & FLAG_FN != 0 { FLAG_CONTROL | FLAG_COMMAND } else { m };
        let (t, d) = (no_fn(t), no_fn(d));
        (t, if d == t { FLAG_CONTROL | FLAG_SHIFT } else { d })
    };
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
        // fn alone: a press only counts once held for SHARED_HOLD; a quick press is the Mac's.
        let talk_now = if shared(TALK.load(Ordering::Relaxed)) {
            match (talk_chord(flags), self.talk_pressed) {
                (true, None) => {
                    self.talk_pressed = Some(now);
                    false
                }
                (true, Some(t)) => self.talk_since.is_some() || now.saturating_duration_since(t) >= SHARED_HOLD,
                (false, _) => {
                    self.talk_pressed = None;
                    false
                }
            }
        } else {
            talk_chord(flags)
        };
        match (self.talk_since, talk_now) {
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
        if shared(DICTATE.load(Ordering::Relaxed)) {
            match (dictate_chord(flags), self.dictate_pressed) {
                (true, None) => self.dictate_pressed = Some(now),
                (true, Some(t)) if !self.dictate && now.saturating_duration_since(t) >= SHARED_HOLD => {
                    self.dictate = true;
                    out.push(HotkeyEvent::DictateDown);
                }
                (false, Some(_)) => {
                    self.dictate_pressed = None;
                    out.push(if std::mem::take(&mut self.dictate) { HotkeyEvent::DictateUp } else { HotkeyEvent::DictateTap });
                }
                _ => {}
            }
        } else {
            let dictate = dictate_chord(flags);
            if dictate != self.dictate {
                self.dictate = dictate;
                out.push(if dictate { HotkeyEvent::DictateDown } else { HotkeyEvent::DictateUp });
            }
        }
        if escape_down && !self.escape {
            out.push(HotkeyEvent::Escape);
        }
        self.escape = escape_down;
        out
    }
}

/// Which poller is the live one, and when it last went round (ms since the first started).
static POLLER: AtomicU64 = AtomicU64::new(0);
static TICKED: AtomicU64 = AtomicU64::new(0);

fn clock() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// How long since the poller last went round (the watchdog starts a new one when it's stuck).
pub fn since_tick() -> Duration {
    Duration::from_millis(clock().saturating_sub(TICKED.load(Ordering::Relaxed)))
}

/// Whether the talk or dictate keys are held right now.
pub fn talk_held() -> bool {
    talk_chord(crate::macos::modifier_flags())
}
pub fn dictate_held() -> bool {
    dictate_chord(crate::macos::modifier_flags())
}
pub fn dictate_is_toggle() -> bool {
    DICTATE_TOGGLE.load(Ordering::Relaxed)
}

/// Spawn the polling thread. Also drives click-through hit testing. A new one replaces the
/// old (which stops at its next round).
pub fn spawn(app: AppHandle) {
    let me = POLLER.fetch_add(1, Ordering::SeqCst) + 1;
    TICKED.store(clock(), Ordering::Relaxed);
    std::thread::Builder::new()
        .name("pixie-hotkey".into())
        .spawn(move || {
            let mut tracker = ChordTracker::default();
            let mut was_open_at_press = false;
            let mut last_cursor = (0.0, 0.0);
            let mut last_active = Instant::now();
            loop {
                if POLLER.load(Ordering::SeqCst) != me {
                    return;
                }
                TICKED.store(clock(), Ordering::Relaxed);
                let flags = crate::macos::modifier_flags();
                let esc = crate::macos::key_down(KEY_ESCAPE);
                for ev in tracker.update(flags, esc, Instant::now()) {
                    handle(&app, ev, &mut was_open_at_press);
                }
                let cursor = crate::hit::tick(&app);
                // Modifier keys only (caps lock and the device bits stay set at rest).
                if flags & 0x00FE_0000 != 0 || esc || cursor != last_cursor {
                    last_cursor = cursor;
                    last_active = Instant::now();
                }
                std::thread::sleep(pace(last_active.elapsed()));
            }
        })
        .expect("spawn hotkey thread");
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
        // Hold to dictate (default), or press once to start and again to stop.
        HotkeyEvent::DictateDown if DICTATE_TOGGLE.load(Ordering::Relaxed) => {
            if session::dictating(app) {
                session::dictate_up(app);
            } else {
                session::dictate_down(app);
            }
        }
        HotkeyEvent::DictateUp if DICTATE_TOGGLE.load(Ordering::Relaxed) => {}
        HotkeyEvent::DictateDown => session::dictate_down(app),
        HotkeyEvent::DictateUp => session::dictate_up(app),
        // A quick fn press stops a press-to-start dictation; otherwise it's the Mac's.
        HotkeyEvent::DictateTap => {
            if DICTATE_TOGGLE.load(Ordering::Relaxed) && session::dictating(app) {
                session::dictate_up(app);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CO: u64 = FLAG_CONTROL | FLAG_OPTION;
    /// The chords are global: tests that change them run one at a time.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn hold_and_release_ctrl_option() {
        let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
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
        let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut t = ChordTracker::default();
        let now = Instant::now();
        assert_eq!(t.update(CO | FLAG_COMMAND, false, now), vec![]);
        assert_eq!(t.update(CO | FLAG_SHIFT, false, now), vec![]);
        // adding cmd mid-hold ends the talk
        assert_eq!(t.update(CO, false, now), vec![HotkeyEvent::TalkDown]);
        assert!(matches!(t.update(CO | FLAG_COMMAND, false, now)[..], [HotkeyEvent::TalkUp { .. }]));
    }

    // fn exists only on the Mac; elsewhere a chord with fn becomes Ctrl+Win (`configure`).
    #[cfg(target_os = "macos")]
    #[test]
    fn fn_control_is_dictation_not_talk() {
        let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let mut t = ChordTracker::default();
        let now = Instant::now();
        assert_eq!(t.update(FLAG_FN | FLAG_CONTROL, false, now), vec![HotkeyEvent::DictateDown]);
        assert_eq!(t.update(0, false, now), vec![HotkeyEvent::DictateUp]);
    }

    /// fn alone is shared with the Mac: its fn-fn Dictation (two quick presses) doesn't wake
    /// Pixie; holding fn does, and letting go ends it.
    #[test]
    fn the_poll_rests_when_nothing_moves() {
        assert_eq!(pace(Duration::from_millis(500)), POLL);
        assert_eq!(pace(REST_AFTER), POLL_RESTING);
        assert!(POLL_RESTING < SHARED_HOLD / 4, "a held fn is still seen well inside its hold");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn fn_alone_waits_for_a_hold() {
        let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        configure("ctrl+option", "fn");
        let mut t = ChordTracker::default();
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        // The Mac's double press: nothing for Pixie but two taps.
        assert_eq!(t.update(FLAG_FN, false, ms(0)), vec![]);
        assert_eq!(t.update(0, false, ms(90)), vec![HotkeyEvent::DictateTap]);
        assert_eq!(t.update(FLAG_FN, false, ms(180)), vec![]);
        assert_eq!(t.update(0, false, ms(260)), vec![HotkeyEvent::DictateTap]);
        // A real hold.
        assert_eq!(t.update(FLAG_FN, false, ms(1000)), vec![]);
        assert_eq!(t.update(FLAG_FN, false, ms(1200)), vec![]);
        assert_eq!(t.update(FLAG_FN, false, ms(1310)), vec![HotkeyEvent::DictateDown]);
        assert_eq!(t.update(FLAG_FN, false, ms(2000)), vec![]);
        assert_eq!(t.update(0, false, ms(2100)), vec![HotkeyEvent::DictateUp]);
        // fn alone as the talk keys: a quick press doesn't open the text box either.
        configure("fn", "fn+ctrl");
        let mut t = ChordTracker::default();
        assert_eq!(t.update(FLAG_FN, false, ms(0)), vec![]);
        assert_eq!(t.update(0, false, ms(100)), vec![]);
        assert_eq!(t.update(FLAG_FN, false, ms(500)), vec![]);
        assert_eq!(t.update(FLAG_FN, false, ms(820)), vec![HotkeyEvent::TalkDown]);
        assert!(matches!(t.update(0, false, ms(1500))[..], [HotkeyEvent::TalkUp { .. }]));
        configure("ctrl+option", "fn+ctrl");
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
