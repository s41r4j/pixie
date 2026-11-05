//! `AudioBus`: plays resource-slot sounds with one gain per category (`ui`, `agent`,
//! `ambient`) and mutual exclusion for long beds (BUILD-SPEC §8).
//!
//! The bus is pure logic over an [`AudioOutput`]; [`RodioOutput`] is the real speaker
//! backend, tests use a recording fake. A missing device or file never panics.

use openclicky_core::resources::{ResourceKind, ResourceStore, SoundCategory};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

pub type PlaybackId = u64;

/// Where sounds actually go.
pub trait AudioOutput: Send {
    fn play(&mut self, id: PlaybackId, path: &Path, gain: f32, looped: bool);
    fn set_gain(&mut self, id: PlaybackId, gain: f32);
    fn stop(&mut self, id: PlaybackId);
    fn stop_all(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Gains {
    pub master: f32,
    pub ui: f32,
    pub agent: f32,
    pub ambient: f32,
}

impl Default for Gains {
    fn default() -> Self {
        Self { master: 1.0, ui: 0.7, agent: 1.0, ambient: 0.5 }
    }
}

impl Gains {
    pub fn category(&self, c: SoundCategory) -> f32 {
        match c {
            SoundCategory::Ui => self.ui,
            SoundCategory::Agent => self.agent,
            SoundCategory::Ambient => self.ambient,
        }
    }
}

/// Per-sound choices from Settings → Sounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlotPref {
    /// 0…1.5, multiplies the slot's gain.
    pub volume: f32,
    pub muted: bool,
    /// A file of theirs to play instead (empty = the default sound).
    pub custom: String,
}

impl Default for SlotPref {
    fn default() -> Self {
        Self { volume: 1.0, muted: false, custom: String::new() }
    }
}

#[derive(Debug, Clone)]
struct Bed {
    id: PlaybackId,
    slot: String,
}

pub struct AudioBus<O: AudioOutput> {
    store: Arc<ResourceStore>,
    out: O,
    gains: Gains,
    muted: bool,
    next_id: PlaybackId,
    bed: Option<Bed>,
    prefs: std::collections::HashMap<String, SlotPref>,
}

fn clamp(g: f32) -> f32 {
    if g.is_finite() { g.clamp(0.0, 2.0) } else { 0.0 }
}

impl<O: AudioOutput> AudioBus<O> {
    pub fn new(store: Arc<ResourceStore>, out: O) -> Self {
        Self { store, out, gains: Gains::default(), muted: false, next_id: 1, bed: None, prefs: Default::default() }
    }

    pub fn gains(&self) -> Gains {
        self.gains
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Slot gain × category gain × master.
    fn effective_gain(&self, slot: &str) -> Option<f32> {
        let e = self.store.entry(ResourceKind::Sound, slot)?;
        let v = self.prefs.get(slot).map(|p| p.volume).unwrap_or(1.0);
        Some(clamp(e.gain * v * self.gains.category(e.category) * self.gains.master))
    }

    /// Play a sound slot. Beds loop and replace any other bed; replaying the bed that is
    /// already playing is a no-op. Returns `None` when muted or the slot has no file.
    pub fn play(&mut self, slot: &str) -> Option<PlaybackId> {
        if self.muted {
            return None;
        }
        let Some(entry) = self.store.entry(ResourceKind::Sound, slot) else {
            log::warn!("audio: unknown sound slot `{slot}`");
            return None;
        };
        let is_bed = entry.bed;
        if is_bed {
            if let Some(b) = &self.bed {
                if b.slot == slot {
                    return Some(b.id);
                }
            }
        }
        let pref = self.prefs.get(slot).cloned().unwrap_or_default();
        if pref.muted {
            return None;
        }
        let custom = Some(std::path::PathBuf::from(&pref.custom)).filter(|p| !pref.custom.is_empty() && p.is_file());
        let path = match custom {
            Some(p) => p,
            None => self.store.pick(ResourceKind::Sound, slot)?,
        };
        let gain = self.effective_gain(slot)?;
        if is_bed {
            self.stop_bed();
        }
        let id = self.next_id;
        self.next_id += 1;
        self.out.play(id, &path, gain, is_bed);
        if is_bed {
            self.bed = Some(Bed { id, slot: slot.to_string() });
        }
        Some(id)
    }

    pub fn current_bed(&self) -> Option<&str> {
        self.bed.as_ref().map(|b| b.slot.as_str())
    }

    pub fn stop_bed(&mut self) {
        if let Some(b) = self.bed.take() {
            self.out.stop(b.id);
        }
    }

    pub fn stop(&mut self, id: PlaybackId) {
        if self.bed.as_ref().is_some_and(|b| b.id == id) {
            self.bed = None;
        }
        self.out.stop(id);
    }

    pub fn stop_all(&mut self) {
        self.bed = None;
        self.out.stop_all();
    }

    /// Update gains; a playing bed follows the new level immediately.
    pub fn set_gains(&mut self, gains: Gains) {
        self.gains = Gains {
            master: clamp(gains.master),
            ui: clamp(gains.ui),
            agent: clamp(gains.agent),
            ambient: clamp(gains.ambient),
        };
        if let Some(b) = self.bed.clone() {
            if let Some(g) = self.effective_gain(&b.slot) {
                self.out.set_gain(b.id, g);
            }
        }
    }

    /// Replace the per-sound choices.
    pub fn set_prefs(&mut self, prefs: std::collections::HashMap<String, SlotPref>) {
        self.prefs = prefs;
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if muted {
            self.stop_all();
        }
    }
}

// ------------------------------------------------------------------ rodio backend

#[cfg(feature = "rodio")]
pub use rodio_out::RodioOutput;

#[cfg(feature = "rodio")]
mod rodio_out {
    use super::{AudioOutput, PlaybackId};
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{self, Receiver, Sender};

