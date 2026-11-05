//! OpenAI Chat Completions over raw HTTP + SSE (also works with OpenAI-compatible servers).

use crate::{
    Content, Effort, LlmError, Message, OnEvent, Provider, ProviderConfig, Request, Response, Role, StopReason, StreamEvent, Usage,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

pub struct OpenAiProvider {
    cfg: ProviderConfig,
    http: reqwest::Client,
}

fn content_text(c: &[Content]) -> String {
    c.iter()
        .filter_map(|b| match b {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Our Anthropic-shaped history → Chat Completions messages.
pub(crate) fn to_chat(system: &str, messages: &[Message]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in messages {
        match m.role {
            Role::Assistant => {
                let text = content_text(&m.content);
                let calls: Vec<Value> = m
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::ToolUse { id, name, input } => Some(json!({
                            "id": id, "type": "function",
                            "function": { "name": name, "arguments": input.to_string() }
                        })),
                        _ => None,
                    })
                    .collect();
                let mut msg = json!({ "role": "assistant", "content": if text.is_empty() { Value::Null } else { Value::String(text) } });
                if !calls.is_empty() {
                    msg["tool_calls"] = Value::Array(calls);
                }
                out.push(msg);
            }
            Role::User => {
                // Tool results become `tool` messages; images in them are described.
                let mut parts: Vec<Value> = vec![];
                for c in &m.content {
                    match c {
                        Content::ToolResult { tool_use_id, content, is_error } => {
                            let mut t = content_text(content);
                            if content.iter().any(|x| matches!(x, Content::Image { .. })) {
                                t.push_str("\n[an image was attached; it follows in the next message]");
                            }
                            if *is_error {
                                t = format!("ERROR: {t}");
                            }
                            out.push(json!({ "role": "tool", "tool_call_id": tool_use_id, "content": t }));
                            for x in content {
                                if let Content::Image { source } = x {
                                    parts.push(json!({ "type": "image_url", "image_url": { "url": format!("data:{};base64,{}", source.media_type, source.data) } }));
                                }
                            }
                        }
                        Content::Text { text } => parts.push(json!({ "type": "text", "text": text })),
                        Content::Image { source } => parts.push(json!({
                            "type": "image_url",
                            "image_url": { "url": format!("data:{};base64,{}", source.media_type, source.data) }
                        })),
                        _ => {}
                    }
                }
                if !parts.is_empty() {
                    out.push(json!({ "role": "user", "content": parts }));
                }
            }
        }
    }
    out
}

impl OpenAiProvider {
    pub fn new(cfg: ProviderConfig) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(600))
            .build()
            .expect("http client");
        Self { cfg, http }
    }

    pub fn body(&self, req: &Request) -> Value {
        let model = req.model.clone().unwrap_or_else(|| self.cfg.model_for(req.effort).to_string());
        let mut body = json!({
            "model": model,
            "messages": to_chat(&req.system, &req.messages),
            "stream": true,
            "stream_options": { "include_usage": true },
            "max_completion_tokens": req.max_tokens,
        });
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.input_schema } }))
                    .collect(),
            );
        }
        // Reasoning models take an effort hint; plain chat models reject it.
        let m = model.to_lowercase();
        if m.starts_with("gpt-5") || m.starts_with('o') {
            body["reasoning_effort"] = json!(match req.effort {
                Effort::Fast => "low",
                Effort::Balanced => "medium",
                Effort::Deep => "high",
            });
        }
        body
    }
}

#[async_trait::async_trait]
impl Provider for OpenAiProvider {
    fn label(&self) -> String {
        format!("OpenAI · {}", self.cfg.fast_model)
    }

