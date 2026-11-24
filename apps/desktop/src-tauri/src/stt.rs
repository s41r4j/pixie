//! Pixie's own speech to text (`pixie-stt`), the same on every system: words appear as they're
//! spoken, with no cloud and nothing from the operating system's own dictation.
//!
//! - The library (sherpa-onnx, shared with the natural voice) and an English streaming model are
//!   downloaded on request into `<app_data>/voices/` ([`install`]).
//! - The recogniser runs as a child of this same program ([`pixie_stt::engine`]) and stays loaded
//!   for a while after a dictation, so the next one starts at once.
//! - The microphone is read here (cpal), turned into 16 kHz mono and piped to it.
//! - With "type as you speak", each change goes to the text field as the fewest keystrokes
//!   (`live_type_*`: the Swift helper on the Mac, `native` elsewhere).

use pixie_core::sync::LockOk;
use crate::{emit, AppState};
use pixie_core::Event;
use pixie_stt::engine::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// The streaming English model (Apache-2.0, k2-fsa icefall; about 73 MB): remote name → ours.
const MODEL_URL: &str = "https://huggingface.co/csukuangfj/sherpa-onnx-streaming-zipformer-en-2023-06-26/resolve/main";
const MODEL_FILES: [(&str, &str); 4] = [
    ("encoder-epoch-99-avg-1-chunk-16-left-128.int8.onnx", "encoder.int8.onnx"),
    ("decoder-epoch-99-avg-1-chunk-16-left-128.int8.onnx", "decoder.int8.onnx"),
    ("joiner-epoch-99-avg-1-chunk-16-left-128.int8.onnx", "joiner.int8.onnx"),
    ("tokens.txt", "tokens.txt"),
];

/// sherpa-onnx v2.0.1 for this system: (download, the folder inside the archive).
fn runtime_package() -> Option<(&'static str, &'static str)> {
    const BASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/";
    let name = if cfg!(target_os = "macos") {
        "sherpa-onnx-v2.0.1-osx-universal2-shared"
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        "sherpa-onnx-v2.0.1-win-x64-shared-MT-Release"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "sherpa-onnx-v2.0.1-linux-x64-shared"
    } else {
        return None;
    };
    // Leaked once: two short strings for the life of the app.
    static URL: OnceLock<String> = OnceLock::new();
    Some((URL.get_or_init(|| format!("{BASE}{name}.tar.bz2")).as_str(), name))
}

/// Unload the model after this long without a dictation.
const IDLE_UNLOAD: Duration = Duration::from_secs(10 * 60);

fn runtime_dir(data: &Path) -> PathBuf {
    crate::kokoro::voices_dir(data).join("runtime")
}
fn model_dir(data: &Path) -> PathBuf {
    crate::kokoro::voices_dir(data).join("stt-en")
}

pub fn installed(data: &Path) -> bool {
    pixie_stt::ffi::installed(&runtime_dir(data), &model_dir(data))
}

/// What a recording is for.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum For {
    Talk,
    Dictate,
}

/// Should this recording use Pixie's engine? Settings → Voice → Speech to text:
/// "pixie" always (once downloaded), "system" never, and "auto" (the default) for dictation, and
/// for talking too where the system has no recogniser of its own (Linux). The model is English.
pub fn wanted(app: &AppHandle, kind: For) -> bool {
    let st = app.state::<AppState>();
    let (engine, locale) = st.settings.lock().map(|s| (s.stt_engine.clone(), s.locale.to_lowercase())).unwrap_or_default();
    if engine == "system" || !installed(&st.data_dir) {
        return false;
    }
    let english = locale.is_empty() || locale.starts_with("en");
    match engine.as_str() {
        "pixie" => true,
        _ => english && (kind == For::Dictate || cfg!(target_os = "linux")),
    }
}

struct Shared {
    engine: Mutex<Option<Engine>>,
    /// The recording in progress.
    active: Mutex<Option<Active>>,
    last_used: Mutex<Instant>,
    installing: AtomicBool,
    progress: Mutex<(u64, u64)>,
    /// Typing as they speak in this recording, and whether it was asked for at all.
    live: AtomicBool,
    live_asked: AtomicBool,
    /// Which recording the engine's words belong to (words from an older one are dropped).
    session: AtomicU64,
}

