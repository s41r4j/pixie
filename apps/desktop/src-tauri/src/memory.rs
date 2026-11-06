//! Long-term memory: short notes about the user that last across days.
//!
//! Notes live in `<app_data>/memory.json` (local only, editable in Settings → Memory). They come
//! from three places: "remember that…" (the `memory` tool, source "said"), a background pass that
//! reads a chat once it has gone quiet (source "learned", only when memory is on "auto"), and the
//! `LEARNED` lines at the end of jobs (source "job"). The most useful ~15 go into every prompt;
//! the rest are found with the tool's search. Anything that looks like a password or key is
//! refused. Settings → Memory → Off stops both reading and learning.

use crate::AppState;
use openclicky_llm::{Effort, Message, Request, ToolDef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub const KINDS: &[&str] = &["about", "preference", "person", "project", "howto", "other"];
/// Notes that always go into the prompt.
const CORE: usize = 15;
const MAX_NOTES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub id: String,
    pub text: String,
    #[serde(default = "other")]
    pub kind: String,
    /// "said" (they asked), "learned" (from a chat), "job", "you" (typed in Settings).
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub uses: u32,
    #[serde(default)]
    pub last_used: i64,
}

fn other() -> String {
    "other".into()
}

static LOCK: Mutex<()> = Mutex::new(());

fn path(data: &Path) -> PathBuf {
    data.join("memory.json")
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn new_id() -> String {
    format!("m{:x}{:04x}", now(), rand::random::<u16>())
}

/// "auto" (learns from chats), "manual" (only "remember…") or "off".
pub fn mode(app: &AppHandle) -> String {
    let m = app.state::<AppState>().settings.lock().map(|s| s.memory.clone()).unwrap_or_default();
    if matches!(m.as_str(), "manual" | "off") {
        m
    } else {
        "auto".into()
    }
}

pub fn load(data: &Path) -> Vec<Note> {
    if let Some(v) = std::fs::read_to_string(path(data)).ok().and_then(|s| serde_json::from_str::<Vec<Note>>(&s).ok()) {
        return v;
    }
    // First run with notes: bring over the old memory.md lines (from jobs).
    let old = std::fs::read_to_string(data.join("memory.md")).unwrap_or_default();
    let t = now();
    let notes: Vec<Note> = old
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- "))
        .map(|l| strip_date(l).to_string())
        .filter(|l| !l.is_empty())
        .map(|text| Note { id: new_id(), text, kind: "other".into(), source: "job".into(), created: t, updated: t, uses: 0, last_used: 0 })
        .collect();
    if !notes.is_empty() {
        save(data, &notes);
    }
    notes
}

/// "fact (2026-11-05)" → "fact".
fn strip_date(l: &str) -> &str {
    let l = l.trim();
    match l.rfind(" (") {
        Some(i) if l.ends_with(')') && l[i + 2..l.len() - 1].chars().all(|c| c.is_ascii_digit() || c == '-') => &l[..i],
        _ => l,
    }
}

fn save(data: &Path, notes: &[Note]) {
    let _ = std::fs::create_dir_all(data);
    if let Ok(s) = serde_json::to_string_pretty(notes) {
        let tmp = path(data).with_extension("json.tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(tmp, path(data));
        }
    }
}

fn words(s: &str) -> HashSet<String> {
    const STOP: &[&str] = &["the", "a", "an", "is", "are", "and", "or", "to", "of", "in", "on", "for", "they", "their", "them", "with", "at", "my", "i", "me", "it", "be", "that", "this", "user", "s"];
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP.contains(w))
        .map(str::to_string)
        .collect()
}

/// Same fact, said differently?
fn similar(a: &str, b: &str) -> bool {
    let (wa, wb) = (words(a), words(b));
    if wa.is_empty() || wb.is_empty() {
        return false;
    }
    let inter = wa.intersection(&wb).count() as f64;
    let union = wa.union(&wb).count() as f64;
    inter / union >= 0.7
}

/// Passwords, keys, tokens, card numbers: never kept.
pub fn looks_secret(text: &str) -> bool {
    let l = text.to_lowercase();
    if ["password", "passcode", "api key", "apikey", "secret key", "private key", "token is", "pin is", "pin code", "cvv", "seed phrase", "recovery phrase"].iter().any(|k| l.contains(k)) {
        return true;
    }
    text.split_whitespace().any(|w| {
        let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        let digits = w.chars().filter(char::is_ascii_digit).count();
        w.starts_with("sk-") || w.starts_with("ghp_") || w.starts_with("xox") || w.starts_with("AKIA") || (w.len() >= 24 && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && digits >= 4) || (digits >= 13 && digits == w.len())
    })
}

fn clean_kind(k: &str) -> String {
    let k = k.trim().to_lowercase();
    if KINDS.contains(&k.as_str()) {
        k
    } else {
        "other".into()
    }
}