    fn supports_images(&self) -> bool {
        true
    }

    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.cfg.api_key)
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
        let mut text = String::new();
        // index → (id, name, args)
        let mut calls: BTreeMap<u64, (String, String, String)> = BTreeMap::new();
        let mut stop = StopReason::EndTurn;
        let mut usage = Usage::default();
        let mut buf = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| LlmError::Network(e.to_string()))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buf.find('\n') {
                let line: String = buf.drain(..=pos).collect();
                let Some(data) = line.trim().strip_prefix("data:") else { continue };
                let data = data.trim();
                if data == "[DONE]" || data.is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(data).map_err(|e| LlmError::Decode(e.to_string()))?;
                if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                    usage.input_tokens = u["prompt_tokens"].as_u64().unwrap_or(0);
                    usage.output_tokens = u["completion_tokens"].as_u64().unwrap_or(0);
                }
                let Some(choice) = v["choices"].get(0) else { continue };
                let d = &choice["delta"];
                if let Some(t) = d["content"].as_str() {
                    if !t.is_empty() {
                        text.push_str(t);
                        on_event(StreamEvent::TextDelta(t.to_string()));
                    }
                }
                if let Some(tcs) = d["tool_calls"].as_array() {
                    for tc in tcs {
                        let i = tc["index"].as_u64().unwrap_or(0);
                        let e = calls.entry(i).or_default();
                        if let Some(id) = tc["id"].as_str() {
                            e.0 = id.to_string();
                        }
                        if let Some(n) = tc["function"]["name"].as_str() {
                            if e.1.is_empty() {
                                on_event(StreamEvent::ToolStart { id: e.0.clone(), name: n.to_string() });
                            }
                            e.1.push_str(n);
                        }
                        if let Some(a) = tc["function"]["arguments"].as_str() {
                            e.2.push_str(a);
                        }
                    }
                }
                match choice["finish_reason"].as_str() {
                    Some("tool_calls") => stop = StopReason::ToolUse,
                    Some("length") => stop = StopReason::MaxTokens,
                    Some("content_filter") => stop = StopReason::Refusal,
                    _ => {}
                }
            }
        }
        let mut content = vec![];
        if !text.is_empty() {
            content.push(Content::Text { text });
        }
        for (_, (id, name, args)) in calls {
            let input = if args.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&args).unwrap_or_else(|_| json!({ "__invalid_json": args }))
            };
            on_event(StreamEvent::ToolDone { id: id.clone(), name: name.clone(), input: input.clone() });
            content.push(Content::ToolUse { id, name, input });
        }
        if content.iter().any(|c| matches!(c, Content::ToolUse { .. })) {
            stop = StopReason::ToolUse;
        }
        Ok(Response { content, stop_reason: stop, usage })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolDef;

    #[test]
    fn history_maps_tool_calls_and_results() {
        let msgs = vec![
            Message::user_text("hi"),
            Message::assistant(vec![Content::text("ok"), Content::ToolUse { id: "c1".into(), name: "note".into(), input: json!({"a":1}) }]),
            Message::user(vec![Content::tool_result("c1", "done", false)]),
        ];
        let v = to_chat("sys", &msgs);
        assert_eq!(v[0]["role"], "system");
        assert_eq!(v[2]["tool_calls"][0]["function"]["arguments"], "{\"a\":1}");
        assert_eq!(v[3]["role"], "tool");
        assert_eq!(v[3]["tool_call_id"], "c1");
    }

    #[test]
    fn body_has_tools_and_effort_only_for_reasoning_models() {
        let mut cfg = ProviderConfig::openai("k".into());
        cfg.fast_model = "gpt-4.1-mini".into();
        let p = OpenAiProvider::new(cfg);
        let mut req = Request::new("s", vec![Message::user_text("x")]);
        req.tools = vec![ToolDef { name: "t".into(), description: "d".into(), input_schema: json!({"type":"object"}) }];
        let b = p.body(&req);
        assert_eq!(b["tools"][0]["function"]["name"], "t");
        assert!(b.get("reasoning_effort").is_none());
        req.model = Some("gpt-5".into());
        assert_eq!(p.body(&req)["reasoning_effort"], "low");
    }
}
