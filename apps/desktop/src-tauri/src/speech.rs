//! Streaming text → spoken sentences. Text deltas are cut at sentence ends and queued on the
//! helper's synthesizer as they arrive, so speech starts before the answer is complete.

use crate::{emit, AppState};
use openclicky_core::Event;
use serde_json::json;
use tauri::{AppHandle, Manager};

/// Strip what shouldn't be read aloud.
pub fn speakable(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_code = false;
    for line in s.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || t.starts_with("::") || t.starts_with('|') {
            continue;
        }
        let t = t.trim_start_matches(['#', '>', '-', '*']).trim();
        out.push_str(t);
        out.push(' ');
    }
    out.replace("**", "").replace(['`', '*'], "").replace("  ", " ").trim().to_string()
}

/// Index just past the first sentence end at or after `min` chars.
pub fn sentence_end(buf: &str, min: usize) -> Option<usize> {
    let chars: Vec<(usize, char)> = buf.char_indices().collect();
    for (i, &(at, c)) in chars.iter().enumerate() {
        if at < min && c != '\n' {
            continue;
        }
        let next = chars.get(i + 1).map(|x| x.1);
        let end = match c {
            '\n' => true,
            '.' | '!' | '?' | '…' | ';' => matches!(next, Some(' ') | Some('\n')) && !abbreviation(&buf[..at]),
            _ => false,
        };
        if end {
            return Some(at + c.len_utf8());
        }
    }
    // Very long run without punctuation: break at a comma.
    if buf.len() > 220 {
        if let Some(p) = buf[..200].rfind(", ") {
            return Some(p + 1);
        }
    }
    None
}

fn abbreviation(before: &str) -> bool {
    let w = before.rsplit(' ').next().unwrap_or("");
    matches!(w, "e.g" | "i.e" | "Mr" | "Mrs" | "Ms" | "Dr" | "St" | "vs" | "etc") || w.chars().all(|c| c.is_ascii_digit()) && !w.is_empty()
}

pub struct SpeechStream {
    app: AppHandle,
    turn: u64,
    buf: String,
    n: usize,
    enabled: bool,
}

impl SpeechStream {
    pub fn new(app: &AppHandle, turn: u64) -> Self {
        let enabled = app.state::<AppState>().settings.lock().map(|s| s.speak_answers).unwrap_or(true);
        Self { app: app.clone(), turn, buf: String::new(), n: 0, enabled }
    }

    pub fn push(&mut self, delta: &str) {
        self.buf.push_str(delta);
        let min = if self.n == 0 { 12 } else { 30 };
        while let Some(end) = sentence_end(&self.buf, min) {
            let s: String = self.buf.drain(..end).collect();
            self.say(&s);
        }
    }

    pub fn flush(&mut self) {
        let rest = std::mem::take(&mut self.buf);
        self.say(&rest);
    }

    fn say(&mut self, text: &str) {
        let text = speakable(text);
        if text.chars().filter(|c| c.is_alphanumeric()).count() < 2 {
            return;
        }
        self.n += 1;
        let id = format!("{}-{}", self.turn, self.n);
        emit(&self.app, Event::new("speech.segment", json!({ "id": id, "text": text, "turn": self.turn })));
        if self.enabled {
            speak(&self.app, &id, &text);
        }
    }

    pub fn spoke_anything(&self) -> bool {
        self.n > 0
    }
}

/// Queue a line: the natural local voice when it's installed, else the system voice.
pub fn speak(app: &AppHandle, id: &str, text: &str) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().map(|s| s.clone()).unwrap_or_default();
    state.session.speaking_started(id);
    if s.voice_engine != "apple" && crate::kokoro::speak(app, id, text, s.kokoro_voice, s.voice_speed) {
        return;
    }
    state.helper.send("speak", json!({ "text": text, "utterance": id, "voice": s.voice_id, "rate": s.speech_rate }));
}

pub fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    crate::kokoro::stop();
    state.helper.send("speak_stop", json!({}));
    state.session.speaking_cleared();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_split_at_ends_not_abbreviations() {
        let b = "It's 3.5 km away. Take the e.g. tram! Then walk";
        let e = sentence_end(b, 5).unwrap();
        assert_eq!(&b[..e], "It's 3.5 km away.");
        let rest = &b[e..];
        let e2 = sentence_end(rest, 5).unwrap();
        assert_eq!(rest[..e2].trim(), "Take the e.g. tram!");
        assert!(sentence_end("Then walk", 5).is_none());
    }

    #[test]
    fn speakable_strips_markdown() {
        assert_eq!(speakable("**Bold** and `code`\n```\nx\n```\n- item"), "Bold and code item");
        assert_eq!(speakable("::figure[a]{b=1}\nHi"), "Hi");
    }
}
