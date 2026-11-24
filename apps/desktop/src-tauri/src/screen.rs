//! Screen context for a turn: screenshot + OCR + the frontmost app's controls, from the helper.

use pixie_core::sync::LockOk;
use crate::AppState;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TextLine {
    pub text: String,
    pub frame: Frame,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Element {
    pub id: String,
    pub role: String,
    pub label: String,
    pub frame: Frame,
    /// Its middle can't be clicked right now: scrolled out of its list, outside the window or off
    /// every screen (the helper works it out from the scroll areas it sits in).
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Capture {
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub display: Frame,
    #[serde(default)]
    pub cursor: Option<Value>,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub window: String,
    #[serde(default)]
    pub text: Vec<TextLine>,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub image: Option<Value>,
    /// The front window's frame (to say which screen it's on).
    #[serde(default)]
    pub window_frame: Option<Frame>,
    /// The helper may read other apps' controls (Accessibility).
    #[serde(default)]
    pub ax_trusted: Option<bool>,
    /// The front app's windows that aren't on screen (another desktop, or minimized).
    #[serde(default)]
    pub elsewhere: Vec<String>,
    /// The helper's snapshot these controls came from (it keeps the exact elements per snapshot).
    #[serde(default)]
    pub gen: Option<i64>,
    #[serde(skip)]
    pub error: Option<String>,
    /// Control ids the model saw earlier this turn that aren't on screen any more.
    #[serde(skip)]
    pub gone: Vec<String>,
    /// `text` came from Accessibility (after an action) rather than OCR of a screenshot.
    #[serde(skip)]
    pub ax_text: bool,
}

impl Capture {
    /// Screenshot pixels → screen points: (scale, offset x, offset y). 1:1 when the image is
    /// as wide as the screen or there's no image.
    pub fn pixel_scale(&self) -> (f64, f64, f64) {
        let d = &self.display;
        let w = self.image.as_ref().and_then(|i| i["w"].as_f64()).unwrap_or(0.0);
        let k = if w > 0.0 && d.w > 0.0 { d.w / w } else { 1.0 };
        (k, d.x, d.y)
    }

    pub fn image_base64(&self) -> Option<String> {
        let p = self.path.as_ref()?;
        let bytes = std::fs::read(p).ok()?;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    /// Everything on screen (look full=true).
    pub fn describe(&self) -> String {
        self.describe_with(None)
    }

    /// The screen for this request: the controls and text lines that match its words, then the
    /// ones nearest the pointer, up to 30 and 40, with a line saying how many more there are.
    /// Requests about reading the screen ("summarize this page") get all the text.
    pub fn describe_for(&self, request: &str) -> String {
        self.describe_with(Some(request))
    }

    fn describe_with(&self, request: Option<&str>) -> String {
        if self.secure {
            return "A password field is focused, so I didn't look at the screen.".into();
        }
        if let Some(e) = &self.error {
            return format!("I couldn't see the screen this time ({e}).");
        }
        let mut s = String::new();
        let d = &self.display;
        s.push_str(&format!(
            "Coordinates are screen points, origin top-left. This display spans x {}…{}, y {}…{}.\n",
            d.x, d.x + d.w, d.y, d.y + d.h
        ));
        if let Some(c) = &self.cursor {
            s.push_str(&format!("Their pointer is at ({}, {}).\n", c["x"], c["y"]));
        }
        if let Some(img) = &self.image {
            if let (Some(w), true) = (img["w"].as_f64(), d.w > 0.0) {
                if (w - d.w).abs() < 2.0 {
                    s.push_str(&format!(
                        "The screenshot is 1:1 with screen points: image pixel (px, py) is screen point (px + {}, py + {}).\n",
                        d.x, d.y
                    ));
                } else {
                    s.push_str(&format!(
                        "The screenshot is {} px wide; multiply its pixel positions by {:.4} and add ({}, {}) to get screen points.\n",
                        w, d.w / w, d.x, d.y
                    ));
                }
            }
        }
        s.push_str("For pointing, prefer the exact frames in the lists below over estimating from the image.\n");
        if !self.app.is_empty() {
            s.push_str(&format!("Frontmost app: {}", self.app));
            if !self.window.is_empty() {
                s.push_str(&format!(", window \"{}\"", self.window));
            }
            s.push('\n');
        }
        if !self.gone.is_empty() {
            s.push_str(&format!("Gone since your last view (don't use these ids): {}.\n", self.gone.join(", ")));
        }
        let (max_controls, max_text) = match request {
            None => (140, 220),
            Some(q) if wants_text(q) => (60, 220),
            Some(_) => (30, 40),
        };
        let words = request.map(request_words).unwrap_or_default();
        let pointer = self.cursor.as_ref().and_then(|c| Some((c["x"].as_f64()?, c["y"].as_f64()?)));
        // Menu bar icons match "menu bar", "icon", "status" as well as their own names.
        let elements = pick(&self.elements, max_controls, &words, pointer, |e| {
            let extra = if e.role == "menubaritem" { " menu bar icons status top right" } else { "" };
            (format!("{}{extra}", e.label), &e.frame)
        });
        if !elements.is_empty() {
            s.push_str("\nControls in the frontmost window (id, kind, label, x y w h; ids stay the same for this request):\n");
            for e in &elements {
                let f = &e.frame;
                s.push_str(&format!("{} {} \"{}\" {} {} {} {}{}\n", e.id, e.role, trunc(&e.label, 60), f.x, f.y, f.w, f.h, out_of_view(e)));
            }
        }
        let mut text = pick(&self.text, max_text, &words, pointer, |t| (t.text.clone(), &t.frame));
        text.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y).then(a.frame.x.total_cmp(&b.frame.x)));
        if !text.is_empty() {
            s.push_str("\nText on screen, top to bottom (x y w h: text):\n");
            for t in &text {
                let f = &t.frame;
                s.push_str(&format!("{} {} {} {}: {}\n", f.x, f.y, f.w, f.h, trunc(&t.text, 90)));
            }
        }
        let (more_c, more_t) = (self.elements.len().min(140) - elements.len(), self.text.len().min(220) - text.len());
        if more_c + more_t > 0 {
            s.push_str(&format!("\n({more_c} more controls and {more_t} more text lines not listed; call look with full=true to see everything.)\n"));
        }
        s
    }
}

/// Lower-case words of the request worth matching against labels (3+ letters, not filler).
fn request_words(q: &str) -> Vec<String> {
    const SKIP: &[&str] = &["the", "and", "for", "you", "can", "what", "where", "which", "this", "that", "with", "please", "how", "does", "show", "tell", "click", "open", "find", "button", "there", "here", "into", "from", "have", "are", "is"];
    q.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3 && !SKIP.contains(w))
        .map(str::to_string)
        .collect()
}

