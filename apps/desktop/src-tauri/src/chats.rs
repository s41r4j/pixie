//! Saved conversations: every voice/typed conversation is kept as `<app_data>/chats/<id>.json`
//! so the Jobs window can show past chats next to jobs. A conversation ends with "New chat" or
//! after ten quiet minutes.

use crate::session::ChatEntry;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub started: i64,
    pub updated: i64,
    pub entries: Vec<ChatEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub started: i64,
    pub updated: i64,
    pub count: usize,
    pub preview: String,
}

fn dir(data: &Path) -> PathBuf {
    data.join("chats")
}

fn safe(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn new_id() -> String {
    format!("{}-{:04x}", chrono::Local::now().format("%Y%m%d-%H%M%S"), rand::random::<u16>())
}

fn title_of(entries: &[ChatEntry]) -> String {
    let q = entries.first().map(|e| e.q.trim()).unwrap_or("");
    let t: String = q.chars().take(60).collect();
    if q.chars().count() > 60 {
        format!("{}…", t.trim_end())
    } else if t.is_empty() {
        "Chat".into()
    } else {
        t
    }
}

pub fn save(data: &Path, id: &str, entries: &[ChatEntry]) {
    if !safe(id) || entries.is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(dir(data));
    let chat = Chat {
        id: id.to_string(),
        title: title_of(entries),
        started: entries.first().map(|e| e.at).unwrap_or(0),
        updated: entries.last().map(|e| e.at).unwrap_or(0),
        entries: entries.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&chat) {
        let _ = std::fs::write(dir(data).join(format!("{id}.json")), s);
    }
}

pub fn list(data: &Path) -> Vec<ChatSummary> {
    let mut v: Vec<ChatSummary> = std::fs::read_dir(dir(data))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|s| serde_json::from_str::<Chat>(&s).ok())
        .map(|c| ChatSummary {
            preview: c.entries.last().map(|e| e.a.chars().take(90).collect()).unwrap_or_default(),
            count: c.entries.len(),
            id: c.id,
            title: c.title,
            started: c.started,
            updated: c.updated,
        })
        .collect();
    v.sort_by_key(|c| std::cmp::Reverse(c.updated));
    v.truncate(200);
    v
}

pub fn get(data: &Path, id: &str) -> Option<Chat> {
    if !safe(id) {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(dir(data).join(format!("{id}.json"))).ok()?).ok()
}

pub fn delete(data: &Path, id: &str) {
    if safe(id) {
        let _ = std::fs::remove_file(dir(data).join(format!("{id}.json")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_get_delete() {
        let d = tempfile::tempdir().unwrap();
        let e = |q: &str, at| ChatEntry { q: q.into(), a: "ok".into(), at };
        save(d.path(), "c1", &[e("first question", 1), e("second", 5)]);
        save(d.path(), "c2", &[e("other", 9)]);
        save(d.path(), "../evil", &[e("x", 1)]);
        let l = list(d.path());
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].id, "c2");
        assert_eq!(get(d.path(), "c1").unwrap().title, "first question");
        delete(d.path(), "c1");
        assert!(get(d.path(), "c1").is_none());
    }
}