/// Add a note, or refresh the one that already says it. Returns (note, was it new).
pub fn add(data: &Path, text: &str, kind: &str, source: &str) -> Result<(Note, bool), String> {
    let text = text.trim().trim_start_matches("- ").trim();
    if text.is_empty() {
        return Err("Nothing to remember.".into());
    }
    if text.chars().count() > 300 {
        return Err("Keep a note under 300 characters; split it into a few.".into());
    }
    if looks_secret(text) {
        return Err("That looks like a password or key; I don't keep those.".into());
    }
    let _g = LOCK.lock().unwrap();
    let mut notes = load(data);
    let t = now();
    if let Some(n) = notes.iter_mut().find(|n| n.text.eq_ignore_ascii_case(text) || similar(&n.text, text)) {
        n.text = text.to_string();
        n.updated = t;
        if source == "said" {
            n.source = "said".into();
        }
        let n = n.clone();
        save(data, &notes);
        return Ok((n, false));
    }
    let n = Note { id: new_id(), text: text.to_string(), kind: clean_kind(kind), source: source.into(), created: t, updated: t, uses: 0, last_used: 0 };
    notes.push(n.clone());
    if notes.len() > MAX_NOTES {
        // Drop the least useful learned notes first; never what they asked to keep.
        notes.sort_by(|a, b| score(b, t).total_cmp(&score(a, t)));
        let mut keep = 0;
        notes.retain(|n| {
            keep += 1;
            keep <= MAX_NOTES || n.source == "said" || n.source == "you"
        });
    }
    save(data, &notes);
    Ok((n, true))
}

pub fn update(data: &Path, id: &str, text: &str, kind: Option<&str>) -> Result<Note, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("A note can't be empty.".into());
    }
    if looks_secret(text) {
        return Err("That looks like a password or key; I don't keep those.".into());
    }
    let _g = LOCK.lock().unwrap();
    let mut notes = load(data);
    let n = notes.iter_mut().find(|n| n.id == id).ok_or("No such note.")?;
    n.text = text.to_string();
    if let Some(k) = kind {
        n.kind = clean_kind(k);
    }
    n.updated = now();
    let n = n.clone();
    save(data, &notes);
    Ok(n)
}

pub fn remove(data: &Path, id: &str) -> bool {
    let _g = LOCK.lock().unwrap();
    let mut notes = load(data);
    let before = notes.len();
    notes.retain(|n| n.id != id);
    let gone = notes.len() != before;
    if gone {
        save(data, &notes);
    }
    gone
}

pub fn clear(data: &Path) {
    let _g = LOCK.lock().unwrap();
    save(data, &[]);
    let _ = std::fs::remove_file(data.join("memory.md"));
}

/// How useful a note is: what they asked to keep first, then what's used, then what's recent.
fn score(n: &Note, t: i64) -> f64 {
    let origin = match n.source.as_str() {
        "said" | "you" => 3.0,
        "job" => 1.0,
        _ => 0.5,
    };
    let kind = if matches!(n.kind.as_str(), "about" | "preference") { 1.0 } else { 0.0 };
    let age_days = ((t - n.updated.max(n.last_used)) as f64 / 86_400_000.0).max(0.0);
    origin + kind + (n.uses as f64 + 1.0).ln() + 1.0 / (1.0 + age_days / 14.0)
}

/// Best matches for a query (and they count as used).
pub fn search(data: &Path, query: &str, limit: usize) -> Vec<Note> {
    let q = words(query);
    let _g = LOCK.lock().unwrap();
    let mut notes = load(data);
    let t = now();
    let mut hits: Vec<(f64, usize)> = notes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let w = words(&format!("{} {}", n.text, n.kind));
            let overlap = q.intersection(&w).count();
            let partial = q.iter().filter(|qw| qw.len() >= 4 && w.iter().any(|nw| nw.starts_with(qw.as_str()) || qw.starts_with(nw.as_str()))).count();
            let m = overlap as f64 + 0.5 * partial as f64;
            (m > 0.0).then(|| (m + score(n, t) * 0.1, i))
        })
        .collect();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0));
    let picked: Vec<usize> = hits.into_iter().take(limit).map(|(_, i)| i).collect();
    for &i in &picked {
        notes[i].uses += 1;
        notes[i].last_used = t;
    }
    let out = picked.iter().map(|&i| notes[i].clone()).collect();
    save(data, &notes);
    out
}

/// What goes into every prompt.
pub fn core(data: &Path) -> Vec<Note> {
    let mut notes = load(data);
    let t = now();
    notes.sort_by(|a, b| score(b, t).total_cmp(&score(a, t)));
    notes.truncate(CORE);
    notes
}

