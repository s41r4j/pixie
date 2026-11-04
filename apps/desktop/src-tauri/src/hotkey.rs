//! Push-to-talk: hold **ctrl+option** to talk (BUILD-SPEC §1).
//!
//! Modifier-only chords can't be registered as global shortcuts, and event taps need
//! Accessibility/Input Monitoring. Polling `CGEventSourceFlagsState` needs no permission
//! and costs ~nothing at 50 Hz, so the hotkey works before the wizard is done.
//!
//! Panel behaviour:
//! - press   → panel opens in `listening` (sound on open)
//! - release → `idle` (M2 ends the turn here)
//! - a quick tap (<250 ms) while the panel was already open → close it
//! - Escape  → cancel + close

use crate::panel::{self, PanelMode};
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
    /// fn+control. TODO(M4): dictate clean text into the focused field.
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

fn talk_chord(flags: u64) -> bool {
    flags & FLAG_CONTROL != 0
        && flags & FLAG_OPTION != 0
        && flags & (FLAG_COMMAND | FLAG_SHIFT) == 0
}

fn dictate_chord(flags: u64) -> bool {
    flags & FLAG_FN != 0
        && flags & FLAG_CONTROL != 0
        && flags & (FLAG_OPTION | FLAG_COMMAND | FLAG_SHIFT) == 0
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

/// Spawn the polling thread (macOS only; other platforms have no hotkey yet).
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
            panel::set_mode(app, PanelMode::Listening);
        }
        HotkeyEvent::TalkUp { held } => {
            if held < TAP && *was_open_at_press {
                panel::hide(app);
            } else {
                // TODO(M2): end the turn → STT final → Claude → TTS.
                panel::set_mode(app, PanelMode::Idle);
            }
        }
        HotkeyEvent::Escape => {
            if panel::mode(app) != PanelMode::Hidden {
                crate::emit(app, openclicky_core::Event::new("hotkey.cancel", serde_json::json!({})));
                panel::hide(app);
            }
        }
        HotkeyEvent::DictateDown | HotkeyEvent::DictateUp => {} // TODO(M4)
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
    fn escape_fires_once_per_press() {
        let mut t = ChordTracker::default();
        let now = Instant::now();
        assert_eq!(t.update(0, true, now), vec![HotkeyEvent::Escape]);
        assert_eq!(t.update(0, true, now), vec![]);
        assert_eq!(t.update(0, false, now), vec![]);
        assert_eq!(t.update(0, true, now), vec![HotkeyEvent::Escape]);
    }
}
