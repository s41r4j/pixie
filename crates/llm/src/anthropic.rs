//! Anthropic Messages API over raw HTTP + SSE (Rust has no official SDK).
//! Serves Claude and any Messages-compatible endpoint (DeepSeek's `/anthropic`).

use crate::{
    Content, Effort, LlmError, OnEvent, Provider, ProviderConfig, ProviderKind, Request, Response,
    StopReason, StreamEvent, Usage,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;

pub struct AnthropicProvider {
    cfg: ProviderConfig,
    http: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(cfg: ProviderConfig) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(600))
            .build()
            .expect("http client");
        Self { cfg, http }
    }

    /// Request body. Kept separate so tests can snapshot it.
    pub fn body(&self, req: &Request) -> Value {
        let model = req.model.clone().unwrap_or_else(|| self.cfg.model_for(req.effort).to_string());
        let mut body = json!({
            "model": model,
            "max_tokens": req.max_tokens,
            "system": req.system,
            "messages": req.messages,
            "stream": true,
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::to_value(&req.tools).unwrap_or(Value::Null);
        }
        match self.cfg.kind {
            // Claude Opus 5.5: thinking can't be disabled; effort is the control.
            ProviderKind::Claude => {
                let effort = match req.effort {
                    Effort::Fast => "low",
                    Effort::Balanced => "medium",
                    Effort::Deep => "high",
                };
                body["output_config"] = json!({ "effort": effort });
            }
            // DeepSeek: thinking off for voice latency, on for jobs.
            ProviderKind::DeepSeek => {
                if req.effort == Effort::Fast {
                    body["thinking"] = json!({ "type": "disabled" });
                }
            }
        }
        body
    }

    async fn send_once(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        let url = format!("{}/v1/messages", self.cfg.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(url)
            .header("x-api-key", &self.cfg.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&self.body(req))
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
                .unwrap_or(text);
            return Err(LlmError::Api { status: status.as_u16(), message });
        }

        let mut acc = Accumulator::default();
        let mut buf = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| LlmError::Network(e.to_string()))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buf.find("\n\n") {
                let frame: String = buf.drain(..pos + 2).collect();
                for line in frame.lines() {
                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        if data.is_empty() || data == "[DONE]" {
                            continue;
                        }
                        let v: Value = serde_json::from_str(data).map_err(|e| LlmError::Decode(e.to_string()))?;
                        acc.apply(&v, on_event)?;
                    }
                }
            }
        }
        Ok(acc.finish())
    }
}

#[async_trait::async_trait]
impl Provider for AnthropicProvider {
    fn label(&self) -> String {
        let name = match self.cfg.kind {
            ProviderKind::Claude => "Claude",
            ProviderKind::DeepSeek => "DeepSeek",
        };
        format!("{name} · {}", self.cfg.fast_model)
    }

    fn supports_images(&self) -> bool {
        // deepseek-v4-pro is text-only; flash and Claude take images.
        !self.cfg.fast_model.contains("v4-pro")
    }

    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        let mut attempt = 0;
        loop {
            let mut emitted = false;
            let mut wrapped = |e: StreamEvent| {
                emitted = true;
                on_event(e)
            };
            match self.send_once(req, &mut wrapped).await {
                Err(e) if e.retryable() && !emitted && attempt < 2 => {
                    attempt += 1;
                    log::warn!("llm: {e}; retry {attempt}");
                    tokio::time::sleep(Duration::from_millis(400 * 2u64.pow(attempt))).await;
                }
                other => return other,
            }
        }
    }
}

/// Builds the final message from SSE events.
#[derive(Default)]
pub(crate) struct Accumulator {
    blocks: Vec<Block>,
    stop: StopReason,
    usage: Usage,
}

enum Block {
    Text(String),
    Tool { id: String, name: String, json: String },
    Thinking { text: String, signature: String },
    Redacted(String),
    Other,
}