/// The prompt's "what you already know about them" lines (none when memory is off).
pub fn prompt_lines(app: &AppHandle) -> Vec<String> {
    if mode(app) == "off" {
        return vec![];
    }
    let data = app.state::<AppState>().data_dir.clone();
    let all = load(&data).len();
    let mut lines: Vec<String> = core(&data).into_iter().map(|n| n.text).collect();
    if all > lines.len() {
        lines.push(format!("({} more notes: memory action=search finds them)", all - lines.len()));
    }
    lines
}

// ---- the `memory` tool ----

pub fn tool() -> ToolDef {
    ToolDef {
        name: "memory".into(),
        description: "Your long-term memory of them: short notes kept across days, on their computer. The most useful ones are already in your prompt.\n\
action=remember text=<one short fact in third person, e.g. \"Prefers short answers\", \"Their manager is Priya\"> kind=about|preference|person|project|howto|other: when they say remember/note that, or tell you something lasting about themselves, their people, projects or setup. Never passwords, keys or card numbers.\n\
action=forget text=<what to forget> (or id): when they ask you to forget something. action=search query=<words>: look up what you know (people, projects, how they like things). action=list: everything.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["remember", "forget", "search", "list"] },
                "text": { "type": "string" },
                "kind": { "type": "string", "enum": KINDS },
                "query": { "type": "string" },
                "id": { "type": "string" }
            },
            "required": ["action"]
        }),
    }
}

fn line(n: &Note) -> String {
    format!("{} [{}] {}", n.id, n.kind, n.text)
}

pub async fn call(app: &AppHandle, input: &Value) -> Result<String, String> {
    let data = app.state::<AppState>().data_dir.clone();
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let m = mode(app);
    let out = match s("action").as_str() {
        "remember" => {
            if m == "off" {
                return Err("Memory is switched off (Settings → Memory); I can't keep notes right now.".into());
            }
            let (n, fresh) = add(&data, &s("text"), &s("kind"), "said")?;
            if fresh {
                format!("Remembered: {}", n.text)
            } else {
                format!("Already knew that; updated it: {}", n.text)
            }
        }
        "forget" => {
            let id = s("id");
            if !id.is_empty() {
                if remove(&data, &id) {
                    "Forgotten.".into()
                } else {
                    return Err("No note with that id.".into());
                }
            } else {
                let q = s("text");
                if q.is_empty() {
                    return Err("Say what to forget.".into());
                }
                let hits = search(&data, &q, 3);
                let strong: Vec<&Note> = hits.iter().filter(|n| similar(&n.text, &q) || words(&q).is_subset(&words(&n.text)) || n.text.to_lowercase().contains(&q.to_lowercase())).collect();
                match strong.as_slice() {
                    [] if hits.is_empty() => "I didn't have anything about that.".into(),
                    [] => format!("Not sure which one; call forget with an id:\n{}", hits.iter().map(line).collect::<Vec<_>>().join("\n")),
                    list => {
                        for n in list {
                            remove(&data, &n.id);
                        }
                        format!("Forgot: {}", list.iter().map(|n| n.text.as_str()).collect::<Vec<_>>().join("; "))
                    }
                }
            }
        }
        "search" => {
            let hits = search(&data, &s("query"), 8);
            if hits.is_empty() {
                "Nothing about that in memory.".into()
            } else {
                hits.iter().map(line).collect::<Vec<_>>().join("\n")
            }
        }
        "list" => {
            let all = load(&data);
            if all.is_empty() {
                "No notes yet.".into()
            } else {
                all.iter().map(line).collect::<Vec<_>>().join("\n")
            }
        }
        other => return Err(format!("Unknown memory action \"{other}\".")),
    };
    crate::emit(app, openclicky_core::Event::new("memory.changed", json!({})));
    Ok(out)
}

// ---- learning from chats (memory = auto) ----

const LEARN: &str = "You keep a small long-term memory about the user for their desktop helper, Pixie. \
Read the conversation and list only facts worth knowing weeks from now: who they are and what they do, how they like Pixie to answer or act, people (name + who they are to them), ongoing projects, their setup and how-tos for their tools. \
Skip: one-off questions and answers, what was on screen, passing moods or plans for today, anything Pixie said about itself, and anything private like passwords, keys, money or health details. \
Write each fact as one short sentence in third person (\"Works as a nurse in Leeds\", \"Prefers answers without small talk\"). \
Don't repeat existing notes; if a fact corrects or updates one, give that note's id in replaces. Most conversations have nothing worth keeping: then return an empty list. \
Reply with JSON only: {\"notes\":[{\"text\":\"…\",\"kind\":\"about|preference|person|project|howto|other\",\"replaces\":\"\"}]}";

/// JSON out of a model reply (tolerates code fences and chatter around it).
fn json_in(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(&text[start..=end]).ok()
}

