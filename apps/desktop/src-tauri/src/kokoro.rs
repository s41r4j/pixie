//! Natural local voice: Kokoro (82M) through sherpa-onnx, run by the `openclicky-voice`
//! sidecar. The model loads once; sentences are synthesised in order and handed to the helper
//! for playback (with word timing), so speech starts while the answer is still streaming.
//!
//! Files: `<app_data>/voices/runtime` (sherpa-onnx v2.0.1 osx-arm64, shared) and
//! `<app_data>/voices/kokoro` (kokoro-multi-lang-v1_0). Settings → Voice can download them.

use crate::AppState;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

pub const RUNTIME_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/sherpa-onnx-v2.0.1-osx-universal2-shared.tar.bz2";
pub const MODEL_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2";

/// The voices worth offering (speaker IDs in kokoro-multi-lang-v1_0).
pub const VOICES: &[(i32, &str, &str)] = &[
    (3, "Heart", "warm, American (default)"),
    (2, "Bella", "bright, American"),
    (6, "Nicole", "soft, American"),
    (9, "Sarah", "calm, American"),
    (16, "Michael", "relaxed, American"),
    (14, "Fenrir", "deep, American"),
    (21, "Emma", "clear, British"),
    (26, "George", "steady, British"),
];

struct Engine {
    stdin: Mutex<Option<ChildStdin>>,
    ready: AtomicBool,
    gen: AtomicU64,
    /// request id → (generation, text)
    pending: Mutex<HashMap<String, (u64, String)>>,
    installing: AtomicBool,
}

fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(|| Engine {
        stdin: Mutex::new(None),
        ready: AtomicBool::new(false),
        gen: AtomicU64::new(0),
        pending: Mutex::new(HashMap::new()),
        installing: AtomicBool::new(false),
    })
}

pub fn voices_dir(data: &Path) -> PathBuf {
    data.join("voices")
}

pub fn installed(data: &Path) -> bool {
    let v = voices_dir(data);
    v.join("runtime/lib/libsherpa-onnx-c-api.dylib").exists() && v.join("kokoro/model.onnx").exists()
}

fn binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let p = exe.parent()?.join("openclicky-voice");
    p.is_file().then_some(p)
}

pub fn ready() -> bool {
    engine().ready.load(Ordering::SeqCst)
}

#[derive(Serialize)]
pub struct Status {
    installed: bool,
    ready: bool,
    installing: bool,
    voices: Vec<serde_json::Value>,
    /// Bytes downloaded / total across both files while installing.
    done: u64,
    total: u64,
}

/// (downloaded, total) for the install in progress.
fn progress() -> &'static Mutex<(u64, u64)> {
    static P: std::sync::OnceLock<Mutex<(u64, u64)>> = std::sync::OnceLock::new();
    P.get_or_init(|| Mutex::new((0, 0)))
}

/// Size of a download (follows redirects); 0 when unknown.
fn remote_size(url: &str) -> u64 {
    Command::new("/usr/bin/curl")
        .args(["-sIL", url])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .and_then(|h| h.lines().filter_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<u64>().ok())).next_back())
        .unwrap_or(0)
}

pub fn status(app: &AppHandle) -> Status {
    let data = app.state::<AppState>().data_dir.clone();
    Status {
        installed: installed(&data),
        ready: ready(),
        installing: engine().installing.load(Ordering::SeqCst),
        voices: VOICES.iter().map(|(id, n, d)| json!({ "id": id, "name": n, "description": d })).collect(),
        done: progress().lock().map(|p| p.0).unwrap_or(0),
        total: progress().lock().map(|p| p.1).unwrap_or(0),
    }
}

/// Start the voice server if the model is there.
pub fn start(app: AppHandle) {
    let data = app.state::<AppState>().data_dir.clone();
    if !installed(&data) {
        log::info!("voice: Kokoro not installed; using the system voice");
        return;
    }
    let Some(bin) = binary() else {
        log::warn!("voice: openclicky-voice sidecar missing");
        return;
    };
    std::thread::Builder::new()
        .name("openclicky-voice".into())
        .spawn(move || {
            let v = voices_dir(&data);
            let mut child = match Command::new(&bin)
                .arg(v.join("runtime"))
                .arg(v.join("kokoro"))
                .arg("4")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    log::error!("voice: {e}");
                    return;
                }
            };
            *engine().stdin.lock().unwrap() = child.stdin.take();
            let out = child.stdout.take().expect("stdout");
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                let f: Vec<&str> = line.splitn(4, '\t').collect();
                match f.as_slice() {
                    ["ready", n, ..] => {
                        engine().ready.store(true, Ordering::SeqCst);
                        log::info!("voice: Kokoro ready ({n} voices)");
                    }
                    ["fatal", msg, ..] => log::error!("voice: {msg}"),
                    [id, "ok", ..] => delivered(&app, id, true),
                    [id, "err", msg, ..] => {
                        log::warn!("voice: {id}: {msg}");
                        delivered(&app, id, false);
                    }
                    _ => {}
                }
            }
            engine().ready.store(false, Ordering::SeqCst);
            *engine().stdin.lock().unwrap() = None;
            let _ = child.wait();
            log::warn!("voice: server stopped; falling back to the system voice");
        })
        .expect("voice thread");
}