    enum Cmd {
        Play { id: PlaybackId, path: PathBuf, gain: f32, looped: bool },
        SetGain { id: PlaybackId, gain: f32 },
        Stop(PlaybackId),
        StopAll,
    }

    /// Speaker output on a dedicated thread (rodio's stream handle is not `Send`).
    pub struct RodioOutput {
        tx: Sender<Cmd>,
    }

    impl RodioOutput {
        pub fn spawn() -> Self {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("openclicky-audio".into())
                .spawn(move || run(rx))
                .expect("spawn audio thread");
            Self { tx }
        }

        fn send(&self, cmd: Cmd) {
            let _ = self.tx.send(cmd);
        }
    }

    fn run(rx: Receiver<Cmd>) {
        let stream = match rodio::OutputStream::try_default() {
            Ok(s) => Some(s),
            Err(e) => {
                log::error!("audio: no output device ({e}); sounds disabled");
                None
            }
        };
        let mut sinks: HashMap<PlaybackId, rodio::Sink> = HashMap::new();
        while let Ok(cmd) = rx.recv() {
            sinks.retain(|_, s| !s.empty());
            let Some((_stream, handle)) = stream.as_ref() else { continue };
            match cmd {
                Cmd::Play { id, path, gain, looped } => match start(handle, &path, gain, looped) {
                    Ok(sink) => {
                        sinks.insert(id, sink);
                    }
                    Err(e) => log::warn!("audio: cannot play {}: {e}", path.display()),
                },
                Cmd::SetGain { id, gain } => {
                    if let Some(s) = sinks.get(&id) {
                        s.set_volume(gain);
                    }
                }
                Cmd::Stop(id) => {
                    if let Some(s) = sinks.remove(&id) {
                        s.stop();
                    }
                }
                Cmd::StopAll => {
                    for (_, s) in sinks.drain() {
                        s.stop();
                    }
                }
            }
        }
    }

    fn start(
        handle: &rodio::OutputStreamHandle,
        path: &Path,
        gain: f32,
        looped: bool,
    ) -> Result<rodio::Sink, Box<dyn std::error::Error>> {
        use rodio::Source;
        let sink = rodio::Sink::try_new(handle)?;
        let decoder = rodio::Decoder::new(BufReader::new(File::open(path)?))?;
        sink.set_volume(gain);
        if looped {
            sink.append(decoder.repeat_infinite());
        } else {
            sink.append(decoder);
        }
        Ok(sink)
    }

    impl AudioOutput for RodioOutput {
        fn play(&mut self, id: PlaybackId, path: &Path, gain: f32, looped: bool) {
            self.send(Cmd::Play { id, path: path.to_path_buf(), gain, looped });
        }
        fn set_gain(&mut self, id: PlaybackId, gain: f32) {
            self.send(Cmd::SetGain { id, gain });
        }
        fn stop(&mut self, id: PlaybackId) {
            self.send(Cmd::Stop(id));
        }
        fn stop_all(&mut self) {
            self.send(Cmd::StopAll);
        }
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use openclicky_core::resources::Manifest;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Play { id: PlaybackId, file: String, gain: f32, looped: bool },
        Gain(PlaybackId, f32),
        Stop(PlaybackId),
        StopAll,
    }

    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<Vec<Call>>>);
    impl Fake {
        fn calls(&self) -> Vec<Call> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }
    impl AudioOutput for Fake {
        fn play(&mut self, id: PlaybackId, path: &Path, gain: f32, looped: bool) {
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            self.0.lock().unwrap().push(Call::Play { id, file, gain, looped });
        }
        fn set_gain(&mut self, id: PlaybackId, gain: f32) {
            self.0.lock().unwrap().push(Call::Gain(id, gain));
        }
        fn stop(&mut self, id: PlaybackId) {
            self.0.lock().unwrap().push(Call::Stop(id));
        }
        fn stop_all(&mut self) {
            self.0.lock().unwrap().push(Call::StopAll);
        }
    }