/// Learn from part of a conversation. Returns the notes added or changed.
pub async fn learn(app: &AppHandle, entries: &[crate::session::ChatEntry]) -> Vec<String> {
    if entries.is_empty() || mode(app) != "auto" {
        return vec![];
    }
    let st = app.state::<AppState>();
    let Some(provider) = st.provider() else { return vec![] };
    let data = st.data_dir.clone();
    let existing = load(&data);
    let mut msg = String::from("Existing notes:\n");
    for n in existing.iter().rev().take(80) {
        msg.push_str(&format!("{} {}\n", n.id, n.text));
    }
    msg.push_str("\nConversation:\n");
    for e in entries {
        msg.push_str(&format!("They: {}\nPixie: {}\n", e.q.chars().take(600).collect::<String>(), e.a.chars().take(600).collect::<String>()));
    }
    let mut req = Request::new(LEARN, vec![Message::user_text(msg)]);
    req.max_tokens = 600;
    req.effort = Effort::Fast;
    let r = match tokio::time::timeout(Duration::from_secs(40), provider.complete(&req, &mut |_| {})).await {
        Ok(Ok(r)) => r.text(),
        _ => return vec![],
    };
    let Some(v) = json_in(&r) else { return vec![] };
    let mut changed = vec![];
    for n in v["notes"].as_array().into_iter().flatten().take(8) {
        let text = n["text"].as_str().unwrap_or("").trim();
        let kind = n["kind"].as_str().unwrap_or("other");
        let replaces = n["replaces"].as_str().unwrap_or("").trim();
        if text.is_empty() || looks_secret(text) {
            continue;
        }
        let ok = if !replaces.is_empty() && existing.iter().any(|e| e.id == replaces && e.source != "said" && e.source != "you") {
            update(&data, replaces, text, Some(kind)).is_ok()
        } else {
            matches!(add(&data, text, kind, "learned"), Ok((_, true)))
        };
        if ok {
            changed.push(text.to_string());
        }
    }
    if !changed.is_empty() {
        log::info!("memory: learned {} note(s)", changed.len());
        crate::emit(app, openclicky_core::Event::new("memory.changed", json!({})));
    }
    changed
}

// ---- Settings → Memory ----

#[tauri::command]
pub fn memory_list(app: AppHandle) -> Vec<Note> {
    let mut v = load(&app.state::<AppState>().data_dir);
    v.sort_by_key(|n| std::cmp::Reverse(n.updated));
    v
}

#[tauri::command]
pub fn memory_add(app: AppHandle, text: String, kind: String) -> Result<Note, String> {
    let r = add(&app.state::<AppState>().data_dir, &text, &kind, "you").map(|(n, _)| n);
    crate::emit(&app, openclicky_core::Event::new("memory.changed", json!({})));
    r
}

#[tauri::command]
pub fn memory_update(app: AppHandle, id: String, text: String, kind: Option<String>) -> Result<Note, String> {
    update(&app.state::<AppState>().data_dir, &id, &text, kind.as_deref())
}

#[tauri::command]
pub fn memory_delete(app: AppHandle, id: String) {
    remove(&app.state::<AppState>().data_dir, &id);
}

#[tauri::command]
pub fn memory_clear(app: AppHandle) {
    clear(&app.state::<AppState>().data_dir);
    crate::emit(&app, openclicky_core::Event::new("memory.changed", json!({})));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_dedupe_and_refuse_secrets() {
        let d = tempfile::tempdir().unwrap();
        let (a, new) = add(d.path(), "Prefers short answers", "preference", "said").unwrap();
        assert!(new);
        let (b, new) = add(d.path(), "prefers short answers", "preference", "learned").unwrap();
        assert!(!new);
        assert_eq!(a.id, b.id);
        assert_eq!(b.source, "said");
        assert!(add(d.path(), "Their GitHub token is ghp_abcdefghijklmnopqrstuvwxyz123456", "other", "said").is_err());
        assert!(add(d.path(), "The wifi password is hunter2", "other", "said").is_err());
        add(d.path(), "Their manager is Priya Shah", "person", "learned").unwrap();
        let hits = search(d.path(), "who is priya", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].uses, 1);
        assert_eq!(load(d.path()).len(), 2);
        assert!(remove(d.path(), &a.id));
        assert_eq!(load(d.path()).len(), 1);
    }

    #[test]
    fn old_memory_md_comes_over() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("memory.md"), "# What Pixie knows about you\n\n- Uses Notion for notes (2026-11-05)\n").unwrap();
        let v = load(d.path());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].text, "Uses Notion for notes");
        assert!(d.path().join("memory.json").exists());
    }

    #[test]
    fn json_from_chatty_reply() {
        let v = json_in("Sure:\n```json\n{\"notes\":[{\"text\":\"Likes tea\",\"kind\":\"preference\"}]}\n```").unwrap();
        assert_eq!(v["notes"][0]["text"], "Likes tea");
    }
}
