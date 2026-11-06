//! The Swift helper's job on Windows and Linux, in-process: the same commands and the same
//! JSON answers (`capture`, `ax_snapshot`, `click_at`, `listen_*`, `speak`, `play`…), so the rest
//! of the app doesn't know which platform it's on. Geometry is in logical points, like the Mac.
//!
//! Commands run one at a time on a worker thread (UI Automation wants one COM thread);
//! speech plays on its own thread so a long sentence never blocks a screenshot.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod win;
#[cfg(target_os = "linux")]
use linux as os;
#[cfg(windows)]
use win as os;

#[cfg(windows)]
mod meter;
mod player;

use crate::macos::Rect;
use serde_json::{json, Value};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

static EVENTS: OnceLock<broadcast::Sender<(String, Value)>> = OnceLock::new();
type Job = (String, Value, oneshot::Sender<Result<Value, String>>);
static WORKER: OnceLock<mpsc::Sender<Job>> = OnceLock::new();

pub fn emit(name: &str, data: Value) {
    if let Some(tx) = EVENTS.get() {
        let _ = tx.send((name.to_string(), data));
    }
}

/// Start the worker and the speech player.
pub fn start(events: broadcast::Sender<(String, Value)>) {
    let _ = EVENTS.set(events);
    let (tx, rx) = mpsc::channel::<Job>();
    if WORKER.set(tx).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("pixie-native".into())
        .spawn(move || {
            #[cfg(windows)]
            win::init_thread();
            for (cmd, args, reply) in rx {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(&cmd, &args))).unwrap_or_else(|_| Err(format!("{cmd} crashed")));
                let _ = reply.send(r);
            }
        })
        .expect("native thread");
    player::start();
    emit("ready", json!({ "pid": std::process::id() }));
}

pub async fn call(cmd: &str, args: Value, timeout: Duration) -> Result<Value, String> {
    // Speech goes straight to the player: queuing it behind a screenshot would add lag.
    match cmd {
        "speak" => return Ok(player::speak(&args)),
        "play" => return Ok(player::play(&args)),
        "speak_stop" => {
            player::stop();
            return Ok(json!({}));
        }
        _ => {}
    }
    let worker = WORKER.get().ok_or("The native layer isn't running.")?;
    let (tx, rx) = oneshot::channel();
    worker.send((cmd.to_string(), args, tx)).map_err(|_| "The native layer stopped.".to_string())?;
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => Err("The native layer went away.".into()),
        Err(_) => Err(format!("{cmd} didn't finish in time.")),
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn f(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Value {
    json!({ "x": x.round() as i64, "y": y.round() as i64, "w": w.round() as i64, "h": h.round() as i64 })
}

const MAC_ONLY: &str = "That's a Mac feature (Calendar, Reminders and Contacts come from macOS). On this computer, use Google in Settings → Apps.";

fn handle(cmd: &str, args: &Value) -> Result<Value, String> {
    match cmd {
        "ping" => Ok(json!({ "pid": std::process::id(), "ax_trusted": true })),
        "capture" => capture(args),
        "cursor" => {
            let (x, y) = crate::macos::cursor();
            Ok(json!({ "x": x as i64, "y": y as i64 }))
        }
        "ax_snapshot" => Ok(os::snapshot(160)),
        "click_at" => {
            let mut r = os::click(f(args, "x"), f(args, "y"), args.get("button").and_then(Value::as_str).unwrap_or("left"), args.get("count").and_then(Value::as_u64).unwrap_or(1) as u32)?;
            std::thread::sleep(Duration::from_millis(450));
            r["after"] = os::controls_after();
            Ok(r)
        }
        "drag" => {
            let mut r = os::drag(f(args, "x"), f(args, "y"), f(args, "to_x"), f(args, "to_y"))?;
            r["after"] = os::controls_after();
            Ok(r)
        }
        "insert_text" => insert(s(args, "text")),
        "mac_step" => {
            let target = s(args, "target");
            match s(args, "action") {
                "press" | "click" => os::press(target),
                "focus" => os::focus(target),
                "menu" => os::menu(target),
                "type" => insert(target),
                "key" => os::press_keys(target).map(|_| json!({ "key": target })),
                "find" => os::find_json(target),
                "open" => os::open(target).map(|_| json!({ "opened": target })),
                "scroll" => {
                    os::scroll(target.to_lowercase().contains("up"));
                    Ok(json!({ "scrolled": target }))
                }
                a => Err(format!("Unknown step \"{a}\".")),
            }
        }
        "open" => os::open(s(args, "target")).map(|_| json!({})),
        "frontmost" => Ok(os::frontmost()),
        "voices" => Ok(os::voices()),
        "listen_start" => os::listen_start(s(args, "locale")),
        "listen_stop" => Ok(os::listen_stop()),
        "listen_cancel" => {
            os::listen_cancel();
            Ok(json!({}))
        }
        "pim_status" => Ok(json!({ "calendar": "unsupported", "reminders": "unsupported", "contacts": "unsupported" })),
        c if c.starts_with("pim_") || c.starts_with("calendar_") || c.starts_with("reminder") || c == "contacts_find" => Err(MAC_ONLY.into()),
        c => Err(format!("{c} isn't available on this computer yet.")),
    }
}

/// Clipboard + paste (Ctrl+V), then put the clipboard back, like the Mac.
fn insert(text: &str) -> Result<Value, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let saved = cb.get_text().ok();
    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(40));
    os::press_keys("ctrl+v")?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        if let (Some(t), Ok(mut cb)) = (saved, arboard::Clipboard::new()) {
            let _ = cb.set_text(t);
        }
    });
    #[allow(unused_mut)]
    let mut out = json!({ "inserted": text.chars().count(), "app": os::frontmost()["app"] });
    #[cfg(windows)]
    {
        let f = os::focused();
        if !f.is_empty() {
            let (role, label) = f.split_once(' ').unwrap_or((f.as_str(), ""));
            out["into"] = json!({ "role": role, "label": label });
        }
    }
    Ok(out)
}