/// Requests that need the screen's text, not just its controls.
pub fn wants_text(q: &str) -> bool {
    let q = q.to_lowercase();
    ["read", "summar", "what does it say", "what's it say", "translate", "this page", "this article", "this email", "this message", "this error", "the error", "proofread", "explain this", "what is this", "what's this", "rewrite", "reply to", "tl;dr", "tldr", "this code", "this document", "this doc"]
        .iter()
        .any(|k| q.contains(k))
}

/// Requests that are clearly not about the screen ("what's the capital of Australia?"): no word
/// that points at something on it. Pixie can still call look if it turns out to be.
pub fn about_screen(q: &str) -> bool {
    const SCREEN: &[&str] = &[
        "this", "that", "these", "those", "here", "there", "screen", "page", "window", "tab", "tabs", "app", "apps", "button", "menu", "menus", "click", "open", "close",
        "where", "see", "look", "looking", "show", "read", "find", "selected", "highlighted", "error", "file", "files", "folder", "type", "press", "scroll", "link",
        "image", "picture", "photo", "code", "line", "email", "mail", "message", "document", "doc", "form", "field", "settings", "icon", "top", "bottom", "left", "right",
        "corner", "front", "title", "bar", "toolbar", "sidebar", "dock", "desktop", "finder", "browser", "safari", "chrome", "video", "playing", "says", "current",
        "currently", "pointer", "cursor", "mouse", "fix", "above", "below", "next", "copy", "paste", "draw", "circle",
        "point", "highlight", "summarize", "summarise", "translate", "reply", "send", "save", "delete", "rename", "move", "fill", "select", "zoom",
    ];
    let q = q.to_lowercase();
    if wants_text(&q) {
        return true;
    }
    let words: Vec<&str> = q.split(|c: char| !(c.is_alphanumeric() || c == '\'')).filter(|w| !w.is_empty()).collect();
    words.iter().any(|w| SCREEN.contains(w))
}

/// Up to `n` items: those whose text matches the request's words first, then nearest the
/// pointer, keeping the original order among ties.
fn pick<'a, T>(items: &'a [T], n: usize, words: &[String], pointer: Option<(f64, f64)>, of: impl Fn(&T) -> (String, &Frame)) -> Vec<&'a T> {
    if items.len() <= n {
        return items.iter().collect();
    }
    let mut scored: Vec<(usize, f64, &T)> = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let (label, f) = of(it);
            let l = label.to_lowercase();
            let hits = words.iter().filter(|w| l.contains(w.as_str())).count() as f64;
            let dist = pointer.map(|(x, y)| ((f.x + f.w / 2.0 - x).powi(2) + (f.y + f.h / 2.0 - y).powi(2)).sqrt()).unwrap_or(i as f64);
            (i, hits * 1e6 - dist, it)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut keep: Vec<(usize, &T)> = scored.into_iter().take(n).map(|(i, _, t)| (i, t)).collect();
    keep.sort_by_key(|(i, _)| *i);
    keep.into_iter().map(|(_, t)| t).collect()
}

/// Marks a control that can't be clicked where it is (clicking its id scrolls to it first).
fn out_of_view(e: &Element) -> &'static str {
    if e.hidden {
        " (out of view: click its id and Pixie scrolls to it first; never click its x y)"
    } else {
        ""
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.replace('\n', " ")
    } else {
        format!("{}…", s.chars().take(n).collect::<String>().replace('\n', " "))
    }
}

// ---- stable control ids ----------------------------------------------------------------------
//
// The helper numbers controls e1, e2… afresh on every snapshot, so "e12" from the first look
// could be a different control after a click. Pixie keeps its own ids instead: the same control
// (same kind and label, about the same frame, or the only one of its kind and label) keeps its id
// for the whole request; new controls get new numbers; ids that disappear are reported as gone.

/// Which request the ids belong to: a new voice turn starts again from e1.
static TURN: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct Known {
    id: String,
    raw: String,
    role: String,
    label: String,
    frame: Frame,
    gen: Option<i64>,
    hidden: bool,
}

/// A control id the model used, resolved against the latest snapshot.
#[derive(Debug, Clone)]
pub struct Target {
    pub frame: Frame,
    pub label: String,
    /// The helper's own id for it in snapshot `gen`.
    pub raw: String,
    pub gen: Option<i64>,
    pub hidden: bool,
}

#[derive(Default)]
struct Ids {
    turn: u64,
    next: u32,
    /// The latest snapshot, with Pixie's ids.
    last: Vec<Known>,
}

fn ids() -> &'static Mutex<Ids> {
    static I: OnceLock<Mutex<Ids>> = OnceLock::new();
    I.get_or_init(|| Mutex::new(Ids::default()))
}

/// A new voice turn: control ids start again from e1.
pub fn begin_turn(n: u64) {
    TURN.store(n, Ordering::SeqCst);
}

fn near(a: &Frame, b: &Frame) -> bool {
    (a.x - b.x).abs() <= 6.0 && (a.y - b.y).abs() <= 6.0 && (a.w - b.w).abs() <= 6.0 && (a.h - b.h).abs() <= 6.0
}

/// Give a fresh snapshot's controls Pixie's ids (rewriting `id`); returns the ids that went away.
pub fn stabilize(elements: &mut [Element], gen: Option<i64>) -> Vec<String> {
    let mut g = ids().lock_ok();
    let turn = TURN.load(Ordering::SeqCst);
    if g.turn != turn {
        *g = Ids { turn, ..Default::default() };
    }
    let prev = std::mem::take(&mut g.last);
    let mut used = vec![false; prev.len()];
    let mut assigned: Vec<Option<String>> = vec![None; elements.len()];
    // 1. Same kind, label and frame.
    for (i, e) in elements.iter().enumerate() {
        if let Some(j) = prev.iter().enumerate().position(|(j, k)| !used[j] && k.role == e.role && k.label == e.label && near(&k.frame, &e.frame)) {
            used[j] = true;
            assigned[i] = Some(prev[j].id.clone());
        }
    }
    // 2. Moved: the only one left with that kind and label, before and now.
    for i in 0..elements.len() {
        if assigned[i].is_some() || elements[i].label.is_empty() {
            continue;
        }
        let e = &elements[i];
        let same_now = elements.iter().enumerate().filter(|(k, x)| assigned[*k].is_none() && x.role == e.role && x.label == e.label).count();
        let before: Vec<usize> = (0..prev.len()).filter(|&j| !used[j] && prev[j].role == e.role && prev[j].label == e.label).collect();
        if same_now == 1 && before.len() == 1 {
            used[before[0]] = true;
            assigned[i] = Some(prev[before[0]].id.clone());
        }
    }
    // 3. New controls get new numbers.
    let mut last = Vec::with_capacity(elements.len());
    for (i, e) in elements.iter_mut().enumerate() {
        let id = match assigned[i].take() {
            Some(id) => id,
            None => {
                g.next += 1;
                format!("e{}", g.next)
            }
        };
        last.push(Known { id: id.clone(), raw: std::mem::replace(&mut e.id, id), role: e.role.clone(), label: e.label.clone(), frame: e.frame.clone(), gen, hidden: e.hidden });
    }
    g.last = last;
    prev.into_iter().zip(used).filter(|(_, u)| !u).map(|(k, _)| k.id).collect()
}

