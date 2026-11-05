//! Settings (`<app_data>/settings.json`) and secrets from `.env` files.
//!
//! Keys are looked up in: the process environment → `<app_data>/.env` → the repo's `.env`
//! (dev builds) → keys typed into Settings. Keys never go into the repo or the bundle.

use openclicky_llm::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Typed into Settings; stored locally.
    pub deepseek_key: String,
    pub anthropic_key: String,
    /// "", "deepseek" or "claude".
    pub provider: String,
    pub voice_id: String,
    pub speech_rate: f64,
    /// "kokoro" (natural, local) or "apple".
    pub voice_engine: String,
    pub kokoro_voice: i32,
    pub voice_speed: f64,
    pub speak_answers: bool,
    pub share_screen: bool,
    pub master_volume: f32,
    pub muted: bool,
    pub onboarded: bool,
    pub user_name: String,
    pub locale: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            deepseek_key: String::new(),
            anthropic_key: String::new(),
            provider: String::new(),
            voice_id: String::new(),
            speech_rate: 0.52,
            voice_engine: "kokoro".into(),
            kokoro_voice: 3,
            voice_speed: 1.0,
            speak_answers: true,
            share_screen: true,
            master_volume: 1.0,
            muted: false,
            onboarded: false,
            user_name: String::new(),
            locale: String::new(),
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) {
        let _ = std::fs::create_dir_all(dir);
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), s);
        }
    }
}

/// Minimal `.env` parser: `KEY=value`, optional quotes, `#` comments, `export ` prefix.
pub fn parse_dotenv(src: &str) -> HashMap<String, String> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let l = l.strip_prefix("export ").unwrap_or(l);
            let (k, v) = l.split_once('=')?;
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(v);
            Some((k.trim().to_string(), v.to_string()))
        })
        .collect()
}

fn env_files(app_data: &Path) -> Vec<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."));
    vec![app_data.join(".env"), repo.join(".env"), repo.join(".env.local")]
}

/// All variables, earlier sources winning.
pub fn vars(app_data: &Path, settings: &Settings) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    if !settings.deepseek_key.trim().is_empty() {
        out.insert("DEEPSEEK".into(), settings.deepseek_key.clone());
    }
    if !settings.anthropic_key.trim().is_empty() {
        out.insert("ANTHROPIC_API_KEY".into(), settings.anthropic_key.clone());
    }
    if !settings.provider.is_empty() {
        out.insert("OPENCLICKY_PROVIDER".into(), settings.provider.clone());
    }
    for f in env_files(app_data).iter().rev() {
        if let Ok(src) = std::fs::read_to_string(f) {
            out.extend(parse_dotenv(&src));
        }
    }
    out.extend(std::env::vars());
    // Settings typed by the user win over files.
    if !settings.deepseek_key.trim().is_empty() {
        out.insert("DEEPSEEK".into(), settings.deepseek_key.clone());
    }
    if !settings.anthropic_key.trim().is_empty() {
        out.insert("ANTHROPIC_API_KEY".into(), settings.anthropic_key.clone());
    }
    if !settings.provider.is_empty() {
        out.insert("OPENCLICKY_PROVIDER".into(), settings.provider.clone());
    }
    out
}

pub fn provider_config(app_data: &Path, settings: &Settings) -> Option<ProviderConfig> {
    ProviderConfig::from_vars(&vars(app_data, settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenv() {
        let m = parse_dotenv("# c\nDEEPSEEK=sk-1\nexport A=\"b c\"\nB='x'\n\nbad line\n");
        assert_eq!(m["DEEPSEEK"], "sk-1");
        assert_eq!(m["A"], "b c");
        assert_eq!(m["B"], "x");
        assert_eq!(m.len(), 3);
    }
}
