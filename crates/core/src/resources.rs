//! Resource slots (BUILD-SPEC §8).
//!
//! All media is addressed by **slot name**, never by path. `resources/manifest.json` maps
//! each slot to one or more files (variants), a gain and a playback category. Dropping a
//! replacement file in place (or editing the manifest) changes media without touching code.
//!
//! The store never panics on bad media: unknown slots and missing files resolve to `None`
//! and are reported by [`ResourceStore::missing`].

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Sound,
    Icon,
    Video,
    Font,
}

impl ResourceKind {
    pub const ALL: [ResourceKind; 4] =
        [ResourceKind::Sound, ResourceKind::Icon, ResourceKind::Video, ResourceKind::Font];

    pub fn as_str(self) -> &'static str {
        match self {
            ResourceKind::Sound => "sound",
            ResourceKind::Icon => "icon",
            ResourceKind::Video => "video",
            ResourceKind::Font => "font",
        }
    }

    /// Accepts both singular and plural (`sound`/`sounds`), as used in `slot://` URLs.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sound" | "sounds" => Some(Self::Sound),
            "icon" | "icons" => Some(Self::Icon),
            "video" | "videos" => Some(Self::Video),
            "font" | "fonts" => Some(Self::Font),
            _ => None,
        }
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Audio category; the bus applies one gain per category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SoundCategory {
    #[default]
    Ui,
    Agent,
    Ambient,
}

fn default_gain() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotEntry {
    /// Paths relative to the resources root. More than one = variants, picked at random.
    pub files: Vec<String>,
    #[serde(default = "default_gain")]
    pub gain: f32,
    #[serde(default)]
    pub category: SoundCategory,
    /// Long ambient bed: loops, and only one bed plays at a time.
    #[serde(default)]
    pub bed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub sounds: BTreeMap<String, SlotEntry>,
    #[serde(default)]
    pub icons: BTreeMap<String, SlotEntry>,
    #[serde(default)]
    pub videos: BTreeMap<String, SlotEntry>,
    #[serde(default)]
    pub fonts: BTreeMap<String, SlotEntry>,
}

