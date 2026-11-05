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
    pub openai_key: String,
    /// Chosen models per provider id ("claude", "openai", "deepseek").
    pub models: std::collections::BTreeMap<String, ModelChoice>,
    /// Price per million tokens by model id (overrides the built-in table).
    pub prices: std::collections::BTreeMap<String, Price>,
    /// Settings → Coding.
    pub coding: openclicky_tools::coders::Prefs,
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
    /// Show the buddy pointer next to the cursor.
    pub buddy: bool,
    /// Let Pixie click/drag for you (otherwise it only points).
    pub can_click: bool,
    pub user_name: String,
    pub locale: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelChoice {
    /// Voice turns (fast).
    pub fast: String,
    /// Background jobs (deep).
    pub deep: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Price {
    /// USD per million input tokens.
    pub input: f64,
    /// USD per million output tokens.
    pub output: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            deepseek_key: String::new(),
            anthropic_key: String::new(),
            openai_key: String::new(),
            models: Default::default(),
            prices: Default::default(),
            coding: Default::default(),
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
            buddy: true,
            can_click: true,
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
    for f in env_files(app_data).iter().rev() {
        if let Ok(src) = std::fs::read_to_string(f) {
            out.extend(parse_dotenv(&src));
        }
    }
    out.extend(std::env::vars());
    // Settings typed by the user win over files.
    for (key, val) in [
        ("DEEPSEEK", &settings.deepseek_key),
        ("ANTHROPIC_API_KEY", &settings.anthropic_key),
        ("OPENAI_API_KEY", &settings.openai_key),
        ("OPENCLICKY_PROVIDER", &settings.provider),
    ] {
        if !val.trim().is_empty() {
            out.insert(key.into(), val.trim().to_string());
        }
    }
    for (provider, choice) in &settings.models {
        let prefix = match provider.as_str() {
            "claude" => "ANTHROPIC",
            "openai" => "OPENAI",
            "deepseek" => "DEEPSEEK",
            _ => continue,
        };
        if !choice.fast.trim().is_empty() {
            out.insert(format!("{prefix}_FAST_MODEL"), choice.fast.trim().into());
        }
        if !choice.deep.trim().is_empty() {
            out.insert(format!("{prefix}_DEEP_MODEL"), choice.deep.trim().into());
        }
    }
    out
}

/// The API key for one provider, wherever it came from.
pub fn key_for(app_data: &Path, settings: &Settings, provider: &str) -> Option<String> {
    let v = vars(app_data, settings);
    let get = |k: &str| v.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    match provider {
        "claude" => get("ANTHROPIC_API_KEY"),
        "openai" => get("OPENAI_API_KEY"),
        "deepseek" => get("DEEPSEEK").or_else(|| get("DEEPSEEK_API_KEY")),
        _ => None,
    }
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
