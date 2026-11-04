//! The job's final message is a report for the app (BUILD-SPEC §6):
//!
//! ```text
//! <body for the user, answer first>
//! DONE_TITLE: Desk Pick
//! NEXT_ACTIONS:
//! - Find a cheaper one
//! ARTIFACTS:
//! - /abs/path/output/desk.pdf
//! LEARNED:
//! - Prefers standing desks
//! ```
//!
//! Optional `DRAFT:` (free text) and `CHOICES:` (list). Block names are case-sensitive and
//! must start a line; anything before the first block is the body.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Report {
    pub body: String,
    pub done_title: Option<String>,
    pub next_actions: Vec<String>,
    pub artifacts: Vec<String>,
    pub learned: Vec<String>,
    pub choices: Vec<String>,
    pub draft: Option<String>,
}

const KEYS: [&str; 6] = ["DONE_TITLE", "NEXT_ACTIONS", "ARTIFACTS", "LEARNED", "CHOICES", "DRAFT"];

fn block_start(line: &str) -> Option<(&'static str, &str)> {
    let t = line.trim_start().trim_start_matches("**");
    KEYS.iter().find_map(|k| {
        let rest = t.strip_prefix(k)?;
        let rest = rest.trim_start_matches("**");
        rest.strip_prefix(':').map(|r| (*k, r.trim().trim_start_matches("**").trim()))
    })
}

fn bullet(line: &str) -> Option<String> {
    let t = line.trim();
    let item = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| {
        let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
        (d > 0).then(|| t[d..].strip_prefix(". ").or_else(|| t[d..].strip_prefix(") "))).flatten()
    })?;
    let item = item.trim().trim_matches('`').trim();
    (!item.is_empty() && item != "(none)" && !item.eq_ignore_ascii_case("none")).then(|| item.to_string())
}

pub fn parse(text: &str) -> Report {
    let mut r = Report::default();
    let mut body: Vec<&str> = vec![];
    let mut current: Option<&'static str> = None;
    let mut draft: Vec<&str> = vec![];
    for line in text.lines() {
        if let Some((key, rest)) = block_start(line) {
            current = Some(key);
            match key {
                "DONE_TITLE" => r.done_title = (!rest.is_empty()).then(|| rest.to_string()),
                "DRAFT" if !rest.is_empty() => draft.push(rest),
                _ if !rest.is_empty() => push(&mut r, key, rest.to_string()),
                _ => {}
            }
            continue;
        }
        match current {
            None => body.push(line),
            Some("DRAFT") => draft.push(line),
            Some("DONE_TITLE") => {
                if r.done_title.is_none() && !line.trim().is_empty() {
                    r.done_title = Some(line.trim().to_string());
                }
            }
            Some(key) => {
                if let Some(item) = bullet(line) {
                    push(&mut r, key, item);
                }
            }
        }
    }
    r.body = body.join("\n").trim().to_string();
    let d = draft.join("\n").trim().to_string();
    r.draft = (!d.is_empty()).then_some(d);
    r
}

fn push(r: &mut Report, key: &str, item: String) {
    match key {
        "NEXT_ACTIONS" => r.next_actions.push(item),
        "ARTIFACTS" => r.artifacts.push(item),
        "LEARNED" => r.learned.push(item),
        "CHOICES" => r.choices.push(item),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_report() {
        let r = parse(
            "The PDF is ready: one page on peptides.\nIt has 4 sources.\n\nDONE_TITLE: Peptides Report\nNEXT_ACTIONS:\n- Make a shorter version\n- Email it to Sam\nARTIFACTS:\n- /Users/me/jobs/1/output/peptides.pdf\n- https://example.com/x\nLEARNED:\n- Likes one-page summaries\n",
        );
        assert_eq!(r.body, "The PDF is ready: one page on peptides.\nIt has 4 sources.");
        assert_eq!(r.done_title.as_deref(), Some("Peptides Report"));
        assert_eq!(r.next_actions, vec!["Make a shorter version", "Email it to Sam"]);
        assert_eq!(r.artifacts.len(), 2);
        assert_eq!(r.learned, vec!["Likes one-page summaries"]);
    }

    #[test]
    fn missing_blocks_and_none_items() {
        let r = parse("Just an answer.");
        assert_eq!(r.body, "Just an answer.");
        assert!(r.done_title.is_none() && r.artifacts.is_empty());
        let r = parse("x\nARTIFACTS:\n- (none)\nNEXT_ACTIONS:\n1. Do a thing\n");
        assert!(r.artifacts.is_empty());
        assert_eq!(r.next_actions, vec!["Do a thing"]);
    }

    #[test]
    fn bold_keys_draft_and_choices() {
        let r = parse("Body\n**DONE_TITLE:** Reply Draft\nDRAFT:\nHi Sam,\nFriday works.\nCHOICES:\n- Send it\n- Make it shorter");
        assert_eq!(r.done_title.as_deref(), Some("Reply Draft"));
        // CHOICES starts a new block even after DRAFT.
        assert_eq!(r.choices, vec!["Send it", "Make it shorter"]);
        assert_eq!(r.draft.as_deref(), Some("Hi Sam,\nFriday works."));
    }
}