impl Manifest {
    pub fn table(&self, kind: ResourceKind) -> &BTreeMap<String, SlotEntry> {
        match kind {
            ResourceKind::Sound => &self.sounds,
            ResourceKind::Icon => &self.icons,
            ResourceKind::Video => &self.videos,
            ResourceKind::Font => &self.fonts,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("cannot read manifest {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("invalid manifest {path}: {source}")]
    Parse { path: PathBuf, source: serde_json::Error },
    #[error("unsupported manifest version {0}")]
    Version(u32),
    #[error("{kind} slot `{slot}` has an unsafe path `{file}` (must be relative, inside resources/)")]
    UnsafePath { kind: ResourceKind, slot: String, file: String },
    #[error("{kind} slot `{slot}` lists no files")]
    Empty { kind: ResourceKind, slot: String },
}

/// One file the manifest names but the disk doesn't have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingFile {
    pub kind: ResourceKind,
    pub slot: String,
    pub file: String,
}

#[derive(Debug, Clone)]
pub struct ResourceStore {
    root: PathBuf,
    manifest: Manifest,
}

fn is_safe_relative(file: &str) -> bool {
    let p = Path::new(file);
    !file.is_empty()
        && p.components().all(|c| matches!(c, Component::Normal(_)))
}

impl ResourceStore {
    /// Load `<root>/manifest.json`.
    pub fn load(root: impl Into<PathBuf>) -> Result<Self, ResourceError> {
        let root = root.into();
        let path = root.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|source| ResourceError::Io { path: path.clone(), source })?;
        let manifest: Manifest = serde_json::from_str(&text)
            .map_err(|source| ResourceError::Parse { path: path.clone(), source })?;
        Self::from_manifest(root, manifest)
    }

    pub fn from_manifest(root: impl Into<PathBuf>, manifest: Manifest) -> Result<Self, ResourceError> {
        if manifest.version != 1 {
            return Err(ResourceError::Version(manifest.version));
        }
        for kind in ResourceKind::ALL {
            for (slot, entry) in manifest.table(kind) {
                if entry.files.is_empty() {
                    return Err(ResourceError::Empty { kind, slot: slot.clone() });
                }
                if let Some(bad) = entry.files.iter().find(|f| !is_safe_relative(f)) {
                    return Err(ResourceError::UnsafePath {
                        kind,
                        slot: slot.clone(),
                        file: bad.clone(),
                    });
                }
            }
        }
        Ok(Self { root: root.into(), manifest })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn entry(&self, kind: ResourceKind, slot: &str) -> Option<&SlotEntry> {
        self.manifest.table(kind).get(slot)
    }

    /// All variants of a slot that exist on disk.
    pub fn variants(&self, kind: ResourceKind, slot: &str) -> Vec<PathBuf> {
        self.entry(kind, slot)
            .map(|e| {
                e.files
                    .iter()
                    .map(|f| self.root.join(f))
                    .filter(|p| p.is_file())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// First existing variant (deterministic; use for icons, fonts, videos).
    pub fn resolve(&self, kind: ResourceKind, slot: &str) -> Option<PathBuf> {
        let found = self.variants(kind, slot).into_iter().next();
        if found.is_none() {
            log::warn!("resource slot {kind}/{slot} has no file on disk");
        }
        found
    }

    /// Random existing variant (use for sounds like `badge-pop-1..3`).
    pub fn pick(&self, kind: ResourceKind, slot: &str) -> Option<PathBuf> {
        let found = self.variants(kind, slot).choose(&mut rand::thread_rng()).cloned();
        if found.is_none() {
            log::warn!("resource slot {kind}/{slot} has no file on disk");
        }
        found
    }

    /// Every file the manifest names that isn't on disk.
    pub fn missing(&self) -> Vec<MissingFile> {
        let mut out = Vec::new();
        for kind in ResourceKind::ALL {
            for (slot, entry) in self.manifest.table(kind) {
                for file in &entry.files {
                    if !self.root.join(file).is_file() {
                        out.push(MissingFile { kind, slot: slot.clone(), file: file.clone() });
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn manifest(json: &str) -> Manifest {
        serde_json::from_str(json).unwrap()
    }

    fn store_with(json: &str, files: &[&str]) -> (tempfile::TempDir, ResourceStore) {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            let p = dir.path().join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"x").unwrap();
        }
        let store = ResourceStore::from_manifest(dir.path(), manifest(json)).unwrap();
        (dir, store)
    }

    const SAMPLE: &str = r#"{
        "version": 1,
        "sounds": {
            "agent-launch": { "files": ["sounds/agent-launch.wav"], "gain": 0.9, "category": "agent" },
            "badge-pop": { "files": ["sounds/badge-pop-1.wav", "sounds/badge-pop-2.wav", "sounds/badge-pop-3.wav"] },
            "pencil-paper-bed": { "files": ["sounds/pencil-paper-bed-1.wav"], "category": "ambient", "bed": true }
        },
        "icons": { "tray-template": { "files": ["icons/tray-template.png"] } }
    }"#;

    #[test]
    fn resolves_existing_slot_and_defaults() {
        let (_d, s) = store_with(SAMPLE, &["sounds/agent-launch.wav", "icons/tray-template.png"]);
        let e = s.entry(ResourceKind::Sound, "agent-launch").unwrap();
        assert_eq!(e.gain, 0.9);
        assert_eq!(e.category, SoundCategory::Agent);
        assert!(s.resolve(ResourceKind::Sound, "agent-launch").unwrap().ends_with("sounds/agent-launch.wav"));
        assert!(s.resolve(ResourceKind::Icon, "tray-template").is_some());
        let pop = s.entry(ResourceKind::Sound, "badge-pop").unwrap();
        assert_eq!(pop.gain, 1.0);
        assert_eq!(pop.category, SoundCategory::Ui);
        assert!(!pop.bed);
        assert!(s.entry(ResourceKind::Sound, "pencil-paper-bed").unwrap().bed);
    }

    #[test]
    fn unknown_slot_and_missing_file_are_none_not_panic() {
        let (_d, s) = store_with(SAMPLE, &[]);
        assert!(s.resolve(ResourceKind::Sound, "nope").is_none());
        assert!(s.pick(ResourceKind::Sound, "agent-launch").is_none());
        assert!(s.resolve(ResourceKind::Font, "barlow-regular").is_none());
    }

    #[test]
    fn pick_only_returns_existing_variants() {
        let (_d, s) = store_with(SAMPLE, &["sounds/badge-pop-2.wav"]);
        for _ in 0..20 {
            assert!(s.pick(ResourceKind::Sound, "badge-pop").unwrap().ends_with("badge-pop-2.wav"));
        }
        assert_eq!(s.variants(ResourceKind::Sound, "badge-pop").len(), 1);
    }

    #[test]
    fn reports_missing_files() {
        let (_d, s) = store_with(SAMPLE, &["sounds/agent-launch.wav", "sounds/badge-pop-1.wav"]);
        let missing: Vec<_> = s.missing().into_iter().map(|m| m.file).collect();
        assert_eq!(
            missing,
            vec![
                "sounds/badge-pop-2.wav",
                "sounds/badge-pop-3.wav",
                "sounds/pencil-paper-bed-1.wav",
                "icons/tray-template.png",
            ]
        );
    }

    #[test]
    fn rejects_paths_that_escape_root() {
        for bad in ["../secret.wav", "/etc/passwd", "sounds/../../x.wav", ""] {
            let json = format!(r#"{{"version":1,"sounds":{{"x":{{"files":["{bad}"]}}}}}}"#);
            let err = ResourceStore::from_manifest("/tmp", manifest(&json)).unwrap_err();
            assert!(matches!(err, ResourceError::UnsafePath { .. }), "{bad}: {err}");
        }
    }

    #[test]
    fn rejects_bad_version_and_empty_slot() {
        assert!(matches!(
            ResourceStore::from_manifest("/tmp", manifest(r#"{"version":2}"#)),
            Err(ResourceError::Version(2))
        ));
        assert!(matches!(
            ResourceStore::from_manifest("/tmp", manifest(r#"{"version":1,"icons":{"a":{"files":[]}}}"#)),
            Err(ResourceError::Empty { .. })
        ));
    }

    #[test]
    fn kind_parse_accepts_plural() {
        assert_eq!(ResourceKind::parse("sounds"), Some(ResourceKind::Sound));
        assert_eq!(ResourceKind::parse("font"), Some(ResourceKind::Font));
        assert_eq!(ResourceKind::parse("x"), None);
    }

    /// The shipped manifest must cover every slot in BUILD-SPEC §8 and every file must exist
    /// (placeholders are generated by `scripts/gen-placeholders.mjs`).
    #[test]
    fn shipped_manifest_covers_spec_and_files_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
        let store = ResourceStore::load(&root).expect("resources/manifest.json loads");

        let mut spec_sounds: Vec<String> = [
            "agent-launch", "agent-done", "agent-needs-you", "agent-close", "badge-pop",
            "clicky-question", "clicky-surprised", "clicky-text-open", "clicky-text-close",
            "clicky-text-send", "clicky-text-receive", "ding", "enter", "pen-flick",
            "highlighter", "pencil-paper-bed", "pen-screen-bed", "handwriting-bed", "eraser-bed",
            "paper-place", "photo-drop", "slide", "sticky-press", "stamp", "tada", "whoosh",
            "nope", "sparkle", "target-ping", "page-clear", "gel-pop", "type-keys",
            "reveal-boot", "hatching", "home-reveal", "boing", "build-3d",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        spec_sounds.sort();
        let shipped: Vec<String> = store.manifest().sounds.keys().cloned().collect();
        for s in &spec_sounds {
            assert!(shipped.contains(s), "manifest missing sound slot {s}");
        }
        for (kind, slot) in [
            (ResourceKind::Icon, "app-icon-1024"),
            (ResourceKind::Icon, "tray-template"),
            (ResourceKind::Icon, "ext-16"),
            (ResourceKind::Icon, "ext-128"),
            (ResourceKind::Video, "onboarding-intro"),
            (ResourceKind::Video, "paywall-intro"),
            (ResourceKind::Font, "barlow-regular"),
            (ResourceKind::Font, "barlow-medium"),
            (ResourceKind::Font, "barlow-semibold"),
            (ResourceKind::Font, "patrick-hand"),
        ] {
            assert!(store.entry(kind, slot).is_some(), "manifest missing {kind}/{slot}");
        }
        for bed in ["pencil-paper-bed", "pen-screen-bed", "handwriting-bed", "eraser-bed"] {
            assert!(store.entry(ResourceKind::Sound, bed).unwrap().bed, "{bed} must be a bed");
        }
        assert_eq!(store.missing(), vec![], "run `npm run placeholders` to generate missing media");
    }
}