/// A control id ("e12") the model used → its frame, label and the helper's own id for it, from
/// the latest snapshot. `Err` when it isn't on screen any more.
pub fn element(id: &str) -> Option<Result<Target, String>> {
    let id = id.trim().to_ascii_lowercase();
    let is_id = id.len() > 1 && id.starts_with('e') && id[1..].chars().all(|c| c.is_ascii_digit());
    if !is_id {
        return None;
    }
    let g = ids().lock_ok();
    Some(match g.last.iter().find(|k| k.id == id) {
        Some(k) => Ok(Target { frame: k.frame.clone(), label: k.label.clone(), raw: k.raw.clone(), gen: k.gen, hidden: k.hidden }),
        None => Err(format!("{id} isn't on screen any more (the window changed). Use an id from the latest controls list, or call look.")),
    })
}

/// Screenshots for a question are sent this wide (fewer image tokens every step); controls and
/// text keep exact screen-point frames, and `look full=true` sends the full resolution.
pub const IMAGE_WIDTH: u32 = 1280;

/// Capture the screen under the pointer (our own windows excluded).
pub async fn capture(app: &AppHandle, n: u64) -> Capture {
    capture_at(app, n, IMAGE_WIDTH).await
}

/// Capture with the screenshot at most `max_width` pixels wide.
pub async fn capture_at(app: &AppHandle, n: u64, max_width: u32) -> Capture {
    capture_on(app, n, max_width, None).await
}

/// Capture the screen containing `at` (global points), or the pointer's.
pub async fn capture_on(app: &AppHandle, n: u64, max_width: u32, at: Option<(f64, f64)>) -> Capture {
    let state = app.state::<AppState>();
    let dir = state.data_dir.join("session/tmp");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("screen-{}.jpg", n % 8));
    let mut args = json!({ "path": path, "max_width": max_width, "ocr": true });
    if let Some((x, y)) = at {
        args["at"] = json!({ "x": x, "y": y });
    }
    let started = std::time::Instant::now();
    let r = state.helper.call("capture", args, Duration::from_secs(8)).await;
    match r {
        Ok(v) => {
            // How long each part took in the helper (image, text, controls, all), in ms.
            let mut took = v.get("took_ms").map(|t| t.to_string()).unwrap_or_default();
            if v.get("partial").and_then(Value::as_bool) == Some(true) {
                took.push_str(" (partial: a part ran past 5 s)");
            }
            let mut c: Capture = serde_json::from_value(v).unwrap_or_default();
            if c.path.as_ref().map(|p| !p.exists()).unwrap_or(false) {
                c.path = None;
            }
            c.gone = stabilize(&mut c.elements, c.gen);
            let icons: Vec<&str> = c.elements.iter().filter(|e| e.role == "menubaritem" && e.frame.x > c.display.x + c.display.w / 2.0).map(|e| e.label.as_str()).collect();
            log::info!("capture: {} ({}), {} controls (accessibility {:?}), {} text lines, menu bar icons: {}; {} ms in all, parts {took}", c.app, c.window, c.elements.len(), c.ax_trusted, c.text.len(), icons.join(", "), started.elapsed().as_millis());
            // Pointing and clicking snap against the newest screen only.
            *state.last_capture.lock_ok() = Some(c.clone());
            c
        }
        Err(e) => Capture { error: Some(e), ..Default::default() },
    }
}

// ---- Several screens: one line of awareness, and looking at another screen on request. ----

#[derive(Debug, Clone, PartialEq)]
pub struct Display {
    pub name: String,
    pub frame: Frame,
    pub notch: bool,
}

impl Display {
    fn center(&self) -> (f64, f64) {
        (self.frame.x + self.frame.w / 2.0, self.frame.y + self.frame.h / 2.0)
    }
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.frame.x && x < self.frame.x + self.frame.w && y >= self.frame.y && y < self.frame.y + self.frame.h
    }
    fn builtin(&self) -> bool {
        self.notch || self.name.to_lowercase().contains("built-in")
    }
}

/// The screens, named (no capture; a hop to the main thread). Mirrored screens count once.
pub fn displays(app: &AppHandle) -> Vec<Display> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        let names = crate::macos::screen_names();
        let v: Vec<Display> = crate::macos::screens()
            .into_iter()
            .enumerate()
            .map(|(i, s)| Display {
                name: names.get(i).cloned().unwrap_or_else(|| format!("Screen {}", i + 1)),
                frame: Frame { x: s.frame.x, y: s.frame.y, w: s.frame.w, h: s.frame.h },
                notch: s.notch_width.is_some(),
            })
            .collect();
        let _ = tx.send(v);
    });
    let mut v: Vec<Display> = rx.recv_timeout(Duration::from_millis(400)).unwrap_or_default();
    let mut seen: Vec<Frame> = vec![];
    v.retain(|d| {
        let dup = seen.contains(&d.frame);
        seen.push(d.frame.clone());
        !dup
    });
    // Two screens of the same model: "LG", "LG 2".
    for i in 1..v.len() {
        let n = v[..i].iter().filter(|d| d.name == v[i].name || d.name.starts_with(&format!("{} ", v[i].name))).count();
        if n > 0 {
            v[i].name = format!("{} {}", v[i].name, n + 1);
        }
    }
    v
}

fn which(ds: &[Display], x: f64, y: f64) -> Option<usize> {
    ds.iter().position(|d| d.contains(x, y))
}

