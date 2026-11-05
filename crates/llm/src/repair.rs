//! Make a message history valid before it is sent.
//!
//! Every provider requires each assistant `tool_use` to be answered by a `tool_result` in the very
//! next (user) message, and rejects results that answer nothing. A job history can break that
//! rule: a run stopped mid-tool, a reply cut off by max_tokens, or a follow-up typed after an
//! unanswered call. [`repair`] fixes the shape without losing what was said:
//! - missing results get a short error result ("not run"),
//! - orphan results (no matching call just before) become plain text,
//! - consecutive messages with the same role are merged,
//! - empty messages are dropped, and the history starts with a user message.

use crate::{Content, Message, Role};
use std::collections::HashSet;

pub fn repair(messages: &[Message]) -> Vec<Message> {
    // 1. Drop empties and merge same-role neighbours.
    let mut merged: Vec<Message> = Vec::with_capacity(messages.len());
    for m in messages {
        if m.content.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(last) if last.role == m.role => {
                // Tool results must come first in a user message.
                if m.role == Role::User {
                    let (mut results, rest): (Vec<_>, Vec<_>) = last.content.drain(..).chain(m.content.iter().cloned()).partition(|c| matches!(c, Content::ToolResult { .. }));
                    results.extend(rest);
                    last.content = results;
                } else {
                    last.content.extend(m.content.iter().cloned());
                }
            }
            _ => merged.push(m.clone()),
        }
    }
    while merged.first().is_some_and(|m| m.role != Role::User) {
        merged.remove(0);
    }

    // 2. Pair tool calls with results.
    let mut out: Vec<Message> = Vec::with_capacity(merged.len() + 2);
    let mut i = 0;
    while i < merged.len() {
        let m = &merged[i];
        if m.role == Role::Assistant {
            let ids: Vec<String> = m
                .content
                .iter()
                .filter_map(|c| if let Content::ToolUse { id, .. } = c { Some(id.clone()) } else { None })
                .collect();
            out.push(m.clone());
            if ids.is_empty() {
                i += 1;
                continue;
            }
            let next = merged.get(i + 1).filter(|n| n.role == Role::User);
            let mut have: HashSet<String> = HashSet::new();
            let mut results = vec![];
            let mut rest = vec![];
            if let Some(n) = next {
                for c in &n.content {
                    match c {
                        Content::ToolResult { tool_use_id, .. } if ids.contains(tool_use_id) && !have.contains(tool_use_id) => {
                            have.insert(tool_use_id.clone());
                            results.push(c.clone());
                        }
                        Content::ToolResult { .. } => rest.push(orphan_text(c)),
                        other => rest.push(other.clone()),
                    }
                }
            }
            for id in &ids {
                if !have.contains(id) {
                    results.push(Content::tool_result(id.clone(), "Not run: the turn ended before this tool call was answered.", true));
                }
            }
            // Keep the calls' order.
            results.sort_by_key(|c| match c {
                Content::ToolResult { tool_use_id, .. } => ids.iter().position(|x| x == tool_use_id).unwrap_or(usize::MAX),
                _ => usize::MAX,
            });
            results.extend(rest);
            out.push(Message::user(results));
            i += if next.is_some() { 2 } else { 1 };
        } else {
            // A user message not right after a tool call: no results allowed.
            let content: Vec<Content> = m.content.iter().map(|c| if matches!(c, Content::ToolResult { .. }) { orphan_text(c) } else { c.clone() }).collect();
            match out.last_mut() {
                Some(last) if last.role == Role::User => last.content.extend(content),
                _ => out.push(Message::user(content)),
            }
            i += 1;
        }
    }
    out
}

fn orphan_text(c: &Content) -> Content {
    match c {
        Content::ToolResult { content, .. } => {
            let t = content.iter().filter_map(|x| if let Content::Text { text } = x { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n");
            Content::text(format!("(earlier tool output) {t}"))
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(id: &str) -> Content {
        Content::ToolUse { id: id.into(), name: "bash".into(), input: json!({}) }
    }

    fn valid(ms: &[Message]) -> bool {
        for (i, m) in ms.iter().enumerate() {
            if i > 0 && ms[i - 1].role == m.role {
                return false;
            }
            if m.role == Role::Assistant {
                let ids: Vec<_> = m.content.iter().filter_map(|c| if let Content::ToolUse { id, .. } = c { Some(id.clone()) } else { None }).collect();
                if !ids.is_empty() {
                    let Some(n) = ms.get(i + 1) else { return false };
                    for id in ids {
                        if !n.content.iter().any(|c| matches!(c, Content::ToolResult { tool_use_id, .. } if *tool_use_id == id)) {
                            return false;
                        }
                    }
                }
            }
        }
        ms.first().map(|m| m.role == Role::User).unwrap_or(true)
    }

    #[test]
    fn unanswered_call_then_follow_up() {
        // The 400 seen in a job: a call left unanswered, then the user's next message.
        let ms = vec![Message::user_text("go"), Message::assistant(vec![Content::text("ok"), call("a"), call("b")]), Message::user_text("now change X")];
        let r = repair(&ms);
        assert!(valid(&r));
        assert_eq!(r.len(), 3);
        assert!(matches!(&r[2].content[0], Content::ToolResult { tool_use_id, is_error: true, .. } if tool_use_id == "a"));
        assert!(matches!(&r[2].content[1], Content::ToolResult { tool_use_id, .. } if tool_use_id == "b"));
        assert_eq!(r[2].content[2], Content::text("now change X"));
    }

    #[test]
    fn trailing_call_and_partial_results() {
        let ms = vec![
            Message::user_text("go"),
            Message::assistant(vec![call("a"), call("b")]),
            Message::user(vec![Content::tool_result("b", "fine", false), Content::text("note")]),
            Message::assistant(vec![call("c")]),
        ];
        let r = repair(&ms);
        assert!(valid(&r));
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn orphans_merges_and_valid_input_unchanged() {
        let ms = vec![Message::user(vec![Content::tool_result("zz", "old", false)]), Message::user_text("hi"), Message::assistant(vec![Content::text("yo")])];
        let r = repair(&ms);
        assert!(valid(&r));
        assert_eq!(r.len(), 2);
        assert!(matches!(&r[0].content[0], Content::Text { .. }));
        let good = vec![Message::user_text("a"), Message::assistant(vec![call("x")]), Message::user(vec![Content::tool_result("x", "ok", false)]), Message::assistant(vec![Content::text("done")])];
        assert_eq!(repair(&good), good);
    }
}