/// A screenshot of one monitor in physical pixels.
pub struct Shot {
    pub img: Bgra,
    /// The monitor in global logical points.
    pub display: Rect,
    pub scale: f64,
}

pub struct Bgra {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Bgra {
    /// Scaled to fit `max` px a side; returns the image and the factor applied (Windows OCR).
    #[cfg(windows)]
    pub fn fit(&self, max: u32) -> (std::borrow::Cow<'_, Bgra>, f64) {
        if self.w <= max && self.h <= max {
            return (std::borrow::Cow::Borrowed(self), 1.0);
        }
        let k = max as f64 / self.w.max(self.h) as f64;
        let (w, h) = ((self.w as f64 * k) as u32, (self.h as f64 * k) as u32);
        let img = image::RgbaImage::from_raw(self.w, self.h, self.px.clone()).expect("size");
        let small = image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle);
        (std::borrow::Cow::Owned(Bgra { w, h, px: small.into_raw() }), k)
    }

    /// JPEG `width` px wide (BGRA → RGB).
    fn jpeg(&self, width: u32, path: &str) -> Result<(u32, u32), String> {
        let rgb: Vec<u8> = self.px.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0]]).collect();
        let img = image::RgbImage::from_raw(self.w, self.h, rgb).ok_or("bad image")?;
        let img = if width < self.w {
            let h = (self.h as f64 * width as f64 / self.w as f64).round() as u32;
            image::imageops::resize(&img, width, h, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 72);
        enc.encode_image(&img).map_err(|e| e.to_string())?;
        Ok((img.width(), img.height()))
    }
}

impl Clone for Bgra {
    fn clone(&self) -> Self {
        Bgra { w: self.w, h: self.h, px: self.px.clone() }
    }
}

/// Same answer as the Mac helper's `capture`.
fn capture(args: &Value) -> Result<Value, String> {
    let shot = os::capture()?;
    let (cx, cy) = crate::macos::cursor();
    let d = shot.display;
    let mut out = json!({
        "secure": false,
        "display": rect(d.x, d.y, d.w, d.h),
        "scale": shot.scale,
        "cursor": { "x": cx as i64, "y": cy as i64 },
    });
    if let Some(path) = args.get("path").and_then(Value::as_str) {
        // 1 image pixel = 1 logical point, so positions read off the image are exact.
        let max = args.get("max_width").and_then(Value::as_u64).unwrap_or(1440) as u32;
        let width = max.min(d.w.round() as u32).max(1);
        let (w, h) = shot.img.jpeg(width, path)?;
        out["path"] = json!(path);
        out["image"] = json!({ "w": w, "h": h });
    }
    if args.get("ocr").and_then(Value::as_bool).unwrap_or(true) {
        out["text"] = json!(os::ocr(&shot));
    }
    let front = os::frontmost();
    out["app"] = front["app"].clone();
    out["bundle"] = front["bundle"].clone();
    let ax = os::snapshot(160);
    out["window"] = ax["window"].clone();
    out["elements"] = ax["elements"].clone();
    out["ax_trusted"] = json!(true);
    Ok(out)
}
