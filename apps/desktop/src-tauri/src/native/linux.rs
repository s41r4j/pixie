//! Linux (X11 first): the desktop's own tools where they exist. Screenshots via grim, gnome-screenshot,
//! spectacle, scrot or ImageMagick; OCR via tesseract; input via xdotool; voice via espeak-ng.
//! There is no UI tree (AT-SPI) or speech recognition yet.

use super::{rect, Bgra, Shot};
use serde_json::{json, Value};
use std::process::Command;
use std::time::Duration;

fn have(bin: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file())).unwrap_or(false)
}

fn run(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin).args(args).output().map_err(|e| format!("{bin}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(format!("{bin}: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

pub fn capture() -> Result<Shot, String> {
    let path = std::env::temp_dir().join(format!("pixie-shot-{}.png", std::process::id()));
    let p = path.to_string_lossy().to_string();
    let tries: [(&str, Vec<&str>); 5] = [
        ("grim", vec![&p]),
        ("gnome-screenshot", vec!["-f", &p]),
        ("spectacle", vec!["-b", "-n", "-f", "-o", &p]),
        ("scrot", vec!["-o", &p]),
        ("import", vec!["-window", "root", &p]),
    ];
    let mut tried = false;
    for (bin, args) in tries.iter() {
        if have(bin) {
            tried = true;
            if run(bin, args).is_ok() && path.is_file() {
                break;
            }
        }
    }
    if !tried {
        return Err("Install grim (Wayland) or scrot (X11) so Pixie can see the screen.".into());
    }
    let img = image::open(&path).map_err(|e| format!("Couldn't read the screenshot: {e}"))?.to_rgba8();
    let _ = std::fs::remove_file(&path);
    // The tools capture every monitor; crop to the one under the cursor.
    let (cx, cy) = crate::macos::cursor();
    let (cpx, cpy) = crate::macos::to_physical(cx, cy);
    let (pos, size, scale) = crate::macos::phys_monitor_at(cpx, cpy);
    let (x, y) = (pos.0.max(0.0) as u32, pos.1.max(0.0) as u32);
    let (w, h) = ((size.0 as u32).min(img.width().saturating_sub(x)), (size.1 as u32).min(img.height().saturating_sub(y)));
    let crop = if w > 0 && h > 0 { image::imageops::crop_imm(&img, x, y, w, h).to_image() } else { img };
    let (w, h) = crop.dimensions();
    let px: Vec<u8> = crop.into_raw().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    Ok(Shot { img: Bgra { w, h, px }, display: crate::macos::Rect { x: pos.0 / scale, y: pos.1 / scale, w: w as f64 / scale, h: h as f64 / scale }, scale })
}

/// tesseract's TSV, grouped into lines.
pub fn ocr(shot: &Shot) -> Vec<Value> {
    if !have("tesseract") {
        return vec![];
    }
    let path = std::env::temp_dir().join(format!("pixie-ocr-{}.png", std::process::id()));
    let rgb: Vec<u8> = shot.img.px.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
    let Some(img) = image::RgbImage::from_raw(shot.img.w, shot.img.h, rgb) else { return vec![] };
    if img.save(&path).is_err() {
        return vec![];
    }
    let tsv = run("tesseract", &[&path.to_string_lossy(), "-", "tsv"]).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    let mut lines: Vec<(String, (f64, f64, f64, f64), Vec<String>)> = vec![];
    for row in tsv.lines().skip(1) {
        let c: Vec<&str> = row.split('\t').collect();
        if c.len() < 12 || c[0] != "5" || c[11].trim().is_empty() || c[10].parse::<f64>().unwrap_or(0.0) < 30.0 {
            continue;
        }
        let key = format!("{}-{}-{}", c[2], c[3], c[4]);
        let n = |i: usize| c[i].parse::<f64>().unwrap_or(0.0);
        let (l, t, w, h) = (n(6), n(7), n(8), n(9));
        match lines.iter_mut().find(|x| x.0 == key) {
            Some(x) => {
                let (x0, y0, x1, y1) = x.1;
                x.1 = (x0.min(l), y0.min(t), x1.max(l + w), y1.max(t + h));
                x.2.push(c[11].to_string());
            }
            None => lines.push((key, (l, t, l + w, t + h), vec![c[11].to_string()])),
        }
    }
    let d = shot.display;
    let k = shot.scale;
    lines
        .into_iter()
        .take(400)
        .map(|(_, (x0, y0, x1, y1), words)| json!({ "text": words.join(" "), "frame": rect(d.x + x0 / k, d.y + y0 / k, (x1 - x0) / k, (y1 - y0) / k) }))
        .collect()
}

pub fn snapshot(_limit: usize) -> Value {
    json!({ "trusted": false, "window": frontmost()["window"], "elements": [] })
}

const NO_TREE: &str = "Pressing controls by name needs the accessibility tree, which Pixie can't read on Linux yet. Point at it or click its position instead.";

pub fn find_json(_t: &str) -> Result<Value, String> {
    Err(NO_TREE.into())
}
pub fn press(_t: &str) -> Result<Value, String> {
    Err(NO_TREE.into())
}
pub fn focus(_t: &str) -> Result<Value, String> {
    Err(NO_TREE.into())
}
pub fn menu(_t: &str) -> Result<Value, String> {
    Err(NO_TREE.into())
}

fn xdotool(args: &[&str]) -> Result<String, String> {
    if !have("xdotool") {
        return Err("Install xdotool so Pixie can click and type (X11).".into());
    }
    run("xdotool", args)
}

pub fn click(x: f64, y: f64, button: &str, count: u32) -> Result<Value, String> {
    let (px, py) = crate::macos::to_physical(x, y);
    let (b, n) = (if button == "right" { "3" } else { "1" }, count.clamp(1, 3).to_string());
    let (sx, sy) = (format!("{}", px.round() as i64), format!("{}", py.round() as i64));
    xdotool(&["mousemove", "--sync", &sx, &sy, "click", "--repeat", &n, "--delay", "40", b, "mousemove_relative", "--sync", "0", "0"])?;
    Ok(json!({ "clicked": { "x": x as i64, "y": y as i64 }, "button": button, "count": count }))
}

pub fn drag(x: f64, y: f64, tx: f64, ty: f64) -> Result<Value, String> {
    let (ax, ay) = crate::macos::to_physical(x, y);
    let (bx, by) = crate::macos::to_physical(tx, ty);
    let f = |v: f64| format!("{}", v.round() as i64);
    xdotool(&["mousemove", "--sync", &f(ax), &f(ay), "mousedown", "1"])?;
    std::thread::sleep(Duration::from_millis(60));
    xdotool(&["mousemove", "--sync", &f(bx), &f(by), "mouseup", "1"])?;
    Ok(json!({ "dragged": { "from": { "x": x as i64, "y": y as i64 }, "to": { "x": tx as i64, "y": ty as i64 } } }))
}

pub fn scroll(up: bool) {
    let _ = xdotool(&["click", "--repeat", "3", if up { "4" } else { "5" }]);
}

/// "cmd+shift+t" → "ctrl+shift+t" in xdotool's names.
pub fn press_keys(combo: &str) -> Result<(), String> {
    let keys: Vec<String> = combo
        .to_lowercase()
        .split('+')
        .map(|p| match p.trim() {
            "cmd" | "command" | "⌘" | "ctrl" | "control" | "⌃" => "ctrl".to_string(),
            "opt" | "option" | "alt" | "⌥" => "alt".into(),
            "shift" | "⇧" => "shift".into(),
            "win" | "super" | "meta" => "super".into(),
            "return" | "enter" => "Return".into(),
            "esc" | "escape" => "Escape".into(),
            "delete" | "backspace" => "BackSpace".into(),
            "forwarddelete" => "Delete".into(),
            "pageup" => "Prior".into(),
            "pagedown" => "Next".into(),
            "space" => "space".into(),
            "tab" => "Tab".into(),
            "left" | "right" | "up" | "down" | "home" | "end" => {
                let mut c = p.trim().chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }
            other => other.to_string(),
        })
        .collect();
    xdotool(&["key", "--clearmodifiers", &keys.join("+")]).map(|_| ())
}

pub fn frontmost() -> Value {
    let window = xdotool(&["getactivewindow", "getwindowname"]).unwrap_or_default().trim().to_string();
    let pid = xdotool(&["getactivewindow", "getwindowpid"]).ok().and_then(|p| p.trim().parse::<u32>().ok()).unwrap_or(0);
    let exe = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default().trim().to_string();
    let app = window.rsplit(" - ").next().filter(|s| !s.is_empty() && s.len() < 40).map(str::to_string).unwrap_or_else(|| exe.clone());
    json!({ "app": app, "bundle": exe, "pid": pid, "secure": false, "window": window })
}

pub fn controls_after() -> Value {
    let f = frontmost();
    json!({ "app": f["app"], "window": f["window"], "focused": "", "controls": [] })
}

pub fn open(target: &str) -> Result<(), String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("Nothing to open.".into());
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let t = t.strip_prefix("~/").map(|r| home.join(r).display().to_string()).unwrap_or_else(|| t.to_string());
    if t.contains("://") || t.starts_with("mailto:") || std::path::Path::new(&t).exists() {
        return Command::new("xdg-open").arg(&t).spawn().map(|_| ()).map_err(|e| e.to_string());
    }
    Command::new("gtk-launch").arg(&t).spawn().or_else(|_| Command::new(&t).spawn()).map(|_| ()).map_err(|_| format!("I couldn't find an app called \"{t}\"."))
}

pub fn voices() -> Value {
    let v = if have("espeak-ng") { vec![json!({ "id": "espeak-ng", "name": "eSpeak NG", "language": "en", "quality": "default" })] } else { vec![] };
    json!({ "voices": v, "default": if v.is_empty() { "" } else { "espeak-ng" } })
}

pub fn synth(text: &str, _voice: &str, rate: f64, out: &std::path::Path) -> Result<(), String> {
    let wpm = ((rate / 0.52) * 175.0).clamp(80.0, 450.0).round().to_string();
    if have("espeak-ng") {
        return run("espeak-ng", &["-s", &wpm, "-w", &out.to_string_lossy(), text]).map(|_| ());
    }
    if have("pico2wave") {
        return run("pico2wave", &["-w", &out.to_string_lossy(), text]).map(|_| ());
    }
    Err("Install espeak-ng for a voice, or download the natural voice in Settings → Voice.".into())
}

pub fn listen_start(_locale: &str) -> Result<Value, String> {
    Err("Voice input isn't available on Linux yet. Type to Pixie in the notch instead.".into())
}

pub fn listen_stop() -> Value {
    json!({ "text": "" })
}

pub fn listen_cancel() {}
