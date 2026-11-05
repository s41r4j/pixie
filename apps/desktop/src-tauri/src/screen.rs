//! Screen context for a turn: screenshot + OCR + the frontmost app's controls, from the helper.

use crate::AppState;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
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
    #[serde(skip)]
    pub error: Option<String>,
}

impl Capture {
    pub fn image_base64(&self) -> Option<String> {
        let p = self.path.as_ref()?;
        let bytes = std::fs::read(p).ok()?;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    /// The text block the model reads alongside the screenshot.
    pub fn describe(&self) -> String {
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
                s.push_str(&format!(" — window \"{}\"", self.window));
            }
            s.push('\n');
        }
        if !self.elements.is_empty() {
            s.push_str("\nControls in the frontmost window (id, kind, label, x y w h):\n");
            for e in self.elements.iter().take(140) {
                let f = &e.frame;
                s.push_str(&format!("{} {} \"{}\" {} {} {} {}\n", e.id, e.role, trunc(&e.label, 60), f.x, f.y, f.w, f.h));
            }
        }
        if !self.text.is_empty() {
            s.push_str("\nText on screen, top to bottom (x y w h: text):\n");
            for t in self.text.iter().take(220) {
                let f = &t.frame;
                s.push_str(&format!("{} {} {} {}: {}\n", f.x, f.y, f.w, f.h, trunc(&t.text, 90)));
            }
        }
        s
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.replace('\n', " ")
    } else {
        format!("{}…", s.chars().take(n).collect::<String>().replace('\n', " "))
    }
}

/// Capture the screen under the pointer (our own windows excluded).
pub async fn capture(app: &AppHandle, n: u64) -> Capture {
    let state = app.state::<AppState>();
    let dir = state.data_dir.join("session/tmp");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("screen-{}.jpg", n % 8));
    let r = state
        .helper
        .call("capture", json!({ "path": path, "max_width": 4096, "ocr": true }), Duration::from_secs(8))
        .await;
    match r {
        Ok(v) => {
            let mut c: Capture = serde_json::from_value(v).unwrap_or_default();
            if c.path.as_ref().map(|p| !p.exists()).unwrap_or(false) {
                c.path = None;
            }
            c
        }
        Err(e) => Capture { error: Some(e), ..Default::default() },
    }
}

/// The best on-screen item (control or text line) for a rectangle the model gave:
/// highest overlap, else the nearest centre within 40 pt; a matching label wins ties.
/// Find an item by element id ("e12") or by its text/label: exact match first, then the
/// shortest line that contains it. Ties go to the one nearest `near` (if given).
pub fn find(c: &Capture, target: &str, near: Option<(f64, f64)>) -> Option<Frame> {
    let t = target.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(e) = c.elements.iter().find(|e| e.id.eq_ignore_ascii_case(t)) {
        return Some(e.frame.clone());
    }
    let tl = t.to_lowercase();
    let cands: Vec<(Frame, String)> = c
        .elements
        .iter()
        .map(|e| (e.frame.clone(), e.label.to_lowercase()))
        .chain(c.text.iter().map(|x| (x.frame.clone(), x.text.to_lowercase())))
        .filter(|(f, s)| f.w > 0.0 && f.h > 0.0 && !s.trim().is_empty())
        .collect();
    let dist = |f: &Frame| near.map(|(x, y)| ((f.x + f.w / 2.0 - x).powi(2) + (f.y + f.h / 2.0 - y).powi(2)).sqrt()).unwrap_or(0.0);
    let pick = |v: Vec<&(Frame, String)>| v.into_iter().min_by(|a, b| dist(&a.0).total_cmp(&dist(&b.0)).then(a.1.len().cmp(&b.1.len()))).map(|x| x.0.clone());
    let exact: Vec<_> = cands.iter().filter(|(_, s)| s.trim() == tl).collect();
    if !exact.is_empty() {
        return pick(exact);
    }
    let contains: Vec<_> = cands.iter().filter(|(_, s)| s.contains(&tl)).collect();
    if !contains.is_empty() {
        // Prefer the tightest match: shortest text containing it, then nearest.
        let min_len = contains.iter().map(|(_, s)| s.len()).min().unwrap_or(0);
        return pick(contains.into_iter().filter(|(_, s)| s.len() <= min_len + 12).collect());
    }
    None
}

pub fn snap(c: &Capture, x: f64, y: f64, w: f64, h: f64, label: &str) -> Option<Frame> {
    let label = label.trim().to_lowercase();
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let mut best: Option<(f64, Frame)> = None;
    let cands = c
        .elements
        .iter()
        .map(|e| (e.frame.clone(), e.label.to_lowercase()))
        .chain(c.text.iter().map(|t| (t.frame.clone(), t.text.to_lowercase())));
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
        if !label.is_empty() && (text.contains(&label) || label.contains(text.as_str())) && !text.is_empty() {
            score += 0.5;
        }
        if best.as_ref().map(|(b, _)| score > *b).unwrap_or(true) {
            best = Some((score, f));
        }
    }
    best.map(|(_, f)| f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap() -> Capture {
        Capture {
            elements: vec![Element { id: "e1".into(), role: "button".into(), label: "Share".into(), frame: Frame { x: 1500.0, y: 10.0, w: 30.0, h: 24.0 } }],
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
}