/// Where each screen sits: left / right (or above / below), by its centre.
fn places(ds: &[Display]) -> Vec<&'static str> {
    if ds.len() == 2 {
        let (a, b) = (ds[0].center(), ds[1].center());
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        return if dy.abs() > dx.abs() {
            if dy > 0.0 { vec!["above", "below"] } else { vec!["below", "above"] }
        } else if dx > 0.0 {
            vec!["left", "right"]
        } else {
            vec!["right", "left"]
        };
    }
    let mut order: Vec<usize> = (0..ds.len()).collect();
    order.sort_by(|a, b| ds[*a].center().0.total_cmp(&ds[*b].center().0));
    let mut out = vec!["middle"; ds.len()];
    if let (Some(f), Some(l)) = (order.first(), order.last()) {
        out[*f] = "left";
        out[*l] = "right";
    }
    out
}

/// One line about the screens, only when there's more than one (nothing is added otherwise).
pub fn screens_line(ds: &[Display], pointer: (f64, f64), window: Option<&Frame>) -> String {
    if ds.len() < 2 {
        return String::new();
    }
    let at = places(ds);
    let list: Vec<String> = ds
        .iter()
        .zip(&at)
        .map(|(d, p)| format!("{} {}×{} ({}{})", d.name, d.frame.w, d.frame.h, p, if d.notch { ", notch" } else { "" }))
        .collect();
    let mut s = format!("{}.", list.join(", "));
    let here = which(ds, pointer.0, pointer.1);
    if let Some(i) = here {
        s.push_str(&format!(" Pointer on {}", ds[i].name));
        let w = window.filter(|f| f.w > 0.0).and_then(|f| which(ds, f.x + f.w / 2.0, f.y + f.h / 2.0));
        if let Some(w) = w.filter(|w| *w != i) {
            s.push_str(&format!("; front window on {}", ds[w].name));
        }
        s.push_str(". The screen below is the pointer's; look display=<name>, other, window or all sees another.");
    }
    s
}

/// The screens `want` names: pointer (default), window, other, left, right, built-in, external,
/// all, a number or a name.
pub fn resolve(ds: &[Display], want: &str, pointer: (f64, f64), window: Option<&Frame>) -> Result<Vec<usize>, String> {
    if ds.is_empty() {
        return Ok(vec![]);
    }
    let here = which(ds, pointer.0, pointer.1).unwrap_or(0);
    let w = want.trim().to_lowercase();
    let pick = |i: Option<usize>| i.map(|i| vec![i]).ok_or_else(|| format!("There's no such screen. Screens: {}.", ds.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(", ")));
    let at = places(ds);
    match w.as_str() {
        "" | "pointer" | "this" | "current" | "mine" | "here" => Ok(vec![here]),
        "window" | "front" | "app" => Ok(vec![window.filter(|f| f.w > 0.0).and_then(|f| which(ds, f.x + f.w / 2.0, f.y + f.h / 2.0)).unwrap_or(here)]),
        "all" | "every" | "both" => Ok((0..ds.len()).take(3).collect()),
        "other" | "second" | "next" => pick((0..ds.len()).find(|i| *i != here)),
        "left" | "right" | "above" | "below" | "middle" => pick(at.iter().position(|p| *p == w)),
        "built-in" | "builtin" | "laptop" | "internal" | "main" => pick(ds.iter().position(Display::builtin).or(Some(0))),
        "external" | "monitor" => pick(ds.iter().position(|d| !d.builtin())),
        _ => {
            if let Ok(n) = w.parse::<usize>() {
                return pick(n.checked_sub(1).filter(|i| *i < ds.len()));
            }
            pick(ds.iter().position(|d| d.name.to_lowercase() == w).or_else(|| ds.iter().position(|d| d.name.to_lowercase().contains(&w))))
        }
    }
}

/// A screen's centre, to capture it.
pub fn center_of(ds: &[Display], i: usize) -> Option<(f64, f64)> {
    ds.get(i).map(Display::center)
}

pub fn name_of(ds: &[Display], i: usize) -> String {
    ds.get(i).map(|d| d.name.clone()).unwrap_or_default()
}

#[derive(Debug, Default, Deserialize)]
struct RawState {
    #[serde(default)]
    trusted: Option<bool>,
    #[serde(default)]
    app: String,
    #[serde(default)]
    window: String,
    #[serde(default)]
    focused: Option<Value>,
    #[serde(default)]
    elements: Vec<Element>,
    #[serde(default)]
    texts: Vec<TextLine>,
    #[serde(default)]
    cursor: Option<Value>,
    #[serde(default)]
    gen: Option<i64>,
}

/// How long the UI gets to settle after an action before Pixie looks again.
pub fn settle_for(action: &str) -> Duration {
    Duration::from_millis(match action {
        "open" => 1200,
        "click" | "double_click" | "right_click" | "press" | "menu" | "drag" => 450,
        "type" => 250,
        _ => 300,
    })
}

/// After an action: read the screen again (Accessibility only, no screenshot), make it the
/// screen Pixie snaps to, and describe it for the model: where focus is, the controls nearest
/// where it acted (with ids and frames), ids that went away, and text that's new.
pub async fn after_action(app: &AppHandle, at: Option<(f64, f64)>, settle: Duration) -> String {
    let st = app.state::<AppState>();
    let r = st.helper.call("screen_state", json!({ "settle": settle.as_secs_f64() }), Duration::from_secs(6) + settle).await;
    let raw: RawState = match r {
        Ok(v) => serde_json::from_value(v).unwrap_or_default(),
        Err(e) => {
            // Don't snap the next click to a screen that's gone.
            *st.last_capture.lock_ok() = None;
            return format!("(Couldn't read the screen after that: {e}. Call look before the next step.)");
        }
    };
    let prev = st.last_capture.lock_ok().clone();
    let mut now = Capture {
        app: raw.app,
        window: raw.window,
        elements: raw.elements,
        text: raw.texts,
        cursor: raw.cursor,
        // The screenshot is from before the action, so it no longer applies.
        display: prev.as_ref().map(|p| p.display.clone()).unwrap_or_default(),
        ax_text: true,
        gen: raw.gen,
        ..Default::default()
    };
    now.gone = stabilize(&mut now.elements, raw.gen);
    let text = describe_after(prev.as_ref(), &now, raw.focused.as_ref(), at, raw.trusted == Some(false));
    *st.last_capture.lock_ok() = Some(now);
    text
}