fn wav_path(app: &AppHandle, id: &str) -> PathBuf {
    let dir = app.state::<AppState>().data_dir.join("session/voice");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{}.wav", id.replace(['/', '\t'], "_")))
}

fn delivered(app: &AppHandle, id: &str, ok: bool) {
    let Some((gen, text)) = engine().pending.lock().unwrap().remove(id) else { return };
    let path = wav_path(app, id);
    let st = app.state::<AppState>();
    if ok && gen == engine().gen.load(Ordering::SeqCst) {
        st.helper.send("play", json!({ "path": path, "utterance": id, "text": text }));
    } else {
        let _ = std::fs::remove_file(&path);
        crate::session::speech_failed(app, id);
    }
}

/// Queue a sentence. False when the natural voice isn't available.
pub fn speak(app: &AppHandle, id: &str, text: &str, sid: i32, speed: f64) -> bool {
    if !ready() {
        return false;
    }
    let clean: String = text.chars().map(|c| if c == '\t' || c == '\n' { ' ' } else { c }).collect();
    let line = format!("{id}\t{sid}\t{speed:.2}\t{}\t{clean}\n", wav_path(app, id).display());
    let mut guard = engine().stdin.lock().unwrap();
    let Some(stdin) = guard.as_mut() else { return false };
    engine().pending.lock().unwrap().insert(id.to_string(), (engine().gen.load(Ordering::SeqCst), clean));
    if stdin.write_all(line.as_bytes()).and_then(|_| stdin.flush()).is_err() {
        engine().pending.lock().unwrap().remove(id);
        return false;
    }
    true
}

/// Drop everything queued (barge-in / escape).
pub fn stop() {
    engine().gen.fetch_add(1, Ordering::SeqCst);
}

/// Download + unpack the runtime and model (user-initiated from Settings → Voice).
pub fn install(app: AppHandle) -> Result<(), String> {
    if engine().installing.swap(true, Ordering::SeqCst) {
        return Err("Already downloading.".into());
    }
    let data = app.state::<AppState>().data_dir.clone();
    std::thread::spawn(move || {
        let v = voices_dir(&data);
        let r = (|| -> Result<(), String> {
            let files = [(RUNTIME_URL, "runtime"), (MODEL_URL, "kokoro")];
            let total: u64 = files.iter().filter(|(_, d)| !v.join(d).exists()).map(|(u, _)| remote_size(u)).sum();
            *progress().lock().unwrap() = (0, total);
            let mut before = 0u64;
            for (url, dest, inner) in [
                (RUNTIME_URL, "runtime", "sherpa-onnx-v2.0.1-osx-universal2-shared"),
                (MODEL_URL, "kokoro", "kokoro-multi-lang-v1_0"),
            ] {
                if v.join(dest).exists() {
                    continue;
                }
                crate::emit(&app, openclicky_core::Event::new("voice.install", json!({ "step": dest })));
                let stage = v.join(format!("dl-{dest}"));
                let _ = std::fs::remove_dir_all(&stage);
                std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
                let mut child = Command::new("/usr/bin/curl").args(["-fsSL", "-o", "pkg.tar.bz2", url]).current_dir(&stage).spawn().map_err(|e| e.to_string())?;
                let file = stage.join("pkg.tar.bz2");
                let mut last_emit = std::time::Instant::now();
                let ok = loop {
                    if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
                        break st;
                    }
                    let now_bytes = before + std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                    progress().lock().unwrap().0 = now_bytes;
                    if last_emit.elapsed() > std::time::Duration::from_millis(400) {
                        last_emit = std::time::Instant::now();
                        crate::emit(&app, openclicky_core::Event::new("voice.install", json!({ "step": dest, "done": now_bytes, "total": total })));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(150));
                };
                if !ok.success() {
                    return Err(format!("download failed: {url}"));
                }
                before += std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                crate::emit(&app, openclicky_core::Event::new("voice.install", json!({ "step": "unpack", "done": before, "total": total })));
                let ok = Command::new("/usr/bin/tar").args(["xjf", "pkg.tar.bz2"]).current_dir(&stage).status().map_err(|e| e.to_string())?;
                if !ok.success() {
                    return Err("couldn't unpack".into());
                }
                std::fs::rename(stage.join(inner), v.join(dest)).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_dir_all(&stage);
            }
            Ok(())
        })();
        engine().installing.store(false, Ordering::SeqCst);
        match r {
            Ok(()) => {
                crate::emit(&app, openclicky_core::Event::new("voice.install", json!({ "step": "done" })));
                start(app);
            }
            Err(e) => crate::emit(&app, openclicky_core::Event::new("voice.install", json!({ "step": "error", "error": e }))),
        }
    });
    Ok(())
}
