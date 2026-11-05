//! Token usage and cost, per model. Every model call is appended to `<app_data>/usage.jsonl`;
//! Settings → Usage summarises it. Prices are per million tokens and come, in order, from: an
//! override in settings, the live price list (OpenRouter's public model catalogue, which carries
//! Anthropic, OpenAI and DeepSeek prices; cached in `<app_data>/prices.json` and refreshed daily),
//! then a few built-in defaults.

use crate::config::{Price, Settings};
use openclicky_llm::{Effort, LlmError, OnEvent, Provider, ProviderConfig, Request, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    /// Unix millis.
    pub ts: i64,
    pub provider: String,
    pub model: String,
    /// "voice" or "job".
    pub kind: String,
    pub input: u64,
    pub output: u64,
}

/// Built-in prices (USD per million tokens) for models whose public price we know.
pub fn default_price(model: &str) -> Option<Price> {
    let p = |i: f64, o: f64| Some(Price { input: i, output: o });
    match model {
        "claude-opus-5-5" => p(4.0, 20.0),
        "claude-sonnet-5-5" => p(2.0, 10.0),
        "claude-haiku-5-5" => p(0.10, 0.50),
        "claude-fable-5-1" => p(10.0, 50.0),
        "claude-opus-5" => p(5.0, 25.0),
        "claude-sonnet-5" => p(2.0, 10.0),
        _ => None,
    }
}

pub fn price(settings: &Settings, model: &str) -> Option<Price> {
    settings
        .prices
        .get(model)
        .copied()
        .filter(|p| p.input > 0.0 || p.output > 0.0)
        .or_else(|| catalog().lock().ok().and_then(|c| c.lookup(model)).map(|e| Price { input: e.input, output: e.output }))
        .or_else(|| default_price(model))
}

// ---- live price list ------------------------------------------------------------------------

pub const CATALOG_URL: &str = "https://openrouter.ai/api/v1/models";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct CatalogEntry {
    /// USD per million tokens.
    pub input: f64,
    pub output: f64,
    pub context: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub fetched_at: i64,
    pub models: BTreeMap<String, CatalogEntry>,
    /// Official model id → its official name, when the id is an alias
    /// ("deepseek-flash" → "DeepSeek-V4.1-Flash"). Learned from the providers' model lists.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

/// "anthropic/claude-opus-5.5" / "claude-opus-5-5-20260101" → "claude-opus-5-5".
pub fn normalize(id: &str) -> String {
    let base = id.rsplit('/').next().unwrap_or(id).to_lowercase().replace(['.', '_', ' '], "-");
    let base = base.split(':').next().unwrap_or(&base).to_string();
    // Drop a trailing date: -20250514 or -2025-08-07.
    let parts: Vec<&str> = base.split('-').collect();
    let n = parts.len();
    if n > 1 && parts[n - 1].len() == 8 && parts[n - 1].chars().all(|c| c.is_ascii_digit()) {
        return parts[..n - 1].join("-");
    }
    if n > 3 && parts[n - 3].len() == 4 && [parts[n - 3], parts[n - 2], parts[n - 1]].iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return parts[..n - 3].join("-");
    }
    base
}

impl Catalog {
    pub fn lookup(&self, model: &str) -> Option<CatalogEntry> {
        self.models.get(&normalize(model)).copied().or_else(|| self.aliases.get(model).and_then(|n| self.models.get(&normalize(n)).copied()))
    }

    /// Parse OpenRouter's `/models` reply (prices there are USD per token, as strings).
    pub fn from_openrouter(v: &Value, now_ms: i64) -> Self {
        let mut models = BTreeMap::new();
        for m in v["data"].as_array().into_iter().flatten() {
            let Some(id) = m["id"].as_str() else { continue };
            // Skip other vendors and variants like ":batch" (discounted, not what we pay).
            if !["anthropic/", "openai/", "deepseek/"].iter().any(|p| id.starts_with(p)) || id.contains(':') {
                continue;
            }
            let per_m = |k: &str| m["pricing"][k].as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| m["pricing"][k].as_f64()).map(|x| x * 1e6);
            let (Some(input), Some(output)) = (per_m("prompt"), per_m("completion")) else { continue };
            if input < 0.0 || output < 0.0 {
                continue;
            }
            let e = CatalogEntry { input, output, context: m["context_length"].as_u64().unwrap_or(0) };
            // Keep the first (canonical) entry when variants normalise to the same name.
            models.entry(normalize(id)).or_insert(e);
        }
        Catalog { fetched_at: now_ms, models, aliases: BTreeMap::new() }
    }
}