struct Active {
    stop: Arc<AtomicBool>,
    /// The final text, once the microphone thread has finished.
    done: Receiver<String>,
}

fn shared() -> &'static Shared {
    static S: OnceLock<Shared> = OnceLock::new();
    S.get_or_init(|| Shared {
        engine: Mutex::new(None),
        active: Mutex::new(None),
        last_used: Mutex::new(Instant::now()),
        installing: AtomicBool::new(false),
        progress: Mutex::new((0, 0)),
        live: AtomicBool::new(false),
        live_asked: AtomicBool::new(false),
        session: AtomicU64::new(0),
    })
}

/// The latest words go to the notch and, with type as you speak, to the text field: in order,
/// and only the newest when several arrive while the field is being typed into.
fn typist(app: &AppHandle) -> &'static tokio::sync::mpsc::UnboundedSender<String> {
    static TX: OnceLock<tokio::sync::mpsc::UnboundedSender<String>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(mut text) = rx.recv().await {
                while let Ok(newer) = rx.try_recv() {
                    text = newer;
                }
                if shared().live.load(Ordering::SeqCst) {
                    let helper = app.state::<AppState>().helper.clone();
                    let _ = helper.call("live_type_update", json!({ "text": text }), Duration::from_secs(3)).await;
                }
            }
        });
        tx
    })
}

/// Start the engine child if it isn't running (about a second the first time).
fn ensure_engine(app: &AppHandle) -> Result<(), String> {
    let s = shared();
    let mut guard = s.engine.lock_ok();
    if guard.as_mut().is_some_and(|e| e.alive()) {
        return Ok(());
    }
    let data = app.state::<AppState>().data_dir.clone();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(pixie_stt::engine::FLAG).arg(runtime_dir(&data)).arg(model_dir(&data));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let t0 = Instant::now();
    let app2 = app.clone();
    let engine = Engine::start(cmd, move |text| {
        emit(&app2, Event::new("mic.partial", json!({ "text": text })));
        let _ = typist(&app2).send(text.to_string());
    })?;
    log::info!("stt: Pixie's speech engine loaded in {} ms", t0.elapsed().as_millis());
    *guard = Some(engine);
    Ok(())
}

/// At launch: a watcher that unloads the model when dictation has been quiet for a while.
pub fn start(_app: AppHandle) {
    let _ = std::thread::Builder::new().name("pixie-stt-idle".into()).spawn(|| loop {
        std::thread::sleep(Duration::from_secs(60));
        let s = shared();
        if s.active.lock_ok().is_some() || s.last_used.lock_ok().elapsed() < IDLE_UNLOAD {
            continue;
        }
        if s.engine.lock_ok().take().is_some() {
            log::info!("stt: quiet for a while, unloading the speech model");
        }
    });
}

/// Samples from the microphone callback, as f32 in the device's own rate and channels.
fn open_mic(tx: Sender<Vec<f32>>) -> Result<(cpal::Stream, u32, u16), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_input_device().ok_or("No microphone input is available.")?;
    let config = device.default_input_config().map_err(|e| format!("Couldn't read the microphone's format: {e}"))?;
    let (rate, channels) = (config.sample_rate().0, config.channels());
    let err = |e| log::warn!("stt: microphone: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |d: &[f32], _: &_| {
                let _ = tx.send(d.to_vec());
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |d: &[i16], _: &_| {
                let _ = tx.send(d.iter().map(|s| *s as f32 / 32768.0).collect());
            },
            err,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |d: &[u16], _: &_| {
                let _ = tx.send(d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0).collect());
            },
            err,
            None,
        ),
        other => return Err(format!("The microphone's format ({other:?}) isn't one Pixie reads.")),
    }
    .map_err(|e| format!("Couldn't start the microphone: {e}"))?;
    stream.play().map_err(|e| format!("Couldn't start the microphone: {e}"))?;
    Ok((stream, rate, channels))
}