/// The compact "what's there now" the model gets after every action.
pub fn describe_after(prev: Option<&Capture>, now: &Capture, focused: Option<&Value>, at: Option<(f64, f64)>, untrusted: bool) -> String {
    let mut s = String::new();
    if untrusted {
        return "Accessibility is off, so I can't read the screen's controls; call look to see it.".into();
    }
    s.push_str(&format!("Now: {}", if now.app.is_empty() { "an app" } else { &now.app }));
    if !now.window.is_empty() {
        s.push_str(&format!(", window \"{}\"", trunc(&now.window, 80)));
    }
    let focus = focused
        .map(|f| {
            let role = f["role"].as_str().unwrap_or("").trim_start_matches("AX").to_lowercase();
            let label = f["label"].as_str().unwrap_or("");
            if role.is_empty() && label.is_empty() { "nothing".to_string() } else { format!("{role} \"{}\"", trunc(label, 60)) }
        })
        .unwrap_or_else(|| "nothing".into());
    s.push_str(&format!("; focused: {focus}.\n"));
    if prev.is_some_and(|p| p.app != now.app && !p.app.is_empty()) {
        s.push_str(&format!("(The front app changed from {}.)\n", prev.map(|p| p.app.as_str()).unwrap_or("")));
    }
    let (cx, cy) = at.or_else(|| now.cursor.as_ref().and_then(|c| Some((c["x"].as_f64()?, c["y"].as_f64()?)))).unwrap_or((0.0, 0.0));
    let mut els: Vec<&Element> = now.elements.iter().collect();
    els.sort_by(|a, b| {
        let d = |e: &Element| (e.frame.x + e.frame.w / 2.0 - cx).powi(2) + (e.frame.y + e.frame.h / 2.0 - cy).powi(2);
        d(a).total_cmp(&d(b))
    });
    const SHOWN: usize = 30;
    if els.is_empty() {
        s.push_str("No controls found in the front window; call look to see the screen.\n");
    } else {
        s.push_str("Controls nearest where you acted (id kind \"label\" x y w h):\n");
        for e in els.iter().take(SHOWN) {
            let f = &e.frame;
            s.push_str(&format!("{} {} \"{}\" {} {} {} {}{}\n", e.id, e.role, trunc(&e.label, 60), f.x, f.y, f.w, f.h, out_of_view(e)));
        }
        if els.len() > SHOWN {
            s.push_str(&format!("({} more controls; call look to see the whole screen.)\n", els.len() - SHOWN));
        }
    }
    if !now.gone.is_empty() {
        let mut g = now.gone.clone();
        g.truncate(20);
        s.push_str(&format!("Gone since your last view (don't use these ids): {}.\n", g.join(", ")));
    }
    // Text that's new since the last after-action read; after a screenshot (OCR, a different
    // source) just the lines nearest where it acted.
    let comparable = prev.is_some_and(|p| p.ax_text);
    let before: std::collections::HashSet<&str> = prev.filter(|p| p.ax_text).map(|p| p.text.iter().map(|t| t.text.as_str()).collect()).unwrap_or_default();
    let mut new_text: Vec<&TextLine> = now.text.iter().filter(|t| !before.contains(t.text.as_str()) && !t.text.trim().is_empty()).collect();
    if !comparable {
        let d = |t: &TextLine| (t.frame.x + t.frame.w / 2.0 - cx).powi(2) + (t.frame.y + t.frame.h / 2.0 - cy).powi(2);
        new_text.sort_by(|a, b| d(a).total_cmp(&d(b)));
        new_text.truncate(15);
    }
    new_text.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y).then(a.frame.x.total_cmp(&b.frame.x)));
    if !new_text.is_empty() {
        s.push_str(if comparable { "New text in the window (x y: text):\n" } else { "Text near there (x y: text):\n" });
        for t in new_text.iter().take(15) {
            s.push_str(&format!("{} {}: {}\n", t.frame.x, t.frame.y, trunc(&t.text, 90)));
        }
        if new_text.len() > 15 {
            s.push_str(&format!("({} more lines.)\n", new_text.len() - 15));
        }
    }
    s.push_str("Check it did what you expected before the next step.");
    s
}

/// Text as OCR and labels are compared: lower case, letters OCR mixes up made the same
/// (I, l, 1 and | → l; 0 → o), punctuation as spaces, single spaces.
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'i' | '1' | '|' | '!' => 'l',
            '0' => 'o',
            '’' | '\'' => continue,
            c => c,
        };
        if c.is_alphanumeric() {
            out.push(c);
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    out.trim_end().to_string()
}

/// One edit apart (a letter OCR misread, dropped or added), for words of 5+ letters.
fn close(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().min(b.len()) < 5 || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    edits + (a.len() - i) + (b.len() - j) <= 1
}

/// How much of `want` (normalised words) is in `have`: 0 to 1.
fn coverage(want: &[&str], have: &str) -> f64 {
    if want.is_empty() {
        return 0.0;
    }
    let have: Vec<&str> = have.split(' ').collect();
    want.iter().filter(|w| have.iter().any(|h| close(w, h))).count() as f64 / want.len() as f64
}

/// Find an item by element id ("e12") or by its text/label. In order: the same text, then the
/// shortest line containing it, then the line with most of its words (OCR misreads and a sentence
/// cut over two lines still find it). All of this ignores case, punctuation and the letters OCR
/// mixes up. Ties go to the one nearest `near` (if given).
pub fn find(c: &Capture, target: &str, near: Option<(f64, f64)>) -> Option<Frame> {
    let t = target.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(e) = c.elements.iter().find(|e| e.id.eq_ignore_ascii_case(t)) {
        return Some(e.frame.clone());
    }
    if let Some(Ok(k)) = element(t) {
        return Some(k.frame);
    }
    let tl = norm(t);
    if tl.is_empty() {
        return None;
    }
    // A control out of view can't be clicked where it's listed. Controls come before text, so
    // a button wins over the same words in a paragraph.
    let cands: Vec<(Frame, String, bool)> = c
        .elements
        .iter()
        .filter(|e| !e.hidden)
        .map(|e| (e.frame.clone(), norm(&e.label), true))
        .chain(c.text.iter().map(|x| (x.frame.clone(), norm(&x.text), false)))
        .filter(|(f, s, _)| f.w > 0.0 && f.h > 0.0 && !s.is_empty())
        .collect();
    let dist = |f: &Frame| near.map(|(x, y)| ((f.x + f.w / 2.0 - x).powi(2) + (f.y + f.h / 2.0 - y).powi(2)).sqrt()).unwrap_or(0.0);
    let pick = |v: Vec<&(Frame, String, bool)>| {
        v.into_iter().min_by(|a, b| dist(&a.0).total_cmp(&dist(&b.0)).then(b.2.cmp(&a.2)).then(a.1.len().cmp(&b.1.len()))).map(|x| x.0.clone())
    };
    let exact: Vec<_> = cands.iter().filter(|(_, s, _)| *s == tl).collect();
    if !exact.is_empty() {
        return pick(exact);
    }
    let contains: Vec<_> = cands.iter().filter(|(_, s, _)| s.contains(&tl)).collect();
    if !contains.is_empty() {
        // Prefer the tightest match: shortest text containing it, then nearest.
        let min_len = contains.iter().map(|(_, s, _)| s.len()).min().unwrap_or(0);
        return pick(contains.into_iter().filter(|(_, s, _)| s.len() <= min_len + 12).collect());
    }
    // Most of its words: one long word read with a slip, or a sentence over two lines.
    let want: Vec<&str> = tl.split(' ').collect();
    let need = if want.len() == 1 { 1.0 } else { 0.6 };
    let scored: Vec<(f64, &(Frame, String, bool))> = cands.iter().map(|c| (coverage(&want, &c.1), c)).filter(|(k, _)| *k >= need).collect();
    let best = scored.iter().map(|(k, _)| *k).fold(0.0, f64::max);
    if best > 0.0 {
        return pick(scored.into_iter().filter(|(k, _)| *k >= best - 0.01).map(|(_, c)| c).collect());
    }
    None
}

