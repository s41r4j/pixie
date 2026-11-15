//! Narrated drawing: marks written inside a spoken answer (`[[circle e12]]`) appear the moment
//! the sentence they sit in starts playing, so pointing and drawing keep pace with the voice
//! instead of all happening before the first word.
//!
//! The text stream is filtered here: marks are cut out of what's shown and spoken, and a
//! private-use character stands in their place so the sentence splitter knows which sentence
//! each one belongs to. When that sentence is queued for speech, its marks wait for the
//! helper's `speak_start`; when nothing is spoken (a typed answer, voice off) they run at once.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::AppHandle;

/// Stand-in characters for marks: U+F0000.. (Supplementary Private Use Area-A), never typed.
const BASE: u32 = 0xF0000;
const KINDS: &[&str] = &["point", "circle", "box", "underline", "arrow", "check", "cross", "3d", "canvas"];

#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub kind: String,
    /// An element id (e12) or exact on-screen text; for `3d`, the object's name.
    pub target: String,
    /// Where an arrow ends.
    pub to: String,
    pub label: String,
}

/// Split `a "b c" d` into words, keeping quoted text together.
fn words(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' | '“' | '”' => {
                if quoted || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|w| !w.is_empty());
    out
}

/// `14, 10, 30, 20` → `14,10,30,20` (coordinates written with spaces stay one target).
fn tight_numbers(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i].is_whitespace() || c[i] == ',' {
            // A run of spaces and one comma between two digits collapses to the comma.
            let mut j = i;
            let mut commas = 0;
            while j < c.len() && (c[j].is_whitespace() || c[j] == ',') {
                commas += (c[j] == ',') as usize;
                j += 1;
            }
            let before = out.chars().last().is_some_and(|p| p.is_ascii_digit());
            let after = c.get(j).is_some_and(|n| n.is_ascii_digit() || *n == '-');
            if commas == 1 && before && after {
                out.push(',');
                i = j;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// `circle e12`, `point "Save" Save button`, `arrow e3 e7`, `3d heart`.
pub fn parse(inner: &str) -> Option<Mark> {
    let w = words(&tight_numbers(inner.trim()));
    let kind = w.first()?.to_lowercase();
    let kind = match kind.as_str() {
        "highlight" | "show" => "point".to_string(),
        "rect" | "square" => "box".to_string(),
        "line" => "arrow".to_string(),
        k => k.to_string(),
    };
    if !KINDS.contains(&kind.as_str()) {
        return None;
    }
    let target = w.get(1)?.clone();
    let (to, rest) = if kind == "arrow" { (w.get(2).cloned().unwrap_or_default(), 3) } else { (String::new(), 2) };
    if kind == "arrow" && to.is_empty() {
        return None;
    }
    let label = w.iter().skip(rest).cloned().collect::<Vec<_>>().join(" ");
    Some(Mark { kind, target, to, label })
}

/// Cuts marks out of a text stream as it arrives.
#[derive(Default)]
pub struct Filter {
    pending: String,
    pub marks: Vec<Mark>,
}

impl Filter {
    /// Text that's safe to pass on: marks replaced by stand-ins, an unfinished `[[…` held back.
    pub fn push(&mut self, delta: &str) -> String {
        self.pending.push_str(delta);
        let mut out = String::new();
        loop {
            let Some(open) = self.pending.find("[[") else {
                // A lone "[" at the very end may be the start of a mark.
                let keep = if self.pending.ends_with('[') { 1 } else { 0 };
                let cut = self.pending.len() - keep;
                out.push_str(&self.pending[..cut]);
                self.pending.drain(..cut);
                break;
            };
            match self.pending[open..].find("]]") {
                Some(close) => {
                    let inner = self.pending[open + 2..open + close].to_string();
                    out.push_str(&self.pending[..open]);
                    if let Some(m) = parse(&inner) {
                        out.push(char::from_u32(BASE + self.marks.len() as u32).unwrap_or(' '));
                        self.marks.push(m);
                    }
                    self.pending.drain(..open + close + 2);
                }
                None if self.pending.len() - open > 160 => {
                    // Not a mark after all: let it through.
                    out.push_str(&self.pending[..open + 2]);
                    self.pending.drain(..open + 2);
                }
                None => {
                    out.push_str(&self.pending[..open]);
                    self.pending.drain(..open);
                    break;
                }
            }
        }
        out
    }

    /// Whatever is still held back at the end of the answer.
    pub fn finish(&mut self) -> String {
        std::mem::take(&mut self.pending)
    }
}

/// The stand-ins in `s`, as indexes into [`Filter::marks`].
pub fn indexes(s: &str) -> Vec<usize> {
    s.chars().filter_map(|c| (c as u32).checked_sub(BASE).filter(|i| *i < 0x1000).map(|i| i as usize)).collect()
}

/// `s` without stand-ins.
pub fn clean(s: &str) -> String {
    s.chars().filter(|c| !(BASE..BASE + 0x1000).contains(&(*c as u32))).collect()
}

/// A whole answer without its marks (for history, the chat log and the final text).
pub fn strip(s: &str) -> String {
    let mut f = Filter::default();
    let mut out = f.push(s);
    out.push_str(&f.finish());
    let mut t = clean(&out);
    for (a, b) in [("  ", " "), (" .", "."), (" ,", ","), (" ;", ";"), (" :", ":"), (" !", "!"), (" ?", "?")] {
        t = t.replace(a, b);
    }
    t.trim().to_string()
}

/// A whole line's text without marks, and its marks (for the `say` tool).
pub fn split(s: &str) -> (String, Vec<Mark>) {
    let mut f = Filter::default();
    let mut out = f.push(s);
    out.push_str(&f.finish());
    (clean(&out).replace("  ", " ").trim().to_string(), f.marks)
}

static WAITING: Mutex<Option<HashMap<String, Vec<Mark>>>> = Mutex::new(None);

/// Run `marks` when utterance `id` starts playing.
pub fn on_start_of(id: &str, marks: Vec<Mark>) {
    if marks.is_empty() {
        return;
    }
    WAITING.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), marks);
}

