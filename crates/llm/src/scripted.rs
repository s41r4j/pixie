//! A provider that replays scripted responses, for tool-loop integration tests.

use crate::{Content, LlmError, OnEvent, Provider, Request, Response, StopReason, StreamEvent};
use std::sync::Mutex;

pub struct ScriptedProvider {
    script: Mutex<std::collections::VecDeque<Response>>,
    /// Every request seen, for assertions.
    pub seen: Mutex<Vec<Request>>,
}

impl ScriptedProvider {
    pub fn new(responses: Vec<Response>) -> Self {
        Self { script: Mutex::new(responses.into()), seen: Mutex::new(vec![]) }
    }

    /// Helper: a response with the given blocks; stop reason inferred from tool use.
    pub fn reply(content: Vec<Content>) -> Response {
        let stop = if content.iter().any(|c| matches!(c, Content::ToolUse { .. })) {
            StopReason::ToolUse
        } else {
            StopReason::EndTurn
        };
        Response { content, stop_reason: stop, usage: Default::default() }
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn label(&self) -> String {
        "Scripted".into()
    }
    fn supports_images(&self) -> bool {
        true
    }
    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        self.seen.lock().unwrap().push(req.clone());
        let r = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| LlmError::Decode("script exhausted".into()))?;
        for c in &r.content {
            match c {
                Content::Text { text } => on_event(StreamEvent::TextDelta(text.clone())),
                Content::ToolUse { id, name, input } => {
                    on_event(StreamEvent::ToolStart { id: id.clone(), name: name.clone() });
                    on_event(StreamEvent::ToolDone { id: id.clone(), name: name.clone(), input: input.clone() });
                }
                _ => {}
            }
        }
        Ok(r)
    }
}