/// The best on-screen item (control or text line) for a rectangle the model gave, with how
/// sure the match is: highest overlap, else the nearest centre within 40 pt; a matching label
/// adds to it.
pub fn snap_scored(c: &Capture, x: f64, y: f64, w: f64, h: f64, label: &str) -> Option<(f64, Frame)> {
    let label = norm(label);
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let mut best: Option<(f64, Frame)> = None;
    let cands = c.elements.iter().map(|e| (e.frame.clone(), norm(&e.label))).chain(c.text.iter().map(|t| (t.frame.clone(), norm(&t.text))));
    for (f, text) in cands {
        if f.w <= 0.0 || f.h <= 0.0 || f.w > 900.0 {
            continue;
        }
        let ix = (x + w).min(f.x + f.w) - x.max(f.x);
        let iy = (y + h).min(f.y + f.h) - y.max(f.y);
        let inter = ix.max(0.0) * iy.max(0.0);
        let union = w * h + f.w * f.h - inter;
        let iou = if union > 0.0 { inter / union } else { 0.0 };
        let dist = ((f.x + f.w / 2.0 - cx).powi(2) + (f.y + f.h / 2.0 - cy).powi(2)).sqrt();
        let mut score = if iou > 0.05 { iou } else if dist < 40.0 { 0.05 * (1.0 - dist / 40.0) } else { continue };
        if !label.is_empty() && !text.is_empty() && (text.contains(&label) || label.contains(text.as_str())) {
            score += 0.5;
        }
        if best.as_ref().map(|(b, _)| score > *b).unwrap_or(true) {
            best = Some((score, f));
        }
    }
    best
}