/// Utterance `id` started (or failed, so the marks still show): run its marks.
pub fn started(app: &AppHandle, id: &str) {
    let marks = WAITING.lock().unwrap().as_mut().and_then(|m| m.remove(id));
    if let Some(marks) = marks {
        run(app, marks);
    }
}

/// Speech was stopped: marks still waiting are dropped.
pub fn cancel_all() {
    if let Some(m) = WAITING.lock().unwrap().as_mut() {
        m.clear();
    }
}

/// Draw marks now (off the event loop: placing the overlay waits on the main thread).
pub fn run(app: &AppHandle, marks: Vec<Mark>) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        for m in marks {
            match run_one(&app, &m) {
                Ok(r) => log::info!("narrate: {} {} → {r}", m.kind, m.target),
                Err(e) => log::info!("narrate: {} {}: {e}", m.kind, m.target),
            }
        }
    });
}

/// `120,340` or `120,340,80,40` as coordinates.
fn coords(t: &str) -> Option<Vec<f64>> {
    let v: Vec<f64> = t.split(',').map(|p| p.trim().parse::<f64>()).collect::<Result<_, _>>().ok()?;
    (v.len() == 2 || v.len() == 4).then_some(v)
}

/// The model saw the screenshot this turn: numbers in marks are read off it, in its pixels.
static SAW_IMAGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_saw_image(on: bool) {
    SAW_IMAGE.store(on, std::sync::atomic::Ordering::SeqCst);
}

/// A target as overlay input. Numbers are screenshot pixels when the model saw one (models read
/// positions off the image; it's sent smaller than the screen), else screen points.
fn where_in(t: &str, scale: (f64, f64, f64)) -> Value {
    match coords(t) {
        Some(v) => {
            let (k, ox, oy) = scale;
            let w = v.get(2).copied().unwrap_or(24.0 / k);
            let h = v.get(3).copied().unwrap_or(24.0 / k);
            json!({ "x": ox + v[0] * k, "y": oy + v[1] * k, "w": w * k, "h": h * k })
        }
        None => json!({ "target": t }),
    }
}

#[cfg(test)]
fn where_(t: &str) -> Value {
    where_in(t, (1.0, 0.0, 0.0))
}

fn scale_now(app: &AppHandle) -> (f64, f64, f64) {
    use tauri::Manager;
    if !SAW_IMAGE.load(std::sync::atomic::Ordering::SeqCst) {
        return (1.0, 0.0, 0.0);
    }
    let cap = app.state::<crate::AppState>().last_capture.lock().unwrap().clone();
    cap.map(|c| c.pixel_scale()).unwrap_or((1.0, 0.0, 0.0))
}

