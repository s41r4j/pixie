//! LLM provider trait + the Anthropic Messages implementation.
//!
//! Every model call in openclicky goes through [`Provider`]. The default implementation speaks
//! the Anthropic Messages API, which both Claude (`api.anthropic.com`) and DeepSeek
//! (`api.deepseek.com/anthropic`) serve, so one client covers both; [`ProviderConfig::from_env`]
//! picks which. [`ScriptedProvider`] replays canned responses for tests.

mod anthropic;
mod config;
mod scripted;

pub use anthropic::AnthropicProvider;
pub use config::{ProviderConfig, ProviderKind};
pub use scripted::ScriptedProvider;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One content block, in Messages API wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<Content>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
    /// Passed back unchanged on later requests (required by thinking models during tool use).
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub kind: String,
    pub media_type: String,
    pub data: String,
}

impl Content {
    pub fn text(t: impl Into<String>) -> Self {
        Content::Text { text: t.into() }
    }

    pub fn jpeg_base64(data: String) -> Self {
        Content::Image {
            source: ImageSource { kind: "base64".into(), media_type: "image/jpeg".into(), data },
        }
    }

    pub fn tool_result(id: impl Into<String>, text: impl Into<String>, is_error: bool) -> Self {
        Content::ToolResult { tool_use_id: id.into(), content: vec![Content::text(text)], is_error }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
}

impl Message {
    pub fn user(content: Vec<Content>) -> Self {
        Self { role: Role::User, content }
    }
    pub fn user_text(t: impl Into<String>) -> Self {
        Self::user(vec![Content::text(t)])
    }
    pub fn assistant(content: Vec<Content>) -> Self {
        Self { role: Role::Assistant, content }
    }

    /// Concatenated text blocks.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| if let Content::Text { text } = c { Some(text.as_str()) } else { None })
            .collect::<Vec<_>>()
            .join("")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// How hard the model should think. Voice turns use `Fast`; jobs use `Deep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Effort {
    #[default]
    Fast,
    Balanced,
    Deep,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: u32,
    pub effort: Effort,
    /// Override the provider's default model.
    pub model: Option<String>,
}

impl Request {
    pub fn new(system: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            system: system.into(),
            messages,
            tools: vec![],
            max_tokens: 4096,
            effort: Effort::Fast,
            model: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    #[default]
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    Refusal,
    PauseTurn,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Response {
    pub content: Vec<Content>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

impl Response {
    pub fn text(&self) -> String {
        Message::assistant(self.content.clone()).text()
    }
    pub fn tool_uses(&self) -> Vec<(String, String, Value)> {
        self.content
            .iter()
            .filter_map(|c| match c {
                Content::ToolUse { id, name, input } => Some((id.clone(), name.clone(), input.clone())),
                _ => None,
            })
            .collect()
    }
}

/// Incremental output while a response streams.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    TextDelta(String),
    /// A tool call started; its input arrives with the final response.
    ToolStart { id: String, name: String },
    ToolDone { id: String, name: String, input: Value },
    Thinking,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("no model provider is configured: add DEEPSEEK or ANTHROPIC_API_KEY to .env, or set a key in Settings")]
    NotConfigured,
    #[error("couldn't reach the model: {0}")]
    Network(String),
    #[error("the model service said {status}: {message}")]
    Api { status: u16, message: String },
    #[error("unexpected reply from the model: {0}")]
    Decode(String),
    #[error("cancelled")]
    Cancelled,
}

impl LlmError {
    pub fn retryable(&self) -> bool {
        matches!(self, LlmError::Network(_)) || matches!(self, LlmError::Api { status, .. } if *status == 429 || *status >= 500)
    }
}

pub type OnEvent<'a> = &'a mut (dyn FnMut(StreamEvent) + Send);

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Human name, e.g. "DeepSeek · deepseek-flash".
    fn label(&self) -> String;
    fn supports_images(&self) -> bool;
    /// Stream one response. `on_event` sees deltas as they arrive.
    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError>;
}
