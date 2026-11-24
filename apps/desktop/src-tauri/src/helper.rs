//! Client for the Swift helper sidecar (`pixie-helper`): JSON lines over stdio.
//! It is restarted automatically if it exits.

use pixie_core::sync::LockOk;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

/// Set when Pixie quits, so the helper going with it isn't logged as a failure or restarted.
pub static QUITTING: AtomicBool = AtomicBool::new(false);

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct Helper {
    pub path: Option<PathBuf>,
    stdin: Mutex<Option<ChildStdin>>,
    child: Mutex<Option<Child>>,
    pending: Pending,
    next: AtomicU64,
    events: broadcast::Sender<(String, Value)>,
}

/// `pixie-helper` next to the main binary (bundle: Contents/MacOS; dev: target/debug).
pub fn locate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let p = dir.join("pixie-helper");
    if p.is_file() {
        return Some(p);
    }
    // `cargo run` without tauri's sidecar copy.
    let target = std::env::var("TARGET").unwrap_or_else(|_| "aarch64-apple-darwin".into());
    let alt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("binaries/pixie-helper-{target}"));
    alt.is_file().then_some(alt)
}

impl Helper {
    pub fn spawn() -> Arc<Self> {
        let (events, _) = broadcast::channel(512);
        let h = Arc::new(Self {
            path: locate(),
            stdin: Mutex::new(None),
            child: Mutex::new(None),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next: AtomicU64::new(1),
            events,
        });
        // Windows / Linux: the same commands are answered in-process (src/native).
        #[cfg(not(target_os = "macos"))]
        {
            crate::native::start(h.events.clone());
            return h;
        }
        #[allow(unreachable_code)]
        match &h.path {
            Some(p) => log::info!("helper: {}", p.display()),
            None => log::error!("helper: pixie-helper not found next to the app; voice and screen are off"),
        }
        let sup = h.clone();
        std::thread::Builder::new()
            .name("pixie-helper".into())
            .spawn(move || sup.supervise())
            .expect("helper thread");
        h
    }

    fn supervise(self: Arc<Self>) {
        let Some(path) = self.path.clone() else { return };
        let mut backoff = Duration::from_millis(300);
        loop {
            let mut cmd = Command::new(&path);
            if let Some(f) = pixie_tools::children::file() {
                cmd.env("PIXIE_CHILDREN", f);
            }
            let child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    log::error!("helper: spawn failed: {e}");
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            let stdout = child.stdout.take().expect("stdout");
            *self.stdin.lock_ok() = child.stdin.take();
            *self.child.lock_ok() = Some(child);
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(id) = v.get("id").and_then(Value::as_u64) {
                    if let Some(tx) = self.pending.lock_ok().remove(&id) {
                        let r = if v["ok"].as_bool().unwrap_or(false) {
                            Ok(v.get("result").cloned().unwrap_or(Value::Null))
                        } else {
                            Err(v["error"].as_str().unwrap_or("helper error").to_string())
                        };
                        let _ = tx.send(r);
                    }
                } else if let Some(ev) = v.get("event").and_then(Value::as_str) {
                    let _ = self.events.send((ev.to_string(), v.get("data").cloned().unwrap_or(Value::Null)));
                }
            }
            // Exited: fail everything in flight and restart.
            *self.stdin.lock_ok() = None;
            if let Some(mut c) = self.child.lock_ok().take() {
                let _ = c.wait();
            }
            for (_, tx) in self.pending.lock_ok().drain() {
                let _ = tx.send(Err("The helper restarted.".into()));
            }
            let _ = self.events.send(("restarted".into(), Value::Null));
            if QUITTING.load(Ordering::SeqCst) {
                // Pixie is quitting and took the helper with it: nothing to report or restart.
                return;
            }
            log::warn!("helper exited; restarting in {backoff:?}");
            // Noted a moment later, so a helper ended by Pixie quitting (or being replaced) isn't.
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(3));
                crate::heal::note("helper", "The screen and speech helper stopped", "Restarted it", "");
            });
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_secs(10));
        }
    }

    pub fn available(&self) -> bool {
        cfg!(not(target_os = "macos")) || self.path.is_some()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(String, Value)> {
        self.events.subscribe()
    }

    /// Fire a command and await its reply.
    pub async fn call(&self, cmd: &str, args: Value, timeout: Duration) -> Result<Value, String> {
        #[cfg(not(target_os = "macos"))]
        {
            return crate::native::call(cmd, args, timeout).await;
        }
        #[allow(unreachable_code)]
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock_ok().insert(id, tx);
        let line = json!({ "id": id, "cmd": cmd, "args": args }).to_string();
        {
            // The helper may be starting up; wait briefly for stdin.
            let mut tries = 0;
            loop {
                let sent = {
                    let mut guard = self.stdin.lock_ok();
                    guard.as_mut().is_some_and(|stdin| writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_ok())
                };
                if sent {
                    break;
                }
                tries += 1;
                if tries > 30 || !self.available() {
                    self.pending.lock_ok().remove(&id);
                    return Err("The native helper isn't running.".into());
                }
                // Not a thread sleep: this runs on the async workers every answer shares.
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(r)) => {
                if let Err(e) = &r {
                    // A permission turned off while Pixie runs: say which and open its page.
                    crate::watchdog::helper_error(e);
                }
                r
            }
            Ok(Err(_)) => Err("The helper went away.".into()),
            Err(_) => {
                self.pending.lock_ok().remove(&id);
                Err(format!("The helper didn't answer {cmd} in time."))
            }
        }
    }

    /// Fire and forget.
    pub fn send(self: &Arc<Self>, cmd: &'static str, args: Value) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = me.call(cmd, args, Duration::from_secs(10)).await {
                log::debug!("helper {cmd}: {e}");
            }
        });
    }
}

/// Kill the helper now so the supervisor starts a fresh one (a checkup found it not answering).
pub fn force_restart(app: &tauri::AppHandle) {
    use tauri::Manager;
    let st = app.state::<crate::AppState>();
    let mut guard = st.helper.child.lock_ok();
    if let Some(c) = guard.as_mut() {
        let _ = c.kill();
    }
}

/// Kill the helper once so the supervisor starts a fresh one (picks up new permissions).
pub fn restart(app: &tauri::AppHandle) {
    use tauri::Manager;
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    let st = app.state::<crate::AppState>();
    let mut guard = st.helper.child.lock_ok();
    if let Some(c) = guard.as_mut() {
        let _ = c.kill();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.lock_ok().take() {
            let _ = c.kill();
        }
    }
}