pub fn catalog() -> &'static std::sync::Mutex<Catalog> {
    static C: std::sync::OnceLock<std::sync::Mutex<Catalog>> = std::sync::OnceLock::new();
    C.get_or_init(|| std::sync::Mutex::new(Catalog::default()))
}

/// Load the cached list from disk.
pub fn load_catalog(data: &Path) {
    if let Some(c) = std::fs::read_to_string(data.join("prices.json")).ok().and_then(|s| serde_json::from_str::<Catalog>(&s).ok()) {
        *catalog().lock().unwrap() = c;
    }
}

/// Fetch the live list and cache it. Returns how many models have prices.
pub async fn refresh_catalog(data: &Path) -> Result<usize, String> {
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let resp = http.get(CATALOG_URL).header("User-Agent", "Pixie (open source)").send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("the price list answered {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let c = Catalog::from_openrouter(&v, chrono::Utc::now().timestamp_millis());
    if c.models.is_empty() {
        return Err("the price list was empty".into());
    }
    let n = c.models.len();
    let mut c = c;
    c.aliases = catalog().lock().map(|o| o.aliases.clone()).unwrap_or_default();
    save_catalog(data, &c);
    *catalog().lock().unwrap() = c;
    Ok(n)
}

fn save_catalog(data: &Path, c: &Catalog) {
    let _ = std::fs::write(data.join("prices.json"), serde_json::to_string(c).unwrap_or_default());
}

/// Remember official names for alias ids (from a provider's model list).
pub fn learn_aliases(data: &Path, pairs: &[(String, String)]) {
    let mut c = catalog().lock().unwrap();
    let mut changed = false;
    for (id, name) in pairs {
        if !name.is_empty() && normalize(id) != normalize(name) && c.aliases.get(id) != Some(name) {
            c.aliases.insert(id.clone(), name.clone());
            changed = true;
        }
    }
    if changed {
        save_catalog(data, &c);
    }
}

/// Refresh when the cache is missing or older than a day.
pub async fn refresh_if_stale(data: &Path) {
    load_catalog(data);
    let age = chrono::Utc::now().timestamp_millis() - catalog().lock().map(|c| c.fetched_at).unwrap_or(0);
    if age > 86_400_000 {
        if let Err(e) = refresh_catalog(data).await {
            log::warn!("price list: {e}");
        }
    }
}

/// Wraps the active provider and records each call.
pub struct Recorded {
    inner: Arc<dyn Provider>,
    cfg: ProviderConfig,
    file: PathBuf,
}

impl Recorded {
    pub fn wrap(cfg: ProviderConfig, data: &Path) -> Arc<dyn Provider> {
        Arc::new(Self { inner: cfg.clone().build(), cfg, file: data.join("usage.jsonl") })
    }
}

#[async_trait::async_trait]
impl Provider for Recorded {
    fn label(&self) -> String {
        self.inner.label()
    }
    fn supports_images(&self) -> bool {
        self.inner.supports_images()
    }
    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        let r = self.inner.complete(req, on_event).await;
        if let Ok(resp) = &r {
            let line = Line {
                ts: chrono::Utc::now().timestamp_millis(),
                provider: self.cfg.kind.id().into(),
                model: req.model.clone().unwrap_or_else(|| self.cfg.model_for(req.effort).to_string()),
                kind: if req.effort == Effort::Fast { "voice".into() } else { "job".into() },
                input: resp.usage.input_tokens,
                output: resp.usage.output_tokens,
            };
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.file) {
                let _ = writeln!(f, "{}", serde_json::to_string(&line).unwrap_or_default());
            }
        }
        r
    }
}

