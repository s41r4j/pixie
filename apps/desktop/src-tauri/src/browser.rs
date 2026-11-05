//! Bridge to the user's Chrome (M7): the extension talks to the native host, which relays to
//! this unix socket. One JSON object per line each way; requests carry an `id`.

use crate::AppState;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{mpsc, oneshot};

struct Bridge {
    tx: Mutex<Option<mpsc::UnboundedSender<String>>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    next: AtomicU64,
    hello: AtomicBool,
}

fn bridge() -> &'static Bridge {
    static B: OnceLock<Bridge> = OnceLock::new();
    B.get_or_init(|| Bridge { tx: Mutex::new(None), pending: Mutex::new(HashMap::new()), next: AtomicU64::new(1), hello: AtomicBool::new(false) })
}

pub fn connected(_app: &AppHandle) -> bool {
    bridge().hello.load(Ordering::SeqCst)
}

/// Copy the extension next to the app data and register the native host with each browser.
fn install(app: &AppHandle, data: &Path) {
    let src = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("extension"))
        .filter(|d| d.join("manifest.json").exists())
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../extension")));
    let dest = data.join("extension");
    if let Ok(rd) = std::fs::read_dir(&src) {
        let _ = std::fs::create_dir_all(dest.join("icons"));
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                let _ = std::fs::copy(&p, dest.join(e.file_name()));
            } else if p.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&p) {
                    let _ = std::fs::create_dir_all(dest.join(e.file_name()));
                    for f in sub.flatten() {
                        let _ = std::fs::copy(f.path(), dest.join(e.file_name()).join(f.file_name()));
                    }
                }
            }
        }
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let manifest = openclicky_native_host::host_manifest(&exe);
    for dir in openclicky_native_host::manifest_dirs() {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.json", openclicky_native_host::HOST_NAME));
        if std::fs::read_to_string(&path).ok().as_deref() != Some(manifest.as_str()) {
            match std::fs::write(&path, &manifest) {
                Ok(()) => log::info!("browser: registered native host in {}", dir.display()),
                Err(e) => log::warn!("browser: {}: {e}", dir.display()),
            }
        }
    }
}

pub fn start(app: AppHandle) {
    let data = app.state::<AppState>().data_dir.clone();
    install(&app, &data);
    let sock = data.join("browser.sock");
    tauri::async_runtime::spawn(async move {
        let _ = std::fs::remove_file(&sock);
        let listener = match UnixListener::bind(&sock) {
            Ok(l) => l,
            Err(e) => {
                log::error!("browser: can't listen on {}: {e}", sock.display());
                return;
            }
        };
        loop {
            let Ok((stream, _)) = listener.accept().await else { continue };
            log::info!("browser: native host connected");
            let (rd, mut wr) = stream.into_split();
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            *bridge().tx.lock().unwrap() = Some(tx);
            tauri::async_runtime::spawn(async move {
                while let Some(line) = rx.recv().await {
                    if wr.write_all(line.as_bytes()).await.is_err() || wr.write_all(b"\n").await.is_err() {
                        break;
                    }
                }
            });
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(rd).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                    match v["type"].as_str().unwrap_or("") {
                        "hello" => {
                            bridge().hello.store(true, Ordering::SeqCst);
                            log::info!("browser: extension {} connected", v["version"]);
                            crate::emit(&app2, openclicky_core::Event::new("browser.connected", json!({ "connected": true })));
                        }
                        "result" => {
                            if let Some(id) = v["id"].as_u64() {
                                if let Some(tx) = bridge().pending.lock().unwrap().remove(&id) {
                                    let r = match v.get("error").and_then(Value::as_str) {
                                        Some(e) => Err(e.to_string()),
                                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                                    };
                                    let _ = tx.send(r);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                bridge().hello.store(false, Ordering::SeqCst);
                *bridge().tx.lock().unwrap() = None;
                for (_, tx) in bridge().pending.lock().unwrap().drain() {
                    let _ = tx.send(Err("The browser disconnected.".into()));
                }
                log::info!("browser: disconnected");
                crate::emit(&app2, openclicky_core::Event::new("browser.connected", json!({ "connected": false })));
            });
        }
    });
}

/// The `browser` tool.
pub async fn call(app: &AppHandle, input: &Value) -> Result<String, String> {
    let tx = bridge().tx.lock().unwrap().clone();
    let Some(tx) = tx.filter(|_| connected(app)) else {
        return Err("Their browser isn't connected. They can install the openclicky extension from Settings → Connectors.".into());
    };
    let id = bridge().next.fetch_add(1, Ordering::Relaxed);
    let mut req = input.clone();
    req["id"] = json!(id);
    let (otx, orx) = oneshot::channel();
    bridge().pending.lock().unwrap().insert(id, otx);
    tx.send(req.to_string()).map_err(|_| "The browser disconnected.".to_string())?;
    let limit = if input["operation"] == "batch" { 35 } else { 25 };
    let result = match tokio::time::timeout(Duration::from_secs(limit), orx).await {
        Ok(Ok(r)) => r?,
        Ok(Err(_)) => return Err("The browser went away.".into()),
        Err(_) => {
            bridge().pending.lock().unwrap().remove(&id);
            return Err("The browser didn't answer in time.".into());
        }
    };
    let mut text = result["text"].as_str().map(str::to_string).unwrap_or_else(|| result.to_string());
    if let Some(b64) = result["screenshot"].as_str() {
        let dir = app.state::<AppState>().data_dir.join("session/tmp");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("browser-{id}.png"));
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            let _ = std::fs::write(&path, bytes);
            text.push_str(&format!("\nSaved to {} (use view_image to look at it).", path.display()));
        }
    }
    if text.chars().count() > 50_000 {
        text = format!("{}…[cut]", text.chars().take(50_000).collect::<String>());
    }
    Ok(text)
}
