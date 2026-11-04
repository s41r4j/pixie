//! Which provider to use, from environment / settings.
//!
//! Order: explicit `OPENCLICKY_PROVIDER` → Claude when `ANTHROPIC_API_KEY` is set (the default
//! provider) → DeepSeek when `DEEPSEEK` / `DEEPSEEK_API_KEY` is set.

use crate::{AnthropicProvider, Effort, Provider};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Claude,
    DeepSeek,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    /// Model for voice turns (fast) and jobs (deep).
    pub fast_model: String,
    pub deep_model: String,
}

impl ProviderConfig {
    pub fn claude(api_key: String) -> Self {
        Self {
            kind: ProviderKind::Claude,
            base_url: "https://api.anthropic.com".into(),
            api_key,
            fast_model: "claude-opus-5-5".into(),
            deep_model: "claude-opus-5-5".into(),
        }
    }

    pub fn deepseek(api_key: String) -> Self {
        Self {
            kind: ProviderKind::DeepSeek,
            base_url: "https://api.deepseek.com/anthropic".into(),
            api_key,
            fast_model: "deepseek-flash".into(),
            deep_model: "deepseek-flash".into(),
        }
    }

    /// Resolve from a key/value map (process env merged with `.env` files and settings).
    pub fn from_vars(vars: &HashMap<String, String>) -> Option<Self> {
        let get = |k: &str| vars.get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let deepseek = get("DEEPSEEK").or_else(|| get("DEEPSEEK_API_KEY"));
        let claude = get("ANTHROPIC_API_KEY");
        let wanted = get("OPENCLICKY_PROVIDER").map(|s| s.to_lowercase());
        let mut cfg = match (wanted.as_deref(), claude, deepseek) {
            (Some("deepseek"), _, Some(k)) => Self::deepseek(k),
            (Some("claude" | "anthropic"), Some(k), _) => Self::claude(k),
            (_, Some(k), _) => Self::claude(k),
            (_, None, Some(k)) => Self::deepseek(k),
            _ => return None,
        };
        let prefix = match cfg.kind {
            ProviderKind::Claude => "ANTHROPIC",
            ProviderKind::DeepSeek => "DEEPSEEK",
        };
        if let Some(m) = get(&format!("{prefix}_MODEL")) {
            cfg.fast_model = m.clone();
            cfg.deep_model = m;
        }
        if let Some(m) = get(&format!("{prefix}_FAST_MODEL")) {
            cfg.fast_model = m;
        }
        if let Some(m) = get(&format!("{prefix}_DEEP_MODEL")) {
            cfg.deep_model = m;
        }
        if let Some(u) = get(&format!("{prefix}_BASE_URL")) {
            cfg.base_url = u;
        }
        Some(cfg)
    }

    pub fn model_for(&self, effort: Effort) -> &str {
        match effort {
            Effort::Fast => &self.fast_model,
            _ => &self.deep_model,
        }
    }

    pub fn build(self) -> Arc<dyn Provider> {
        Arc::new(AnthropicProvider::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn deepseek_from_all_caps_var() {
        let c = ProviderConfig::from_vars(&vars(&[("DEEPSEEK", "sk-1")])).unwrap();
        assert_eq!(c.kind, ProviderKind::DeepSeek);
        assert_eq!(c.api_key, "sk-1");
        assert!(c.base_url.ends_with("/anthropic"));
    }

    #[test]
    fn claude_is_default_when_both_present() {
        let c = ProviderConfig::from_vars(&vars(&[("DEEPSEEK", "a"), ("ANTHROPIC_API_KEY", "b")])).unwrap();
        assert_eq!(c.kind, ProviderKind::Claude);
        let c = ProviderConfig::from_vars(&vars(&[
            ("DEEPSEEK", "a"),
            ("ANTHROPIC_API_KEY", "b"),
            ("OPENCLICKY_PROVIDER", "deepseek"),
        ]))
        .unwrap();
        assert_eq!(c.kind, ProviderKind::DeepSeek);
    }

    #[test]
    fn none_without_keys_and_model_override() {
        assert!(ProviderConfig::from_vars(&vars(&[("DEEPSEEK", "  ")])).is_none());
        let c = ProviderConfig::from_vars(&vars(&[("DEEPSEEK", "k"), ("DEEPSEEK_DEEP_MODEL", "deepseek-v4-pro")])).unwrap();
        assert_eq!(c.model_for(Effort::Fast), "deepseek-flash");
        assert_eq!(c.model_for(Effort::Deep), "deepseek-v4-pro");
    }
}