pub fn read(data: &Path) -> Vec<Line> {
    std::fs::read_to_string(data.join("usage.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn line_cost(settings: &Settings, l: &Line) -> f64 {
    price(settings, &l.model).map(|p| (l.input as f64 * p.input + l.output as f64 * p.output) / 1e6).unwrap_or(0.0)
}

/// Filters for [`summary`]: `days` 0 = all time; provider "claude"/"openai"/"deepseek";
/// kind "voice"/"job".
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub days: i64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub kind: String,
}

/// Totals per model, per day, and overall, plus all-time totals (unfiltered).
#[cfg(test)]
pub fn summary(lines: &[Line], settings: &Settings, days: i64, now_ms: i64) -> Value {
    summary_with(lines, settings, &Filter { days, ..Default::default() }, now_ms)
}

pub fn summary_with(all: &[Line], settings: &Settings, f: &Filter, now_ms: i64) -> Value {
    let days = f.days;
    let since = if days > 0 { now_ms - days * 86_400_000 } else { i64::MIN };
    let lines: Vec<Line> = all
        .iter()
        .filter(|l| f.provider.is_empty() || l.provider == f.provider)
        .filter(|l| f.kind.is_empty() || l.kind == f.kind)
        .cloned()
        .collect();
    let lines = &lines[..];
    #[derive(Default)]
    struct Acc {
        provider: String,
        requests: u64,
        input: u64,
        output: u64,
        voice: u64,
        job: u64,
    }
    let mut by_model: BTreeMap<String, Acc> = BTreeMap::new();
    let mut by_day: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    for l in lines.iter().filter(|l| l.ts >= since) {
        let a = by_model.entry(l.model.clone()).or_default();
        a.provider = l.provider.clone();
        a.requests += 1;
        a.input += l.input;
        a.output += l.output;
        if l.kind == "voice" { a.voice += 1 } else { a.job += 1 }
        let day = chrono::DateTime::from_timestamp_millis(l.ts)
            .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        let cost = line_cost(settings, l);
        let d = by_day.entry(day).or_default();
        d.0 += l.input + l.output;
        d.1 += cost;
    }
    let mut total_cost = 0.0;
    let mut unpriced = false;
    let models: Vec<Value> = by_model
        .iter()
        .map(|(m, a)| {
            let p = price(settings, m);
            let cost = p.map(|p| (a.input as f64 * p.input + a.output as f64 * p.output) / 1e6);
            match cost {
                Some(c) => total_cost += c,
                None => unpriced = true,
            }
            json!({
                "model": m, "provider": a.provider, "requests": a.requests, "input": a.input, "output": a.output,
                "voice": a.voice, "job": a.job, "cost": cost, "price": p,
            })
        })
        .collect();
    let (ti, to, tr) = by_model.values().fold((0, 0, 0), |acc, a| (acc.0 + a.input, acc.1 + a.output, acc.2 + a.requests));
    let providers: std::collections::BTreeSet<&str> = all.iter().map(|l| l.provider.as_str()).collect();
    let first = all.iter().map(|l| l.ts).min();
    json!({
        "all_time": {
            "requests": all.len(),
            "tokens": all.iter().map(|l| l.input + l.output).sum::<u64>(),
            "cost": all.iter().map(|l| line_cost(settings, l)).sum::<f64>(),
            "since": first,
        },
        "providers": providers.into_iter().collect::<Vec<_>>(),
        "days": days,
        "requests": tr, "input": ti, "output": to,
        "cost": total_cost, "unpriced": unpriced,
        "models": models,
        "by_day": by_day.into_iter().map(|(d, (t, c))| json!({ "day": d, "tokens": t, "cost": c })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses_and_normalises() {
        assert_eq!(normalize("anthropic/claude-opus-5.5"), "claude-opus-5-5");
        assert_eq!(normalize("claude-sonnet-4-20250514"), "claude-sonnet-4");
        assert_eq!(normalize("gpt-5-2025-08-07"), "gpt-5");
        assert_eq!(normalize("openai/gpt-5-mini"), "gpt-5-mini");
        let v = json!({ "data": [
            { "id": "anthropic/claude-opus-5.5", "context_length": 200000, "pricing": { "prompt": "0.000004", "completion": "0.00002" } },
            { "id": "anthropic/claude-opus-5.5:batch", "pricing": { "prompt": "0.000002", "completion": "0.00001" } },
            { "id": "openai/gpt-5", "pricing": { "prompt": "0.00000125", "completion": "0.00001" } },
            { "id": "meta/llama", "pricing": { "prompt": "0.1", "completion": "0.1" } },
            { "id": "openai/gpt-x", "pricing": { "prompt": "-1", "completion": "-1" } }
        ] });
        let c = Catalog::from_openrouter(&v, 1);
        assert_eq!(c.models.len(), 2);
        let e = c.lookup("claude-opus-5-5-20260101").unwrap();
        assert!((e.input - 4.0).abs() < 1e-9 && (e.output - 20.0).abs() < 1e-9 && e.context == 200000);
        assert!((c.lookup("gpt-5").unwrap().input - 1.25).abs() < 1e-9);
        let mut c = c;
        c.aliases.insert("claude-best".into(), "Claude Opus 5.5".into());
        assert!((c.lookup("claude-best").unwrap().input - 4.0).abs() < 1e-9);
    }

    #[test]
    fn totals_and_costs() {
        let now = 1_800_000_000_000;
        let l = |m: &str, i, o, kind: &str| Line { ts: now - 1000, provider: "claude".into(), model: m.into(), kind: kind.into(), input: i, output: o };
        let lines = vec![l("claude-opus-5-5", 1_000_000, 100_000, "voice"), l("claude-opus-5-5", 0, 0, "job"), l("deepseek-flash", 500, 50, "voice")];
        let mut s = Settings::default();
        let v = summary(&lines, &s, 30, now);
        assert_eq!(v["requests"], 3);
        assert!((v["cost"].as_f64().unwrap() - 6.0).abs() < 1e-9, "4 + 0.1*20");
        assert_eq!(v["unpriced"], true);
        s.prices.insert("deepseek-flash".into(), Price { input: 1.0, output: 2.0 });
        let v = summary(&lines, &s, 30, now);
        assert_eq!(v["unpriced"], false);
        let old = Line { ts: now - 40 * 86_400_000, ..lines[0].clone() };
        assert_eq!(summary(&[old.clone()], &s, 30, now)["requests"], 0);
        assert_eq!(summary(&[old], &s, 0, now)["requests"], 1, "0 days = all time");
        let v = summary_with(&lines, &s, &Filter { days: 30, provider: "claude".into(), kind: "job".into() }, now);
        assert_eq!(v["requests"], 1);
        assert_eq!(v["all_time"]["requests"], 3);
    }
}

// ---- official model lists ---------------------------------------------------------------------

/// A provider's models from its official endpoint, newest first, with name, context window,
/// vision and price. Also records alias ids so usage can be priced.
pub async fn fetch_models(data: &Path, settings: &Settings, provider: &str) -> Result<Vec<Value>, String> {
    let key = crate::config::key_for(data, settings, provider).ok_or("No key for this provider yet.")?;
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().map_err(|e| e.to_string())?;
    let req = match provider {
        "claude" => http.get("https://api.anthropic.com/v1/models?limit=1000").header("x-api-key", &key).header("anthropic-version", "2023-06-01"),
        "openai" => http.get("https://api.openai.com/v1/models").bearer_auth(&key),
        "deepseek" => http.get("https://api.deepseek.com/models").bearer_auth(&key),
        _ => return Err("Unknown provider.".into()),
    };
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let code = resp.status();
        return Err(if code.as_u16() == 401 { format!("{provider} says the key isn't valid (401)") } else { format!("{provider} answered {code}") });
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let mut aliases = vec![];
    let cat = catalog().lock().map(|c| c.clone()).unwrap_or_default();
    let mut out: Vec<Value> = v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            if provider == "openai" {
                // Chat-capable models only.
                let chat = id.starts_with("gpt-") || id.starts_with('o') || id.starts_with("chatgpt");
                let other = ["audio", "realtime", "tts", "transcribe", "image", "search", "embedding", "moderation"].iter().any(|x| id.contains(x));
                if !chat || other {
                    return None;
                }
            }
            let name = m["display_name"].as_str().or(m["name"].as_str()).unwrap_or("").to_string();
            if !name.is_empty() {
                aliases.push((id.clone(), name.clone()));
            }
            // Newest first: Anthropic gives created_at (RFC 3339), OpenAI created (unix seconds).
            let created = m["created"].as_i64().map(|s| s * 1000).or_else(|| {
                m["created_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|d| d.timestamp_millis())
            });
            let live = cat.lookup(&id).or_else(|| (!name.is_empty()).then(|| cat.lookup(&name)).flatten());
            let context = m["context_window"].as_u64().or(m["max_input_tokens"].as_u64()).or(live.map(|e| e.context)).filter(|c| *c > 0);
            let vision = m["input_modalities"].as_array().map(|a| a.iter().any(|x| x == "image"));
            Some(json!({
                "id": id,
                "name": name,
                "vision": vision,
                "created": created,
                "price": live.map(|e| json!({ "input": e.input, "output": e.output })).or_else(|| default_price(&id).map(|p| json!(p))),
                "context": context,
            }))
        })
        .collect();
    learn_aliases(data, &aliases);
    out.sort_by(|a, b| b["created"].as_i64().unwrap_or(0).cmp(&a["created"].as_i64().unwrap_or(0)).then(a["id"].as_str().cmp(&b["id"].as_str())));
    Ok(out)
}

/// At startup: prices, then each keyed provider's list (teaches the aliases).
pub async fn warm(data: &Path, settings: &Settings) {
    refresh_if_stale(data).await;
    for p in ["claude", "openai", "deepseek"] {
        if crate::config::key_for(data, settings, p).is_some() {
            if let Err(e) = fetch_models(data, settings, p).await {
                log::warn!("models {p}: {e}");
            }
        }
    }
}