    fn bus() -> (tempfile::TempDir, AudioBus<Fake>, Fake) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sounds")).unwrap();
        for f in ["launch.wav", "bed-a.wav", "bed-b.wav"] {
            std::fs::write(dir.path().join("sounds").join(f), b"x").unwrap();
        }
        let manifest: Manifest = serde_json::from_str(
            r#"{"version":1,"sounds":{
                "agent-launch":{"files":["sounds/launch.wav"],"gain":0.5,"category":"agent"},
                "bed-a":{"files":["sounds/bed-a.wav"],"category":"ambient","bed":true},
                "bed-b":{"files":["sounds/bed-b.wav"],"category":"ambient","bed":true},
                "ghost":{"files":["sounds/ghost.wav"]}
            }}"#,
        )
        .unwrap();
        let store = Arc::new(ResourceStore::from_manifest(dir.path(), manifest).unwrap());
        let fake = Fake::default();
        (dir, AudioBus::new(store, fake.clone()), fake)
    }

    #[test]
    fn one_shot_uses_slot_category_and_master_gain() {
        let (_d, mut bus, out) = bus();
        bus.set_gains(Gains { master: 0.5, ui: 1.0, agent: 0.8, ambient: 1.0 });
        out.calls();
        let id = bus.play("agent-launch").unwrap();
        assert_eq!(
            out.calls(),
            vec![Call::Play { id, file: "launch.wav".into(), gain: 0.5 * 0.8 * 0.5, looped: false }]
        );
        assert_eq!(bus.current_bed(), None);
    }

    #[test]
    fn per_sound_volume_mute_and_custom_file() {
        let (d, mut bus, out) = bus();
        bus.set_gains(Gains { master: 1.0, ui: 1.0, agent: 1.0, ambient: 1.0 });
        let custom = d.path().join("mine.wav");
        std::fs::write(&custom, b"x").unwrap();
        let mut p = std::collections::HashMap::new();
        p.insert("agent-launch".to_string(), SlotPref { volume: 0.5, muted: false, custom: custom.display().to_string() });
        p.insert("bed-a".to_string(), SlotPref { muted: true, ..Default::default() });
        bus.set_prefs(p);
        out.calls();
        let id = bus.play("agent-launch").unwrap();
        assert_eq!(out.calls(), vec![Call::Play { id, file: "mine.wav".into(), gain: 0.25, looped: false }]);
        assert_eq!(bus.play("bed-a"), None);
    }

    #[test]
    fn beds_are_mutually_exclusive_and_loop() {
        let (_d, mut bus, out) = bus();
        let a = bus.play("bed-a").unwrap();
        assert!(matches!(out.calls()[..], [Call::Play { looped: true, .. }]));
        // same bed again: no restart
        assert_eq!(bus.play("bed-a"), Some(a));
        assert_eq!(out.calls(), vec![]);
        // different bed: old one stops first
        let b = bus.play("bed-b").unwrap();
        let calls = out.calls();
        assert_eq!(calls[0], Call::Stop(a));
        assert!(matches!(calls[1], Call::Play { id, looped: true, .. } if id == b));
        assert_eq!(bus.current_bed(), Some("bed-b"));
        // one-shots don't touch the bed
        bus.play("agent-launch");
        assert_eq!(bus.current_bed(), Some("bed-b"));
        bus.stop_bed();
        assert_eq!(out.calls().last(), Some(&Call::Stop(b)));
        assert_eq!(bus.current_bed(), None);
    }

    #[test]
    fn bed_follows_gain_changes() {
        let (_d, mut bus, out) = bus();
        let a = bus.play("bed-a").unwrap();
        out.calls();
        bus.set_gains(Gains { master: 1.0, ui: 1.0, agent: 1.0, ambient: 0.25 });
        assert_eq!(out.calls(), vec![Call::Gain(a, 0.25)]);
    }

    #[test]
    fn muted_unknown_and_missing_play_nothing() {
        let (_d, mut bus, out) = bus();
        assert_eq!(bus.play("nope"), None);
        assert_eq!(bus.play("ghost"), None);
        assert_eq!(out.calls(), vec![]);
        bus.play("bed-a");
        bus.set_muted(true);
        assert_eq!(out.calls().last(), Some(&Call::StopAll));
        assert_eq!(bus.current_bed(), None);
        assert_eq!(bus.play("agent-launch"), None);
        assert_eq!(out.calls(), vec![]);
    }

    #[test]
    fn gains_are_clamped() {
        let (_d, mut bus, _out) = bus();
        bus.set_gains(Gains { master: 9.0, ui: -1.0, agent: f32::NAN, ambient: 0.3 });
        let g = bus.gains();
        assert_eq!((g.master, g.ui, g.agent, g.ambient), (2.0, 0.0, 0.0, 0.3));
    }
}
