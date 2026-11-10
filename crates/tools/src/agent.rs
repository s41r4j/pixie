//! The tool loop: ask the model, run the tools it calls, feed results back, repeat.

use crate::{execute, ToolCtx};
use openclicky_llm::{Content, Effort, LlmError, Message, Provider, Request, Response, StopReason, StreamEvent, ToolDef};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AgentOptions {
    pub max_steps: usize,
    pub effort: Effort,
    pub max_tokens: u32,
    /// How many leading messages are earlier conversation (see `Request::history`).
    pub history: Option<usize>,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self { max_steps: 40, effort: Effort::Deep, max_tokens: 16_000, history: None }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// Streamed text from the model.
    Text(String),
    ToolStart { name: String, input: Value },
    ToolEnd { name: String, ok: bool, summary: String },
    /// A model response finished (one step).
    Step { n: usize },
}

#[derive(Debug, Clone)]
pub struct AgentOutcome {
    pub messages: Vec<Message>,
    /// Text of the last model message.
    pub final_text: String,
    pub steps: usize,
    pub hit_limit: bool,
}

fn summarize(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    if first.chars().count() > 120 {
        format!("{}…", first.chars().take(120).collect::<String>())
    } else {
        first.to_string()
    }
}

/// A single tool result longer than this is cut (the model can ask for a smaller range).
const RESULT_MAX: usize = 20_000;
/// From this step on, and every `STUB_EVERY` steps after, tool results older than the last
/// `KEEP_STEPS` steps become one-line stubs. Done in batches so the prompt prefix stays the same
/// between them and the provider's cache keeps matching.
const STUB_FROM: usize = 15;
const STUB_EVERY: usize = 10;
const KEEP_STEPS: usize = 6;

fn cap_result(text: String) -> String {
    if text.len() <= RESULT_MAX {
        return text;
    }
    let mut end = RESULT_MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… ({} more characters cut; ask for a smaller part if you need it)", &text[..end], text.len() - end)
}

