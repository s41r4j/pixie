//! Token usage and cost, per model. Every model call is appended to `<app_data>/usage.jsonl`;
//! Settings → Usage summarises it. Prices are per million tokens: a few known defaults, and
//! whatever the user enters in Settings for the rest.

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
    settings.prices.get(model).copied().filter(|p| p.input > 0.0 || p.output > 0.0).or_else(|| default_price(model))
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

/// Totals per model, per day, and overall for the last `days` days.
pub fn summary(lines: &[Line], settings: &Settings, days: i64, now_ms: i64) -> Value {
    let since = now_ms - days * 86_400_000;
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
        let cost = price(settings, &l.model).map(|p| (l.input as f64 * p.input + l.output as f64 * p.output) / 1e6).unwrap_or(0.0);
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
    json!({
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
        assert_eq!(summary(&[old], &s, 30, now)["requests"], 0);
    }
}