fn run_one(app: &AppHandle, m: &Mark) -> Result<String, String> {
    let scale = scale_now(app);
    let where_ = |t: &str| where_in(t, scale);
    let at = where_(&m.target);
    match m.kind.as_str() {
        "3d" => crate::scene3d::focus(app, &m.target, &m.label),
        "canvas" => crate::canvas::focus(app, &m.target),
        "point" => {
            let mut input = at;
            input["label"] = json!(m.label);
            crate::overlay::highlight(app, &input)
        }
        "circle" | "box" | "underline" => {
            let mut op = at;
            op["kind"] = json!(m.kind);
            if !m.label.is_empty() && m.kind != "underline" {
                op["text"] = json!(m.label);
            }
            crate::overlay::draw(app, &json!({ "ops": [op] }))
        }
        "check" | "cross" => {
            // Just right of the thing it's about.
            let r = crate::overlay::rect_of(app, &at)?;
            crate::overlay::draw(app, &json!({ "ops": [{ "kind": m.kind, "x": r.x + r.w + 8.0, "y": r.y + r.h / 2.0 - 14.0 }] }))
        }
        "arrow" => {
            let a = crate::overlay::rect_of(app, &at)?;
            let b = crate::overlay::rect_of(app, &where_(&m.to))?;
            let (ax, ay) = (a.x + a.w / 2.0, a.y + a.h / 2.0);
            let (bx, by) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
            // Stop at the edge of the target, not its middle.
            let (dx, dy) = (bx - ax, by - ay);
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            let pull = (b.w.min(b.h) / 2.0 + 6.0).min(len / 2.0);
            let ex = bx - dx / len * pull;
            let ey = by - dy / len * pull;
            crate::overlay::draw(app, &json!({ "ops": [{ "kind": "arrow", "x": ax, "y": ay, "to_x": ex, "to_y": ey }] }))
        }
        _ => Err("unknown mark".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_parse() {
        assert_eq!(parse("circle e12").unwrap(), Mark { kind: "circle".into(), target: "e12".into(), to: "".into(), label: "".into() });
        let p = parse("point \"Save As…\" the save button").unwrap();
        assert_eq!((p.kind.as_str(), p.target.as_str(), p.label.as_str()), ("point", "Save As…", "the save button"));
        let a = parse("arrow e3 e7").unwrap();
        assert_eq!((a.target.as_str(), a.to.as_str()), ("e3", "e7"));
        assert!(parse("arrow e3").is_none());
        assert!(parse("wiggle e3").is_none());
        assert!(parse("circle").is_none());
        assert_eq!(parse("highlight e4").unwrap().kind, "point");
        let c = parse("point 14, 10, 30, 20 Apple menu").unwrap();
        assert_eq!((c.target.as_str(), c.label.as_str()), ("14,10,30,20", "Apple menu"));
        assert_eq!(parse("arrow 1, 2 3,4").unwrap().to, "3,4");
    }

    #[test]
    fn marks_are_cut_out_of_a_stream_split_anywhere() {
        let text = "This is the menu [[point e2]]. Then click [[circle \"Save\"]] here.";
        for split in 0..text.len() {
            if !text.is_char_boundary(split) {
                continue;
            }
            let mut f = Filter::default();
            let mut out = f.push(&text[..split]);
            out.push_str(&f.push(&text[split..]));
            out.push_str(&f.finish());
            assert_eq!(clean(&out), "This is the menu . Then click  here.", "split at {split}");
            assert_eq!(indexes(&out), vec![0, 1]);
            assert_eq!(f.marks.len(), 2);
            assert_eq!(f.marks[1].target, "Save");
        }
    }

    #[test]
    fn brackets_that_arent_marks_pass_through() {
        let mut f = Filter::default();
        let mut out = f.push("A list [1] and [[not a mark]] end [");
        out.push_str(&f.finish());
        assert_eq!(clean(&out), "A list [1] and  end [");
        assert!(f.marks.is_empty());
        assert_eq!(strip("See [[box e1]] this [[arrow e1 e2]]."), "See this.");
    }

    #[test]
    fn coordinates_work_as_targets() {
        assert_eq!(where_("120,340")["x"], 120.0);
        assert_eq!(where_("1,2,30,40")["h"], 40.0);
        assert_eq!(where_("e12")["target"], "e12");
        // A 1280 px screenshot of a 1710 pt screen at x 0.
        let k = 1710.0 / 1280.0;
        let p = where_in("1017,14,20,12", (k, 0.0, 0.0));
        assert!((p["x"].as_f64().unwrap() - 1358.6).abs() < 0.5, "{p}");
        assert!((p["w"].as_f64().unwrap() - 26.7).abs() < 0.1);
        // Second screen to the right: its origin is added.
        assert_eq!(where_in("10,10", (1.0, 1710.0, -200.0))["y"], -190.0);
    }
}