/// Begin recording. With `live`, words are typed into the focused field as they're heard;
/// the returned value says whether that took (`live`, `live_how`, `live_app`, `live_reason`).
pub async fn begin(app: &AppHandle, live: bool) -> Result<Value, String> {
    let s = shared();
    cancel(app);
    let session = s.session.fetch_add(1, Ordering::SeqCst) + 1;
    *s.last_used.lock_ok() = Instant::now();
    // The field first, so the first word has somewhere to go.
    let mut info = json!({ "engine": "pixie", "live": false });
    s.live.store(false, Ordering::SeqCst);
    s.live_asked.store(live, Ordering::SeqCst);
    if live {
        let helper = app.state::<AppState>().helper.clone();
        match helper.call("live_type_begin", json!({}), Duration::from_secs(3)).await {
            Ok(v) => {
                s.live.store(v["live"].as_bool() == Some(true), Ordering::SeqCst);
                for k in ["live", "live_how", "live_app", "live_reason"] {
                    info[k] = v[k].clone();
                }
            }
            Err(e) => info["live_reason"] = json!(e),
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let (done_tx, done) = channel::<String>();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let (stop2, app2) = (stop.clone(), app.clone());
    std::thread::Builder::new()
        .name("pixie-stt-mic".into())
        .spawn(move || record(app2, session, stop2, ready_tx, done_tx))
        .map_err(|e| e.to_string())?;
    *s.active.lock_ok() = Some(Active { stop, done });
    match tokio::time::timeout(Duration::from_secs(5), ready_rx).await {
        Ok(Ok(Ok(()))) => Ok(info),
        Ok(Ok(Err(e))) => {
            cancel(app);
            Err(e)
        }
        _ => {
            cancel(app);
            Err("The microphone didn't start.".into())
        }
    }
}

/// The microphone thread: open the mic at once (so no word is lost while the model loads),
/// then feed the engine until told to stop, and hand back the final text.
fn record(app: AppHandle, session: u64, stop: Arc<AtomicBool>, ready: tokio::sync::oneshot::Sender<Result<(), String>>, done: Sender<String>) {
    let s = shared();
    let (tx, rx) = channel::<Vec<f32>>();
    let (stream, rate, channels) = match open_mic(tx) {
        Ok(v) => v,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let mut resampler = pixie_stt::audio::Resampler::new(rate, channels);
    let mut heard: Vec<f32> = Vec::new();
    let mut engine_ok = false;
    let mut engine_error = None;
    let mut last_level = Instant::now();
    let (mut frames, mut peak) = (0usize, 0f32);
    loop {
        let stopping = stop.load(Ordering::SeqCst);
        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(chunk) => {
                if last_level.elapsed() >= Duration::from_millis(33) {
                    last_level = Instant::now();
                    emit(&app, Event::new("mic.level", json!({ "level": pixie_stt::audio::level(&chunk) })));
                }
                frames += chunk.len() / channels.max(1) as usize;
                peak = chunk.iter().fold(peak, |p, s| p.max(s.abs()));
                resampler.push(&chunk, &mut heard);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if !engine_ok && engine_error.is_none() {
            // Loading takes about a second the first time; the audio waits in `heard`.
            match ensure_engine(&app) {
                Ok(()) => engine_ok = true,
                Err(e) => engine_error = Some(e),
            }
        }
        if engine_ok && !heard.is_empty() && s.session.load(Ordering::SeqCst) == session {
            if let Some(e) = s.engine.lock_ok().as_mut() {
                e.audio(&heard);
            }
            heard.clear();
        }
        if stopping {
            break;
        }
    }
    drop(stream);
    log::info!("stt: microphone gave {:.1} s at {rate} Hz, {channels} channel(s), peak {peak:.3}", frames as f32 / rate.max(1) as f32);
    // The last few milliseconds that arrived while stopping.
    while let Ok(chunk) = rx.try_recv() {
        resampler.push(&chunk, &mut heard);
    }
    if engine_ok && !heard.is_empty() && s.session.load(Ordering::SeqCst) == session {
        if let Some(e) = s.engine.lock_ok().as_mut() {
            e.audio(&heard);
        }
    }
    if let Some(e) = engine_error {
        log::warn!("stt: {e}");
        crate::heal::note("stt", "Pixie's speech engine didn't start", "Nothing was typed", &e);
    }
    let text = if s.session.load(Ordering::SeqCst) == session { s.engine.lock_ok().as_mut().map(|e| e.finish()).unwrap_or_default() } else { String::new() };
    *s.last_used.lock_ok() = Instant::now();
    let _ = done.send(text);
}

/// Stop recording: everything heard, and (with type as you speak) what's in the field, in the
/// same shape the Mac helper's `listen_stop` gives.
pub async fn finish(app: &AppHandle) -> Option<Value> {
    let s = shared();
    let active = s.active.lock_ok().take()?;
    active.stop.store(true, Ordering::SeqCst);
    let text = tauri::async_runtime::spawn_blocking(move || active.done.recv_timeout(Duration::from_secs(6)).unwrap_or_default()).await.unwrap_or_default();
    let mut out = json!({ "text": text.trim() });
    if s.live.swap(false, Ordering::SeqCst) {
        let helper = app.state::<AppState>().helper.clone();
        if let Ok(v) = helper.call("live_type_end", json!({}), Duration::from_secs(3)).await {
            out["live"] = v["live"].clone();
        }
    } else if s.live_asked.swap(false, Ordering::SeqCst) {
        // Asked for but there was nowhere to type: the text goes on the clipboard.
        let helper = app.state::<AppState>().helper.clone();
        if let Ok(v) = helper.call("live_type_end", json!({}), Duration::from_secs(3)).await {
            out["no_surface"] = v["no_surface"].clone();
        }
    }
    Some(out)
}

/// Is a recording by Pixie's engine in progress?
pub fn recording() -> bool {
    shared().active.lock_ok().is_some()
}

/// Throw the recording away (esc, or a new one starting).
pub fn cancel(app: &AppHandle) {
    let s = shared();
    let Some(active) = s.active.lock_ok().take() else { return };
    active.stop.store(true, Ordering::SeqCst);
    // Words still on their way from this recording belong to nobody.
    s.session.fetch_add(1, Ordering::SeqCst);
    if let Some(e) = s.engine.lock_ok().as_mut() {
        e.forget();
    }
    if s.live.swap(false, Ordering::SeqCst) {
        app.state::<AppState>().helper.send("live_type_cancel", json!({}));
    }
}

/// Text from the engine has no capitals: start it with one, like a sentence, when the tidy-up
/// didn't run (a few words, or no model key).
pub fn sentence(text: &str) -> String {
    let mut c = text.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[derive(Serialize)]
pub struct Status {
    /// Can this system run it at all?
    pub supported: bool,
    pub installed: bool,
    pub installing: bool,
    pub done: u64,
    pub total: u64,
    /// "auto", "pixie" or "system".
    pub engine: String,
    /// Dictation uses it right now.
    pub in_use: bool,
}

pub fn status(app: &AppHandle) -> Status {
    let st = app.state::<AppState>();
    let s = shared();
    let (done, total) = *s.progress.lock_ok();
    Status {
        supported: runtime_package().is_some(),
        installed: installed(&st.data_dir),
        installing: s.installing.load(Ordering::SeqCst),
        done,
        total,
        engine: st.settings.lock().map(|s| s.stt_engine.clone()).unwrap_or_default(),
        in_use: wanted(app, For::Dictate),
    }
}

fn fetch(url: &str, to: &Path, before: u64, app: &AppHandle) -> Result<u64, String> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(async {
        use tokio::io::AsyncWriteExt;
        let http = reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).build().map_err(|e| e.to_string())?;
        let mut resp = http.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| format!("download failed: {e}"))?;
        let part = to.with_extension("part");
        let mut file = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
        let mut got = 0u64;
        let mut last = Instant::now();
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download failed: {e}"))? {
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            got += chunk.len() as u64;
            shared().progress.lock_ok().0 = before + got;
            if last.elapsed() > Duration::from_millis(400) {
                last = Instant::now();
                let total = shared().progress.lock_ok().1;
                emit(app, Event::new("stt.install", json!({ "step": "download", "done": before + got, "total": total })));
            }
        }
        file.flush().await.map_err(|e| e.to_string())?;
        drop(file);
        tokio::fs::rename(&part, to).await.map_err(|e| e.to_string())?;
        Ok(got)
    })
}

/// Download the library (unless the natural voice already brought it) and the model. Started
/// from Settings → Voice; progress goes out as `stt.install` events.
pub fn install(app: AppHandle) -> Result<(), String> {
    let Some((runtime_url, inner)) = runtime_package() else {
        return Err("Pixie's speech engine isn't available for this kind of computer yet.".into());
    };
    let s = shared();
    if s.installing.swap(true, Ordering::SeqCst) {
        return Err("Already downloading.".into());
    }
    let data = app.state::<AppState>().data_dir.clone();
    std::thread::spawn(move || {
        let r = (|| -> Result<(), String> {
            let (runtime, model) = (runtime_dir(&data), model_dir(&data));
            let need_runtime = !pixie_stt::ffi::library(&runtime).exists();
            // Sizes are known well enough for a progress bar: the model, plus the library.
            *s.progress.lock_ok() = (0, 73_000_000 + if need_runtime { 40_000_000 } else { 0 });
            let mut before = 0u64;
            if need_runtime {
                let stage = crate::kokoro::voices_dir(&data).join("dl-stt-runtime");
                let _ = std::fs::remove_dir_all(&stage);
                std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
                before += fetch(runtime_url, &stage.join("pkg.tar.bz2"), before, &app)?;
                emit(&app, Event::new("stt.install", json!({ "step": "unpack" })));
                let mut tar = std::process::Command::new("tar");
                tar.args(["-xjf", "pkg.tar.bz2"]).current_dir(&stage);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    tar.creation_flags(0x0800_0000);
                }
                if !tar.status().map(|s| s.success()).unwrap_or(false) {
                    return Err("couldn't unpack the speech library".into());
                }
                let _ = std::fs::remove_dir_all(&runtime);
                std::fs::rename(stage.join(inner), &runtime).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_dir_all(&stage);
            }
            std::fs::create_dir_all(&model).map_err(|e| e.to_string())?;
            for (remote, ours) in MODEL_FILES {
                let to = model.join(ours);
                if to.metadata().map(|m| m.len() > 0).unwrap_or(false) {
                    continue;
                }
                before += fetch(&format!("{MODEL_URL}/{remote}"), &to, before, &app)?;
            }
            if !installed(&data) {
                return Err("the download finished but files are missing".into());
            }
            Ok(())
        })();
        s.installing.store(false, Ordering::SeqCst);
        match r {
            Ok(()) => {
                log::info!("stt: Pixie's speech engine installed");
                emit(&app, Event::new("stt.install", json!({ "step": "done" })));
            }
            Err(e) => {
                log::warn!("stt: install failed: {e}");
                emit(&app, Event::new("stt.install", json!({ "step": "error", "error": e })));
            }
        }
    });
    Ok(())
}

/// Settings → Voice → remove the speech model (the library stays: the natural voice uses it).
pub fn remove(app: &AppHandle) {
    cancel(app);
    shared().engine.lock_ok().take();
    let _ = std::fs::remove_dir_all(model_dir(&app.state::<AppState>().data_dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sentence_starts_with_a_capital() {
        assert_eq!(sentence("send it by friday"), "Send it by friday");
        assert_eq!(sentence("I think so"), "I think so");
        assert_eq!(sentence(""), "");
        assert_eq!(sentence("élan"), "Élan");
    }

    #[test]
    fn every_supported_system_has_a_library_to_download() {
        if let Some((url, inner)) = runtime_package() {
            assert!(url.starts_with("https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/") && url.ends_with(".tar.bz2"));
            assert!(url.contains(inner));
        }
    }
}
