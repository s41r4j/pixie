//! Screen context for a turn: screenshot + OCR + the frontmost app's controls, from the helper.

use crate::AppState;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Default, Deserialize)]
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
                s.push_str(&format!(
                    "The screenshot is {} px wide; multiply its pixel positions by {:.3} and add the display origin to get screen points.\n",
                    w,
                    d.w / w
                ));
            }
        }
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
        .call("capture", json!({ "path": path, "max_width": 1440, "ocr": true }), Duration::from_secs(8))
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
