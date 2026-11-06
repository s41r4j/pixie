//! Natural local voice: Kokoro (82M) through sherpa-onnx, run by the `openclicky-voice`
//! sidecar. Sentences are synthesised in order and handed to the helper for playback (with word
//! timing), so speech starts while the answer is still streaming.
//!
//! Memory: the loaded model takes ~470 MB, so it isn't kept around. It loads (about a second)
//! when they start talking or the first sentence is queued, and unloads after a few quiet
//! minutes. Sentences queued while it loads are spoken once it's up.
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
use std::time::{Duration, Instant};
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

/// Unload the model after this long without speaking.
const IDLE_UNLOAD: Duration = Duration::from_secs(5 * 60);

struct Engine {
    stdin: Mutex<Option<ChildStdin>>,
    /// The model is loaded and taking requests.
    ready: AtomicBool,
    /// The server process is starting or running.
    running: AtomicBool,
    /// Lines queued while the model loads (lock order: queue, then stdin).
    queue: Mutex<Vec<String>>,
    last_used: Mutex<Option<Instant>>,
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
        running: AtomicBool::new(false),
        queue: Mutex::new(Vec::new()),
        last_used: Mutex::new(None),
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

/// The natural voice can be used here (installed, sidecar present); it may not be loaded yet.
fn available(app: &AppHandle) -> bool {
    cfg!(target_os = "macos") && installed(&app.state::<AppState>().data_dir) && binary().is_some()
}

fn touch() {
    *engine().last_used.lock().unwrap() = Some(Instant::now());
}

#[derive(Serialize)]
pub struct Status {
    /// The natural voice ships for macOS only so far.
    supported: bool,
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
        supported: cfg!(target_os = "macos"),
        installed: installed(&data),
        ready: installed(&data) && binary().is_some(),
        installing: engine().installing.load(Ordering::SeqCst),
        voices: VOICES.iter().map(|(id, n, d)| json!({ "id": id, "name": n, "description": d })).collect(),
        done: progress().lock().map(|p| p.0).unwrap_or(0),
        total: progress().lock().map(|p| p.1).unwrap_or(0),
    }
}

/// At launch: nothing loads yet; a watcher unloads the model when it's been quiet a while.
pub fn start(_app: AppHandle) {
    std::thread::Builder::new()
        .name("openclicky-voice-idle".into())
        .spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            let e = engine();
            let quiet = e.last_used.lock().unwrap().map(|t| t.elapsed() > IDLE_UNLOAD).unwrap_or(true);
            if e.ready.load(Ordering::SeqCst) && quiet && e.pending.lock().unwrap().is_empty() {
                log::info!("voice: quiet for a while, unloading Kokoro");
                // Closing stdin ends the server; the reader thread tidies up.
                *e.stdin.lock().unwrap() = None;
            }
        })
        .expect("voice idle thread");
}

/// Load the model now if it isn't (they started talking, so an answer is coming).
pub fn warm(app: &AppHandle) {
    let voice = app.state::<AppState>().settings.lock().map(|s| s.voice_engine.clone()).unwrap_or_default();
    if voice == "apple" || !available(app) {
        return;
    }
    touch();
    launch(app.clone());
}

/// Start the voice server unless it's already starting or running.
fn launch(app: AppHandle) {
    if engine().running.swap(true, Ordering::SeqCst) {
        return;
    }
    let data = app.state::<AppState>().data_dir.clone();
    let Some(bin) = binary() else {
        engine().running.store(false, Ordering::SeqCst);
        return;
    };
    std::thread::Builder::new()
        .name("openclicky-voice".into())
        .spawn(move || {
            let v = voices_dir(&data);
            let spawned = Command::new(&bin)
                .arg(v.join("runtime"))
                .arg(v.join("kokoro"))
                .arg("4")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let mut child = match spawned {
                Ok(c) => c,
                Err(e) => {
                    log::error!("voice: {e}");
                    stopped(&app);
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
                        // Under the queue lock, so nothing is queued after the flush.
                        let mut q = engine().queue.lock().unwrap();
                        engine().ready.store(true, Ordering::SeqCst);
                        let mut stdin = engine().stdin.lock().unwrap();
                        if let Some(w) = stdin.as_mut() {
                            for l in q.drain(..) {
                                let _ = w.write_all(l.as_bytes());
                            }
                            let _ = w.flush();
                        }
                        log::info!("voice: Kokoro loaded ({n} voices)");
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
            let _ = child.wait();
            stopped(&app);
        })
        .expect("voice thread");
}

/// The server is gone (unloaded, or it failed): anything still waiting falls through.
fn stopped(app: &AppHandle) {
    let waiting: Vec<String> = {
        let mut q = engine().queue.lock().unwrap();
        engine().ready.store(false, Ordering::SeqCst);
        *engine().stdin.lock().unwrap() = None;
        engine().running.store(false, Ordering::SeqCst);
        q.drain(..).filter_map(|l| l.split('\t').next().map(str::to_string)).collect()
    };
    let left: Vec<String> = engine().pending.lock().unwrap().keys().cloned().collect();
    for id in waiting.into_iter().chain(left) {
        delivered(app, &id, false);
    }
    log::info!("voice: Kokoro unloaded");
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

/// Queue a sentence (loading the model first if needed). False when the natural voice isn't
/// available, so the system voice speaks instead.
pub fn speak(app: &AppHandle, id: &str, text: &str, sid: i32, speed: f64) -> bool {
    if !ready() && !available(app) {
        return false;
    }
    touch();
    let clean: String = text.chars().map(|c| if c == '\t' || c == '\n' { ' ' } else { c }).collect();
    let line = format!("{id}\t{sid}\t{speed:.2}\t{}\t{clean}\n", wav_path(app, id).display());
    let gen = engine().gen.load(Ordering::SeqCst);
    let mut q = engine().queue.lock().unwrap();
    engine().pending.lock().unwrap().insert(id.to_string(), (gen, clean));
    if !ready() {
        q.push(line);
        drop(q);
        launch(app.clone());
        return true;
    }
    let mut guard = engine().stdin.lock().unwrap();
    let ok = guard.as_mut().map(|w| w.write_all(line.as_bytes()).and_then(|_| w.flush()).is_ok()).unwrap_or(false);
    if !ok {
        engine().pending.lock().unwrap().remove(id);
    }
    ok
}

/// Drop everything queued (barge-in / escape).
pub fn stop() {
    engine().gen.fetch_add(1, Ordering::SeqCst);
}

/// Download + unpack the runtime and model (user-initiated from Settings → Voice).
pub fn install(app: AppHandle) -> Result<(), String> {
    if cfg!(not(target_os = "macos")) {
        return Err("The natural voice is Mac-only for now; Pixie uses the system voice here.".into());
    }
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
                warm(&app);
            }
            Err(e) => crate::emit(&app, openclicky_core::Event::new("voice.install", json!({ "step": "error", "error": e }))),
        }
    });
    Ok(())
}