impl Accumulator {
    pub(crate) fn apply(&mut self, v: &Value, on_event: OnEvent<'_>) -> Result<(), LlmError> {
        match v["type"].as_str().unwrap_or("") {
            "message_start" => {
                if let Some(n) = v["message"]["usage"]["input_tokens"].as_u64() {
                    self.usage.input_tokens = n;
                }
            }
            "content_block_start" => {
                let b = &v["content_block"];
                let block = match b["type"].as_str().unwrap_or("") {
                    "text" => Block::Text(b["text"].as_str().unwrap_or("").to_string()),
                    "tool_use" => {
                        let id = b["id"].as_str().unwrap_or("").to_string();
                        let name = b["name"].as_str().unwrap_or("").to_string();
                        on_event(StreamEvent::ToolStart { id: id.clone(), name: name.clone() });
                        Block::Tool { id, name, json: String::new() }
                    }
                    "thinking" => {
                        on_event(StreamEvent::Thinking);
                        Block::Thinking {
                            text: b["thinking"].as_str().unwrap_or("").to_string(),
                            signature: b["signature"].as_str().unwrap_or("").to_string(),
                        }
                    }
                    "redacted_thinking" => Block::Redacted(b["data"].as_str().unwrap_or("").to_string()),
                    _ => Block::Other,
                };
                self.blocks.push(block);
            }
            "content_block_delta" => {
                let d = &v["delta"];
                let Some(block) = self.blocks.last_mut() else { return Ok(()) };
                match (d["type"].as_str().unwrap_or(""), block) {
                    ("text_delta", Block::Text(t)) => {
                        let s = d["text"].as_str().unwrap_or("");
                        t.push_str(s);
                        if !s.is_empty() {
                            on_event(StreamEvent::TextDelta(s.to_string()));
                        }
                    }
                    ("input_json_delta", Block::Tool { json, .. }) => {
                        json.push_str(d["partial_json"].as_str().unwrap_or(""));
                    }
                    ("thinking_delta", Block::Thinking { text, .. }) => {
                        text.push_str(d["thinking"].as_str().unwrap_or(""));
                    }
                    ("signature_delta", Block::Thinking { signature, .. }) => {
                        signature.push_str(d["signature"].as_str().unwrap_or(""));
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let Some(Block::Tool { id, name, json }) = self.blocks.last() {
                    let input = parse_input(json);
                    on_event(StreamEvent::ToolDone { id: id.clone(), name: name.clone(), input });
                }
            }
            "message_delta" => {
                if let Some(s) = v["delta"]["stop_reason"].as_str() {
                    self.stop = serde_json::from_value(Value::String(s.into())).unwrap_or(StopReason::Other);
                }
                if let Some(n) = v["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = n;
                }
            }
            "error" => {
                let message = v["error"]["message"].as_str().unwrap_or("stream error").to_string();
                let status = if v["error"]["type"] == "overloaded_error" { 529 } else { 500 };
                return Err(LlmError::Api { status, message });
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Response {
        let content = self
            .blocks
            .into_iter()
            .filter_map(|b| match b {
                Block::Text(text) if !text.is_empty() => Some(Content::Text { text }),
                Block::Tool { id, name, json } => Some(Content::ToolUse { id, name, input: parse_input(&json) }),
                Block::Thinking { text, signature } => Some(Content::Thinking { thinking: text, signature }),
                Block::Redacted(data) => Some(Content::RedactedThinking { data }),
                _ => None,
            })
            .collect();
        Response { content, stop_reason: self.stop, usage: self.usage }
    }
}

/// Tool input JSON; empty means `{}`; invalid JSON is kept as a marker the tool loop reports.
fn parse_input(json: &str) -> Value {
    if json.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(json).unwrap_or_else(|_| json!({ "__invalid_json": json }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Message, ToolDef};

    #[test]
    fn accumulates_text_and_tool_use() {
        let events = [
            json!({"type":"message_start","message":{"usage":{"input_tokens":12}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi "}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"there."}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"highlight","input":{}}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"x\":1"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":",\"y\":2}"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":30}}),
        ];
        let mut acc = Accumulator::default();
        let mut seen = vec![];
        let mut cb = |e: StreamEvent| seen.push(e);
        for e in &events {
            acc.apply(e, &mut cb).unwrap();
        }
        let r = acc.finish();
        assert_eq!(r.text(), "Hi there.");
        assert_eq!(r.stop_reason, StopReason::ToolUse);
        assert_eq!(r.tool_uses(), vec![("t1".into(), "highlight".into(), json!({"x":1,"y":2}))]);
        assert_eq!(r.usage, Usage { input_tokens: 12, output_tokens: 30 });
        assert!(seen.contains(&StreamEvent::TextDelta("Hi ".into())));
        assert!(seen.iter().any(|e| matches!(e, StreamEvent::ToolDone { name, .. } if name == "highlight")));
    }

    #[test]
    fn body_shape_per_provider() {
        let mut req = Request::new("sys", vec![Message::user_text("hi")]);
        req.tools = vec![ToolDef { name: "say".into(), description: "d".into(), input_schema: json!({"type":"object"}) }];
        let ds = AnthropicProvider::new(ProviderConfig::deepseek("k".into())).body(&req);
        assert_eq!(ds["thinking"], json!({"type":"disabled"}));
        assert_eq!(ds["model"], "deepseek-flash");
        assert_eq!(ds["tools"][0]["name"], "say");
        let cl = AnthropicProvider::new(ProviderConfig::claude("k".into())).body(&req);
        assert!(cl.get("thinking").is_none(), "Opus 5.5 rejects disabled thinking");
        assert_eq!(cl["output_config"]["effort"], "low");
        req.effort = Effort::Deep;
        let ds = AnthropicProvider::new(ProviderConfig::deepseek("k".into())).body(&req);
        assert!(ds.get("thinking").is_none());
    }
}