/// Replace tool results older than the last `keep` model steps with short stubs.
fn stub_old_results(messages: &mut [Message], keep: usize) {
    let assistants: Vec<usize> = messages.iter().enumerate().filter(|(_, m)| m.role == openclicky_llm::Role::Assistant).map(|(i, _)| i).collect();
    if assistants.len() <= keep {
        return;
    }
    let cutoff = assistants[assistants.len() - keep];
    let mut names = std::collections::HashMap::new();
    for m in messages.iter() {
        for c in &m.content {
            if let Content::ToolUse { id, name, .. } = c {
                names.insert(id.clone(), name.clone());
            }
        }
    }
    for m in messages[..cutoff].iter_mut() {
        for c in m.content.iter_mut() {
            if let Content::ToolResult { tool_use_id, content, .. } = c {
                let text: String = content.iter().filter_map(|x| if let Content::Text { text } = x { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join(" ");
                let has_image = content.iter().any(|x| matches!(x, Content::Image { .. }));
                if text.len() <= 300 && !has_image || text.starts_with("[Earlier result") {
                    continue;
                }
                let first: String = text.lines().next().unwrap_or("").chars().take(120).collect();
                let name = names.get(tool_use_id.as_str()).map(String::as_str).unwrap_or("a tool");
                *content = vec![Content::text(format!(
                    "[Earlier result of {name}, trimmed to save space: {first}… ({} characters{}). Run it again if you need it.]",
                    text.len(),
                    if has_image { ", and an image" } else { "" }
                ))];
            }
        }
    }
}

/// Run until the model stops calling tools, the step limit is reached, or `ctx.cancel` fires.
pub async fn run(
    provider: &dyn Provider,
    system: &str,
    mut messages: Vec<Message>,
    tools: Vec<ToolDef>,
    ctx: &ToolCtx,
    opts: &AgentOptions,
    on_event: &mut (dyn FnMut(AgentEvent) + Send),
) -> Result<AgentOutcome, LlmError> {
    let mut step = 0;
    let mut hit_limit = false;
    loop {
        if ctx.cancel.is_cancelled() {
            return Err(LlmError::Cancelled);
        }
        step += 1;
        if step >= STUB_FROM && (step - STUB_FROM).is_multiple_of(STUB_EVERY) {
            stub_old_results(&mut messages, KEEP_STEPS);
        }
        let mut req = Request::new(system, messages.clone());
        req.tools = tools.clone();
        req.effort = opts.effort;
        req.max_tokens = opts.max_tokens;
        req.history = opts.history;
        let resp: Response = {
            let mut forward = |e: StreamEvent| {
                if let StreamEvent::TextDelta(t) = e {
                    on_event(AgentEvent::Text(t));
                }
            };
            tokio::select! {
                _ = ctx.cancel.cancelled() => return Err(LlmError::Cancelled),
                r = provider.complete(&req, &mut forward) => r?,
            }
        };
        on_event(AgentEvent::Step { n: step });
        let uses = resp.tool_uses();
        messages.push(Message::assistant(resp.content.clone()));

        // Any tool call gets answered, whatever the stop reason says (some providers report
        // end_turn alongside calls). Leaving one unanswered breaks the next request.
        if uses.is_empty() {
            if resp.stop_reason == StopReason::MaxTokens && step < opts.max_steps {
                messages.push(Message::user_text("Continue exactly where you stopped."));
                continue;
            }
            return Ok(AgentOutcome { final_text: resp.text(), messages, steps: step, hit_limit });
        }
        let truncated = resp.stop_reason == StopReason::MaxTokens;
        let n_uses = uses.len();

        let mut results = Vec::with_capacity(uses.len());
        for (k, (id, name, input)) in uses.into_iter().enumerate() {
            if ctx.cancel.is_cancelled() {
                return Err(LlmError::Cancelled);
            }
            if truncated && k == n_uses - 1 {
                // The reply was cut off while writing this call's input; don't run half a command.
                results.push(Content::tool_result(id, "Not run: your reply hit the length limit while writing this call. Send it again, smaller (split big files into several writes).", true));
                continue;
            }
            on_event(AgentEvent::ToolStart { name: name.clone(), input: input.clone() });
            let out = execute(ctx, &name, &input).await;
            on_event(AgentEvent::ToolEnd { name: name.clone(), ok: !out.is_error, summary: summarize(&out.text) });
            let mut content = vec![Content::text(if out.text.is_empty() { "done".to_string() } else { cap_result(out.text) })];
            for (media, data) in out.images {
                content.push(Content::Image {
                    source: openclicky_llm::ImageSource { kind: "base64".into(), media_type: media, data },
                });
            }
            results.push(Content::ToolResult { tool_use_id: id, content, is_error: out.is_error });
        }
        messages.push(Message::user(results));

        if step >= opts.max_steps {
            if hit_limit {
                // Already asked once to wrap up; stop here.
                let text = messages.iter().rev().find(|m| m.role == openclicky_llm::Role::Assistant).map(|m| m.text()).unwrap_or_default();
                return Ok(AgentOutcome { final_text: text, messages, steps: step, hit_limit });
            }
            hit_limit = true;
            if let Some(Message { content, .. }) = messages.last_mut() {
                content.push(Content::text("You've used your step budget. Don't call more tools: finish the note with what you have and write your final report now."));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CancellationToken, Host, ToolOutput};
    use openclicky_core::prompt::Mode;
    use openclicky_llm::ScriptedProvider;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingHost {
        calls: Mutex<Vec<(String, Value)>>,
        notes: Mutex<usize>,
    }

    #[async_trait::async_trait]
    impl Host for RecordingHost {
        async fn call(&self, _: &ToolCtx, name: &str, input: &Value) -> Option<ToolOutput> {
            self.calls.lock().unwrap().push((name.to_string(), input.clone()));
            match name {
                "update_plan" | "progress" | "say" => Some(ToolOutput::ok("ok")),
                _ => None,
            }
        }
        fn note_changed(&self, _: &ToolCtx) {
            *self.notes.lock().unwrap() += 1;
        }
    }

    fn tool(id: &str, name: &str, input: Value) -> Content {
        Content::ToolUse { id: id.into(), name: name.into(), input }
    }

    /// Integration: the loop against a fake provider replaying a scripted tool sequence.
    #[tokio::test]
    async fn scripted_job_runs_tools_and_reports() {
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(RecordingHost::default());
        let ctx = ToolCtx {
            mode: Mode::Job,
            cwd: dir.path().to_path_buf(),
            note_path: dir.path().join("note.md"),
            job_id: Some("j".into()),
            host: host.clone(),
            cancel: CancellationToken::new(),
            helper: None,
            coding: Default::default(),
        };
        let provider = ScriptedProvider::new(vec![
            ScriptedProvider::reply(vec![
                Content::text("Starting."),
                tool("1", "update_plan", json!({"plan":[{"label":"Write","status":"in_progress"},{"label":"Check","status":"pending"},{"label":"Report","status":"pending"}]})),
                tool("2", "note", json!({"markdown":"# Hello\n\n## Answer\n- [ ] Write file"})),
            ]),
            ScriptedProvider::reply(vec![
                tool("3", "write", json!({"path":"output/hello.txt","content":"hi"})),
                tool("4", "note_tick", json!({"label":"Write file"})),
                tool("5", "nope", json!({})),
            ]),
            ScriptedProvider::reply(vec![Content::text(format!(
                "Done.\nDONE_TITLE: Hello File\nNEXT_ACTIONS:\n- Make another\nARTIFACTS:\n- {}/output/hello.txt",
                dir.path().display()
            ))]),
        ]);
        let mut events = vec![];
        let out = run(&provider, "sys", vec![Message::user_text("make hello")], crate::defs::for_mode(Mode::Job, false), &ctx, &AgentOptions::default(), &mut |e| events.push(e))
            .await
            .unwrap();
        assert_eq!(out.steps, 3);
        let report = openclicky_core::report::parse(&out.final_text);
        assert_eq!(report.done_title.as_deref(), Some("Hello File"));
        assert_eq!(std::fs::read_to_string(dir.path().join("output/hello.txt")).unwrap(), "hi");
        assert!(std::fs::read_to_string(dir.path().join("note.md")).unwrap().contains("- [x] Write file"));
        assert_eq!(*host.notes.lock().unwrap(), 2);
        assert_eq!(host.calls.lock().unwrap()[0].0, "update_plan");
        // The unknown tool came back as an error result, and the loop carried on.
        let seen = provider.seen.lock().unwrap();
        let last_user = seen[2].messages.last().unwrap();
        assert!(last_user.content.iter().any(|c| matches!(c, Content::ToolResult { is_error: true, .. })));
        assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolEnd { name, ok: true, .. } if name == "write")));
    }

    #[tokio::test]
    async fn cancel_stops_the_loop() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            mode: Mode::Job,
            cwd: dir.path().into(),
            note_path: dir.path().join("note.md"),
            job_id: None,
            host: Arc::new(crate::NoHost),
            cancel: CancellationToken::new(),
            helper: None,
            coding: Default::default(),
        };
        ctx.cancel.cancel();
        let provider = ScriptedProvider::new(vec![]);
        let r = run(&provider, "s", vec![], vec![], &ctx, &AgentOptions::default(), &mut |_| {}).await;
        assert!(matches!(r, Err(LlmError::Cancelled)));
    }

    #[test]
    fn long_results_are_capped_and_old_ones_stubbed() {
        let big = "x".repeat(30_000);
        let c = cap_result(big);
        assert!(c.len() < 20_200 && c.contains("more characters cut"));
        let mut msgs = vec![Message::user_text("brief")];
        for i in 0..10 {
            msgs.push(Message::assistant(vec![tool(&format!("t{i}"), "read", json!({}))]));
            msgs.push(Message::user(vec![Content::tool_result(format!("t{i}"), format!("line one of {i}\n{}", "y".repeat(1000)), false)]));
        }
        stub_old_results(&mut msgs, 6);
        let text = |m: &Message| match &m.content[0] {
            Content::ToolResult { content, .. } => match &content[0] {
                Content::Text { text } => text.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        assert!(text(&msgs[2]).starts_with("[Earlier result of read") && text(&msgs[2]).contains("line one of 0"));
        assert!(text(&msgs[8]).starts_with("[Earlier result"), "the 4th of 10 is older than the last 6");
        assert!(!text(&msgs[10]).starts_with("[Earlier"), "the last 6 steps stay whole");
        assert_eq!(text(&msgs[1 + 2]), text(&msgs[3]));
        assert_eq!(msgs[0].text(), "brief");
    }
}