#[cfg(test)]
fn snap(c: &Capture, x: f64, y: f64, w: f64, h: f64, label: &str) -> Option<Frame> {
    snap_scored(c, x, y, w, h, label).map(|(_, f)| f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap() -> Capture {
        Capture {
            elements: vec![Element { id: "e1".into(), role: "button".into(), label: "Share".into(), frame: Frame { x: 1500.0, y: 10.0, w: 30.0, h: 24.0 }, hidden: false }],
            text: vec![TextLine { text: "Total $42".into(), frame: Frame { x: 200.0, y: 600.0, w: 90.0, h: 18.0 } }],
            ..Default::default()
        }
    }

    #[test]
    fn snaps_near_misses_to_real_items() {
        let f = snap(&cap(), 1495.0, 18.0, 30.0, 24.0, "").unwrap();
        assert_eq!((f.x, f.y), (1500.0, 10.0));
        let f = snap(&cap(), 210.0, 590.0, 80.0, 20.0, "total").unwrap();
        assert_eq!(f.y, 600.0);
        assert!(snap(&cap(), 900.0, 900.0, 20.0, 20.0, "").is_none());
    }

    #[test]
    fn finds_by_id_and_text() {
        assert_eq!(find(&cap(), "e1", None).unwrap().x, 1500.0);
        assert_eq!(find(&cap(), "share", None).unwrap().x, 1500.0);
        assert_eq!(find(&cap(), "Total", None).unwrap().y, 600.0);
        assert!(find(&cap(), "nowhere", None).is_none());
    }

    #[test]
    fn finds_text_that_ocr_misread_or_cut() {
        let c = Capture {
            elements: vec![Element { id: "e1".into(), role: "link".into(), label: "Download".into(), frame: Frame { x: 900.0, y: 20.0, w: 70.0, h: 16.0 }, hidden: false }],
            text: vec![
                TextLine { text: "IocalGPT".into(), frame: Frame { x: 507.0, y: 635.0, w: 42.0, h: 12.0 } },
                TextLine { text: "Local question answering".into(), frame: Frame { x: 507.0, y: 655.0, w: 160.0, h: 12.0 } },
                TextLine { text: "over documents.".into(), frame: Frame { x: 507.0, y: 669.0, w: 90.0, h: 12.0 } },
                TextLine { text: "Download Pixie".into(), frame: Frame { x: 600.0, y: 500.0, w: 120.0, h: 20.0 } },
            ],
            ..Default::default()
        };
        assert_eq!(find(&c, "localGPT", None).unwrap().y, 635.0);
        assert_eq!(find(&c, "Local question answering over documents.", None).unwrap().y, 655.0);
        // A control with the exact words wins over a longer line of text.
        assert_eq!(find(&c, "download", None).unwrap().x, 900.0);
        assert_eq!(find(&c, "Download Pixie ↗", None).unwrap().y, 500.0);
        assert!(find(&c, "Upload", None).is_none());
        assert_eq!(norm("Save As…"), "save as");
    }

    fn two() -> Vec<Display> {
        vec![
            Display { name: "Built-in Retina Display".into(), frame: Frame { x: 0.0, y: 0.0, w: 1512.0, h: 982.0 }, notch: true },
            Display { name: "LG HDR 4K".into(), frame: Frame { x: 1512.0, y: -400.0, w: 2560.0, h: 1440.0 }, notch: false },
        ]
    }

    #[test]
    fn one_screen_adds_nothing() {
        assert_eq!(screens_line(&two()[..1], (10.0, 10.0), None), "");
    }

    #[test]
    fn several_screens_get_one_line() {
        let w = Frame { x: 100.0, y: 100.0, w: 800.0, h: 600.0 };
        let l = screens_line(&two(), (2000.0, 100.0), Some(&w));
        assert!(l.starts_with("Built-in Retina Display 1512×982 (left, notch), LG HDR 4K 2560×1440 (right)."), "{l}");
        assert!(l.contains("Pointer on LG HDR 4K; front window on Built-in Retina Display."), "{l}");
        // Same screen: no window note.
        assert!(!screens_line(&two(), (200.0, 200.0), Some(&w)).contains("front window"));
    }

    #[test]
    fn screens_are_picked_by_words() {
        let ds = two();
        let p = (2000.0, 100.0);
        let w = Frame { x: 100.0, y: 100.0, w: 800.0, h: 600.0 };
        assert_eq!(resolve(&ds, "", p, None).unwrap(), vec![1]);
        assert_eq!(resolve(&ds, "other", p, None).unwrap(), vec![0]);
        assert_eq!(resolve(&ds, "window", p, Some(&w)).unwrap(), vec![0]);
        assert_eq!(resolve(&ds, "laptop", p, None).unwrap(), vec![0]);
        assert_eq!(resolve(&ds, "external", p, None).unwrap(), vec![1]);
        assert_eq!(resolve(&ds, "left", p, None).unwrap(), vec![0]);
        assert_eq!(resolve(&ds, "lg", p, None).unwrap(), vec![1]);
        assert_eq!(resolve(&ds, "2", p, None).unwrap(), vec![1]);
        assert_eq!(resolve(&ds, "all", p, None).unwrap(), vec![0, 1]);
        assert!(resolve(&ds, "projector", p, None).is_err());
        // Pointer off every screen (between them): the first.
        assert_eq!(resolve(&ds, "", (-50.0, -50.0), None).unwrap(), vec![0]);
    }

    #[test]
    fn stacked_screens_are_above_and_below() {
        let ds = vec![
            Display { name: "A".into(), frame: Frame { x: 0.0, y: 0.0, w: 1512.0, h: 982.0 }, notch: true },
            Display { name: "B".into(), frame: Frame { x: 0.0, y: -1080.0, w: 1920.0, h: 1080.0 }, notch: false },
        ];
        assert_eq!(places(&ds), vec!["below", "above"]);
    }
}

/// The roadmap's scripted 4-step task: a fake app window whose helper numbering changes on every
/// snapshot, and a fake model that only knows what the tool results tell it (it picks controls
/// by the ids in the newest list). Passes when every step lands on the right control.
#[cfg(test)]
mod multi_step {
    use super::*;
    use pixie_core::prompt::Mode;
    use pixie_llm::{Content, LlmError, Message, OnEvent, Provider, Request, Response, Role};
    use pixie_tools::{AgentOptions, CancellationToken, Host, ToolCtx, ToolOutput};
    use std::sync::{Arc, Mutex};

    /// The id registry is shared, so these tests take turns.
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn el(role: &str, label: &str, x: f64, y: f64, w: f64, h: f64) -> Element {
        Element { id: String::new(), role: role.into(), label: label.into(), frame: Frame { x, y, w, h }, hidden: false }
    }

    /// The window at each stage, in the order the helper would list it (its own e1, e2…).
    fn screen(stage: usize) -> Vec<Element> {
        let base = vec![el("menubaritem", "File", 10.0, 0.0, 40.0, 22.0), el("menubaritem", "Edit", 60.0, 0.0, 40.0, 22.0)];
        let mut v = match stage {
            // The File menu opened: its items come first, so the helper's numbers shift.
            1 => vec![el("menuitem", "New Note", 20.0, 30.0, 140.0, 20.0), el("menuitem", "Open…", 20.0, 52.0, 140.0, 20.0), el("menuitem", "Close", 20.0, 74.0, 140.0, 20.0)],
            // A sheet with a title field; the search field moved a little.
            2 => vec![el("button", "Cancel", 540.0, 340.0, 70.0, 24.0), el("button", "Save", 620.0, 340.0, 60.0, 24.0), el("textfield", "Title", 400.0, 300.0, 280.0, 24.0)],
            _ => vec![],
        };
        v.extend(base);
        v.push(el("textfield", "Search", if stage >= 2 { 312.0 } else { 300.0 }, 40.0, 200.0, 24.0));
        v.push(el("button", "Sidebar", 900.0, 40.0, 30.0, 24.0));
        for (i, e) in v.iter_mut().enumerate() {
            e.id = format!("e{}", i + 1);
        }
        v
    }

    #[derive(Default)]
    struct FakeApp {
        stage: Mutex<usize>,
        prev: Mutex<Option<Capture>>,
        pressed: Mutex<Vec<String>>,
        typed: Mutex<Vec<(String, String)>>,
        search_ids: Mutex<Vec<String>>,
        focused: Mutex<String>,
    }

    impl FakeApp {
        fn hit(&self, x: f64, y: f64) -> Option<String> {
            let stage = *self.stage.lock_ok();
            screen(stage).into_iter().find(|e| x >= e.frame.x && x <= e.frame.x + e.frame.w && y >= e.frame.y && y <= e.frame.y + e.frame.h).map(|e| e.label)
        }

        /// The helper's screen_state after an action, through Pixie's ids and description.
        fn after(&self, at: (f64, f64)) -> String {
            let stage = *self.stage.lock_ok();
            let mut now = Capture { app: "Notes".into(), window: "Notes".into(), elements: screen(stage), ax_text: true, ..Default::default() };
            if stage == 3 {
                now.text.push(TextLine { text: "Groceries".into(), frame: Frame { x: 40.0, y: 120.0, w: 90.0, h: 18.0 } });
            }
            now.gone = stabilize(&mut now.elements, None);
            self.search_ids.lock_ok().extend(now.elements.iter().filter(|e| e.label == "Search").map(|e| e.id.clone()));
            let focused = json!({ "role": "AXTextField", "label": self.focused.lock_ok().clone() });
            let prev = self.prev.lock_ok().take();
            let text = describe_after(prev.as_ref(), &now, Some(&focused), Some(at), false);
            *self.prev.lock_ok() = Some(now);
            text
        }
    }

    #[async_trait::async_trait]
    impl Host for FakeApp {
        async fn call(&self, _: &ToolCtx, name: &str, input: &Value) -> Option<ToolOutput> {
            if name != "mac_step" {
                return Some(ToolOutput::err("not here"));
            }
            let action = input["action"].as_str().unwrap_or("");
            let (x, y) = match action {
                "press" => match element(input["target"].as_str().unwrap_or("")) {
                    Some(Ok(t)) => (t.frame.x + t.frame.w / 2.0, t.frame.y + t.frame.h / 2.0),
                    Some(Err(e)) => return Some(ToolOutput::err(e)),
                    None => return Some(ToolOutput::err("press needs an id here")),
                },
                "type" => (input["x"].as_f64().unwrap_or(-1.0), input["y"].as_f64().unwrap_or(-1.0)),
                _ => return Some(ToolOutput::err("unexpected action")),
            };
            let Some(label) = self.hit(x, y) else { return Some(ToolOutput::err(format!("nothing at ({x}, {y})"))) };
            if action == "type" {
                *self.focused.lock_ok() = label.clone();
                self.typed.lock_ok().push((label, input["target"].as_str().unwrap_or("").into()));
            } else {
                self.pressed.lock_ok().push(label.clone());
                let mut stage = self.stage.lock_ok();
                *stage = match (label.as_str(), *stage) {
                    ("File", 0) => 1,
                    ("New Note", 1) => 2,
                    ("Save", 2) => 3,
                    (_, s) => s,
                };
            }
            Some(ToolOutput::ok(self.after((x, y))))
        }
    }

    /// Knows only the goal list; finds each control's id and frame in the latest text it was given.
    struct FollowModel {
        goals: Mutex<std::collections::VecDeque<(&'static str, &'static str)>>,
    }

    fn latest_text(req: &Request) -> String {
        let m: &Message = req.messages.last().unwrap();
        assert_eq!(m.role, Role::User);
        m.content
            .iter()
            .map(|c| match c {
                Content::Text { text } => text.clone(),
                Content::ToolResult { content, .. } => content.iter().filter_map(|x| if let Content::Text { text } = x { Some(text.clone()) } else { None }).collect(),
                _ => String::new(),
            })
            .collect()
    }

    /// `e7 button "Save" 620 340 60 24` → ("e7", centre).
    fn find_line(text: &str, label: &str) -> Option<(String, f64, f64)> {
        text.lines().find_map(|l| {
            let (head, rest) = l.split_once(&format!(" \"{label}\" "))?;
            let id = head.split_whitespace().next()?.to_string();
            let n: Vec<f64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (id.starts_with('e') && n.len() == 4).then(|| (id, n[0] + n[2] / 2.0, n[1] + n[3] / 2.0))
        })
    }

    #[async_trait::async_trait]
    impl Provider for FollowModel {
        fn label(&self) -> String {
            "follow".into()
        }
        fn supports_images(&self) -> bool {
            false
        }
        async fn complete(&self, req: &Request, _: OnEvent<'_>) -> Result<Response, LlmError> {
            let text = latest_text(req);
            let Some((action, label)) = self.goals.lock_ok().pop_front() else {
                return Ok(pixie_llm::ScriptedProvider::reply(vec![Content::text("Done: the note is saved.")]));
            };
            let (id, x, y) = find_line(&text, label).unwrap_or_else(|| panic!("the model can't see {label} in:\n{text}"));
            let input = if action == "type" { json!({ "action": "type", "target": "Groceries", "x": x, "y": y }) } else { json!({ "action": "press", "target": id }) };
            Ok(pixie_llm::ScriptedProvider::reply(vec![Content::ToolUse { id: format!("t-{label}"), name: "mac_step".into(), input }]))
        }
    }

    #[tokio::test]
    async fn four_steps_land_on_the_right_controls() {
        let _one = SERIAL.lock().await;
        begin_turn(9_000_001);
        let host = Arc::new(FakeApp::default());
        // The first look (what the voice turn starts with).
        let mut first = Capture { app: "Notes".into(), elements: screen(0), ..Default::default() };
        first.gone = stabilize(&mut first.elements, None);
        let start = first.describe();
        *host.prev.lock_ok() = Some(first);

        let model = FollowModel { goals: Mutex::new(vec![("press", "File"), ("press", "New Note"), ("type", "Title"), ("press", "Save")].into()) };
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolCtx {
            mode: Mode::Voice,
            cwd: dir.path().into(),
            note_path: dir.path().join("note.md"),
            job_id: None,
            host: host.clone(),
            cancel: CancellationToken::new(),
            helper: None,
            coding: Default::default(),
        };
        let tools = pixie_tools::defs::for_mode(Mode::Voice, false);
        let out = pixie_tools::run(&model, "test", vec![Message::user_text(format!("{start}\nMake a note called Groceries."))], tools, &ctx, &AgentOptions { max_steps: 12, ..Default::default() }, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(*host.pressed.lock_ok(), vec!["File", "New Note", "Save"]);
        assert_eq!(*host.typed.lock_ok(), vec![("Title".to_string(), "Groceries".to_string())]);
        assert!(out.final_text.contains("saved"));
        // The search field kept its id while it moved and the numbering shifted around it.
        let ids = host.search_ids.lock_ok().clone();
        assert!(ids.len() >= 4 && ids.iter().all(|i| *i == ids[0]), "{ids:?}");
    }

    #[test]
    fn gone_ids_are_reported_and_refused() {
        let _one = SERIAL.blocking_lock();
        begin_turn(9_000_002);
        let mut a = screen(2);
        stabilize(&mut a, None);
        let save = a.iter().find(|e| e.label == "Save").unwrap().id.clone();
        let mut b = screen(3);
        let gone = stabilize(&mut b, None);
        assert!(gone.contains(&save));
        assert!(element(&save).unwrap().is_err());
        // A new request starts the numbering again.
        begin_turn(9_000_003);
        let mut c = screen(0);
        stabilize(&mut c, None);
        assert_eq!(c[0].id, "e1");
    }

    /// A chat list where "M0M" is scrolled below the window (the 0.8.0 bug clicked it at y 1132
    /// on a screen 1107 high): it keeps its id and the snapshot it came from, it's marked out of
    /// view for the model, and a click by its name doesn't pick it.
    #[test]
    fn rows_out_of_view_are_marked_and_not_clicked_by_name() {
        let _one = SERIAL.blocking_lock();
        begin_turn(9_000_004);
        let mut rows = vec![el("button", "Family", 0.0, 540.0, 520.0, 70.0), el("button", "M0M", 0.0, 1100.0, 520.0, 70.0)];
        rows[1].hidden = true;
        stabilize(&mut rows, Some(41));
        let mom = rows[1].id.clone();
        let t = element(&mom).unwrap().unwrap();
        assert!(t.hidden);
        assert_eq!(t.gen, Some(41));
        let c = Capture { elements: rows.clone(), ..Default::default() };
        assert!(describe_after(None, &c, None, Some((10.0, 10.0)), false).contains("\"M0M\" 0 1100 520 70 (out of view"));
        assert!(find(&c, "M0M", None).is_none());
        assert!(find(&c, "Family", None).is_some());
    }
}
