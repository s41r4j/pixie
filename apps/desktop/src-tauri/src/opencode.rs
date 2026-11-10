//! opencode as Pixie's model: free models (and any provider the user connected in opencode)
//! with no API key in Pixie.
//!
//! - **The server.** `opencode serve` on 127.0.0.1 with a random password, started when it's
//!   needed (they start talking, a job starts, Settings → Models opens) and stopped after
//!   [`IDLE`] quiet (it takes ~400 MB). It runs with its own config folder and database
//!   (`<app_data>/opencode/`), so the user's own opencode setup, MCP servers and sessions stay out
//!   and Pixie's sessions never show up in their opencode. Their provider sign-ins (opencode's
//!   auth) still apply.
//! - **Pixie's tools.** opencode is an agent with its own tools; those are all denied. Pixie's
//!   tools are served to it over a small MCP endpoint inside Pixie (localhost, random token).
//!   Every call runs through `openclicky_tools::execute` with the turn's own `ToolCtx`, exactly
//!   as with an API model; calls are routed by opencode's session id.
//! - **A turn.** One opencode session per turn (or job run), Pixie's system prompt set as that
//!   session's instructions, earlier conversation and the screenshot in the prompt. Text deltas
//!   stream back as `AgentEvent::Text`, so speech starts on the first sentence.

use crate::AppState;
use openclicky_llm::{Content, Effort, LlmError, Message, OnEvent, Provider, Request, Response, Role, StopReason, StreamEvent, ToolDef};
use openclicky_tools::{AgentEvent, AgentOptions, AgentOutcome, ToolCtx};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Stop the server after this long without a turn.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// The model used when none is picked: free, takes images and tools.
pub const DEFAULT_MODEL: &str = "opencode/deepseek-v4.1-flash-free";

struct Server {
    child: tokio::process::Child,
    base: String,
    auth: String,
}

/// (takes images, free, variants)
type Caps = (bool, bool, Vec<String>);

struct Turn {
    ctx: ToolCtx,
    allowed: Vec<String>,
    events: tokio::sync::mpsc::UnboundedSender<AgentEvent>,
}

#[derive(Default)]
struct Shared {
    server: tokio::sync::Mutex<Option<Server>>,
    /// The MCP endpoint: (port, token), started once.
    mcp: OnceLock<(u16, String)>,
    turns: Mutex<HashMap<String, Turn>>,
    last_used: Mutex<Option<Instant>>,
    /// Model id → what it can do, from the server's model list.
    models: Mutex<HashMap<String, Caps>>,
}

fn shared() -> &'static Shared {
    static S: OnceLock<Shared> = OnceLock::new();
    S.get_or_init(Shared::default)
}

fn app_handle() -> &'static OnceLock<AppHandle> {
    static A: OnceLock<AppHandle> = OnceLock::new();
    &A
}

/// A server from an earlier run that didn't get to clean up (Pixie crashed or was killed).
fn kill_leftover(data: &std::path::Path) {
    let file = data.join("server.pid");
    let Some(pid) = std::fs::read_to_string(&file).ok().and_then(|p| p.trim().parse::<u32>().ok()) else { return };
    let _ = std::fs::remove_file(&file);
    #[cfg(unix)]
    {
        // The watcher shell and the server under it.
        let _ = std::process::Command::new("/usr/bin/pkill").args(["-P", &pid.to_string()]).status();
        let _ = std::process::Command::new("/bin/kill").arg(pid.to_string()).status();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).status();
    }
}

/// Pixie is quitting: stop the server now.
pub fn on_quit() {
    if let Some(app) = app_handle().get() {
        kill_leftover(&app.state::<AppState>().data_dir.join("opencode"));
    }
}

/// At launch: remember the app and start the idle watcher (nothing else starts yet).
pub fn init(app: &AppHandle) {
    let _ = app_handle().set(app.clone());
    kill_leftover(&app.state::<AppState>().data_dir.join("opencode"));
    if selected(app) {
        // The provider was made before the app handle existed; now its label knows the model.
        app.state::<AppState>().reload_provider();
    }
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let s = shared();
            let quiet = s.last_used.lock().unwrap().map(|t| t.elapsed() > IDLE).unwrap_or(true);
            if quiet && s.turns.lock().unwrap().is_empty() {
                if let Some(mut srv) = s.server.lock().await.take() {
                    log::info!("opencode: quiet for a while, stopping the server");
                    // The server under the watcher first, then the watcher.
                    if let Some(app) = app_handle().get() {
                        kill_leftover(&app.state::<AppState>().data_dir.join("opencode"));
                    }
                    let _ = srv.child.kill().await;
                }
            }
        }
    });
}

/// The opencode CLI on this computer, if any.
pub fn binary() -> Option<PathBuf> {
    openclicky_tools::coders::detect().into_iter().find(|c| c.name == "opencode").map(|c| c.path)
}

/// opencode is the chosen model provider (and installed).
pub fn selected(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.lock().map(|s| s.provider == "opencode").unwrap_or(false) && binary().is_some()
}

/// "provider/model" for voice turns (fast) or agents (deep).
pub fn model_for(app: &AppHandle, effort: Effort) -> String {
    let s = app.state::<AppState>().settings.lock().map(|s| s.models.get("opencode").cloned()).ok().flatten();
    let pick = s.map(|m| if effort == Effort::Fast { m.fast } else { m.deep }).unwrap_or_default();
    if pick.trim().is_empty() {
        DEFAULT_MODEL.into()
    } else {
        pick
    }
}

fn split_model(m: &str) -> (String, String) {
    match m.split_once('/') {
        Some((p, id)) => (p.to_string(), id.to_string()),
        None => ("opencode".into(), m.to_string()),
    }
}

fn touch() {
    *shared().last_used.lock().unwrap() = Some(Instant::now());
}

/// Load the server now if opencode is the model (they started talking).
pub fn warm(app: &AppHandle) {
    if selected(app) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = ensure(&app).await {
                log::warn!("opencode: {e}");
            }
        });
    }
}

fn random_hex(n: usize) -> String {
    (0..n).map(|_| format!("{:02x}", rand::random::<u8>())).collect()
}

/// The running server's (base URL, auth header), starting it if needed.
async fn ensure(app: &AppHandle) -> Result<(String, String), String> {
    touch();
    let s = shared();
    let mut guard = s.server.lock().await;
    if let Some(srv) = guard.as_mut() {
        if matches!(srv.child.try_wait(), Ok(None)) {
            return Ok((srv.base.clone(), srv.auth.clone()));
        }
        log::warn!("opencode: the server stopped; starting it again");
        *guard = None;
    }
    let bin = binary().ok_or("opencode isn't installed (npm i -g opencode-ai, or brew install opencode).")?;
    let (mcp_port, mcp_token) = mcp_endpoint(app).await?;
    let data = app.state::<AppState>().data_dir.join("opencode");
    for d in ["config", "work"] {
        std::fs::create_dir_all(data.join(d)).map_err(|e| e.to_string())?;
    }
    let password = random_hex(24);
    let config = json!({
        "autoupdate": false,
        "share": "disabled",
        "mcp": { "pixie": { "type": "remote", "url": format!("http://127.0.0.1:{mcp_port}/mcp"), "headers": { "authorization": format!("Bearer {mcp_token}") }, "timeout": 600_000 } },
        "agent": {
            "title": { "disable": true },
            "pixie": {
                "description": "Pixie",
                "mode": "primary",
                "prompt": "You are Pixie. Follow the instructions you are given for this session exactly.",
                // opencode's own tools (shell, edits, web…) are off; only Pixie's tools run.
                "permission": { "*": "deny", "pixie_*": "allow" }
            }
        }
    });
    // On Mac and Linux a tiny shell watches Pixie and ends the server when Pixie is gone (a
    // force-quit skips Pixie's own cleanup, and the server would keep its ~400 MB).
    #[cfg(unix)]
    let mut cmd = {
        let mut c = tokio::process::Command::new("/bin/sh");
        c.arg("-c")
            .arg(r#"p=$1; shift; "$@" & c=$!; trap 'kill $c 2>/dev/null' EXIT INT TERM; while kill -0 "$p" 2>/dev/null && kill -0 "$c" 2>/dev/null; do sleep 2; done"#)
            .arg("pixie-opencode")
            .arg(std::process::id().to_string())
            .arg(&bin);
        c
    };
    #[cfg(not(unix))]
    let mut cmd = tokio::process::Command::new(&bin);
    cmd.args(["serve", "--hostname", "127.0.0.1", "--port", "0"])
        .current_dir(data.join("work"))
        .env("XDG_CONFIG_HOME", data.join("config"))
        .env("OPENCODE_DB", data.join("pixie.db"))
        .env("OPENCODE_CONFIG_CONTENT", config.to_string())
        .env("OPENCODE_SERVER_PASSWORD", &password)
        .env("OPENCODE_DISABLE_PROJECT_CONFIG", "1")
        .env("OPENCODE_DISABLE_CLAUDE_CODE", "1")
        .env("OPENCODE_DISABLE_CLAUDE_CODE_SKILLS", "1")
        .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
        .env("OPENCODE_DISABLE_FILEWATCHER", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    // Keys Pixie already has, so those providers' models also work through opencode (opencode
    // reads them from the environment; apps started by macOS don't inherit a shell's).
    {
        let st = app.state::<AppState>();
        let settings = st.settings.lock().unwrap().clone();
        for (provider, var) in [("deepseek", "DEEPSEEK_API_KEY"), ("claude", "ANTHROPIC_API_KEY"), ("openai", "OPENAI_API_KEY")] {
            if let Some(k) = crate::config::key_for(&st.data_dir, &settings, provider) {
                cmd.env(var, k);
            }
        }
    }
    let t0 = Instant::now();
    kill_leftover(&data);
    let mut child = cmd.spawn().map_err(|e| format!("couldn't start opencode: {e}"))?;
    if let Some(pid) = child.id() {
        let _ = std::fs::write(data.join("server.pid"), pid.to_string());
    }
    let out = child.stdout.take().ok_or("no output from opencode")?;
    let mut lines = BufReader::new(out).lines();
    let base = tokio::time::timeout(Duration::from_secs(25), async {
        while let Ok(Some(l)) = lines.next_line().await {
            if let Some(url) = l.trim().strip_prefix("server listening on ") {
                return Some(url.trim().to_string());
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
    .ok_or("opencode didn't start (no \"server listening\" line in 25 s)")?;
    // Keep draining its output so it never blocks.
    tauri::async_runtime::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
    let auth = format!("Basic {}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("opencode:{password}")));
    // It listens before its providers are loaded; wait for the model list (else "model unavailable").
    let ready = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < ready {
        if list_models(&base, &auth).await.is_ok_and(|m| !m.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    log::info!("opencode: server ready at {base} in {} ms", t0.elapsed().as_millis());
    // Pixie's tools must be connected before the first session, or that session has none.
    let mut mcp = String::new();
    while tokio::time::Instant::now() < ready + Duration::from_secs(5) {
        let m = call(&base, &auth, reqwest::Method::GET, "/api/mcp", None).await.unwrap_or(Value::Null);
        mcp = m["data"].as_array().and_then(|a| a.iter().find(|x| x["name"] == "pixie")).map(|x| x["status"]["status"].as_str().unwrap_or("").to_string()).unwrap_or_default();
        if mcp != "pending" && !mcp.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    log::info!("opencode: Pixie's tools {mcp} after {} ms", t0.elapsed().as_millis());
    *guard = Some(Server { child, base: base.clone(), auth: auth.clone() });
    Ok((base, auth))
}

fn http() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| reqwest::Client::builder().connect_timeout(Duration::from_secs(5)).build().expect("http"))
}

async fn call(base: &str, auth: &str, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value, String> {
    let mut r = http().request(method, format!("{base}{path}")).header("authorization", auth).timeout(Duration::from_secs(60));
    if let Some(b) = body {
        r = r.json(&b);
    }
    let resp = r.send().await.map_err(|e| format!("opencode: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["message"].as_str().map(str::to_string)).unwrap_or(text);
        return Err(format!("opencode answered {status}: {msg}"));
    }
    Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::Null) })
}

/// The models opencode offers, newest first: id "provider/model", name, free, images, tools.
pub async fn refresh_models(app: &AppHandle) -> Result<Vec<Value>, String> {
    let (base, auth) = ensure(app).await?;
    list_models(&base, &auth).await
}

async fn list_models(base: &str, auth: &str) -> Result<Vec<Value>, String> {
    let v = call(base, auth, reqwest::Method::GET, "/api/model", None).await?;
    let mut out = Vec::new();
    let mut caps = HashMap::new();
    for m in v["data"].as_array().into_iter().flatten() {
        let id = format!("{}/{}", m["providerID"].as_str().unwrap_or(""), m["id"].as_str().unwrap_or(""));
        let cost = m["cost"].as_array().and_then(|c| c.first()).cloned().unwrap_or(Value::Null);
        let free = cost["input"].as_f64() == Some(0.0) && cost["output"].as_f64() == Some(0.0);
        let images = m["capabilities"]["input"].as_array().is_some_and(|a| a.iter().any(|x| x == "image"));
        let tools = m["capabilities"]["tools"].as_bool().unwrap_or(false);
        if m["status"].as_str().is_some_and(|s| s != "active") || m["enabled"] == false {
            continue;
        }
        let variants: Vec<String> = m["variants"].as_array().into_iter().flatten().filter_map(|v| v["id"].as_str().map(str::to_string)).collect();
        caps.insert(id.clone(), (images, free, variants));
        out.push(json!({
            "id": id, "name": m["name"], "provider": m["providerID"], "free": free, "images": images, "tools": tools,
            "context": m["limit"]["context"], "released": m["time"]["released"],
        }));
    }
    out.sort_by(|a, b| b["free"].as_bool().cmp(&a["free"].as_bool()).then(b["released"].as_i64().cmp(&a["released"].as_i64())));
    *shared().models.lock().unwrap() = caps;
    Ok(out)
}

/// Settings → Models → On this computer.
pub async fn status(app: &AppHandle, start: bool) -> Value {
    let Some(bin) = binary() else {
        return json!({ "installed": false });
    };
    let version = tokio::process::Command::new(&bin).arg("--version").output().await.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let running = shared().server.lock().await.is_some();
    let mut v = json!({ "installed": true, "path": bin, "version": version, "running": running, "selected": selected(app) });
    if start || running {
        match refresh_models(app).await {
            Ok(m) => v["models"] = json!(m),
            Err(e) => v["error"] = json!(e),
        }
    }
    v
}

// ---- Pixie's tools over MCP --------------------------------------------------------------------

/// Every tool any mode may use; a turn only runs the ones it was given.
fn all_tools(app: &AppHandle) -> Vec<ToolDef> {
    use openclicky_core::prompt::Mode;
    let host = crate::host::AppHost { app: app.clone() };
    let browser = crate::browser::connected(app);
    let mut out: Vec<ToolDef> = Vec::new();
    for mode in [Mode::Voice, Mode::Live, Mode::Job] {
        for t in openclicky_tools::defs::for_mode(mode, browser).into_iter().chain(openclicky_tools::Host::extra_tools(&host, mode)) {
            if !out.iter().any(|x| x.name == t.name) {
                out.push(t);
            }
        }
    }
    out
}

async fn mcp_endpoint(app: &AppHandle) -> Result<(u16, String), String> {
    if let Some(e) = shared().mcp.get() {
        return Ok(e.clone());
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = random_hex(24);
    let _ = shared().mcp.set((port, token.clone()));
    let app = app.clone();
    let tok = token.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let (app, tok) = (app.clone(), tok.clone());
            tauri::async_runtime::spawn(async move {
                let _ = serve_conn(app, sock, tok).await;
            });
        }
    });
    log::info!("opencode: Pixie's tools on 127.0.0.1:{port}");
    Ok((port, token))
}

/// One HTTP/1.1 connection (keep-alive): POST /mcp with JSON-RPC.
async fn serve_conn(app: AppHandle, sock: tokio::net::TcpStream, token: String) -> std::io::Result<()> {
    let (r, mut w) = sock.into_split();
    let mut r = BufReader::new(r);
    loop {
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).await? == 0 {
                return Ok(());
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            head.push_str(&line);
        }
        let mut lines = head.lines();
        let request_line = lines.next().unwrap_or("");
        let method = request_line.split_whitespace().next().unwrap_or("").to_string();
        let mut len = 0usize;
        let mut auth = String::new();
        for l in lines {
            if let Some((k, v)) = l.split_once(':') {
                match k.trim().to_ascii_lowercase().as_str() {
                    "content-length" => len = v.trim().parse().unwrap_or(0),
                    "authorization" => auth = v.trim().to_string(),
                    _ => {}
                }
            }
        }
        let mut body = vec![0u8; len.min(8 << 20)];
        r.read_exact(&mut body).await?;
        let (status, out) = if auth != format!("Bearer {token}") {
            ("401 Unauthorized", None)
        } else if method == "POST" {
            match serde_json::from_slice::<Value>(&body) {
                Ok(v) => match rpc(&app, v).await {
                    Some(resp) => ("200 OK", Some(resp)),
                    None => ("202 Accepted", None),
                },
                Err(_) => ("400 Bad Request", None),
            }
        } else if method == "DELETE" {
            ("200 OK", None)
        } else {
            // No server-to-client stream.
            ("405 Method Not Allowed", None)
        };
        let payload = out.map(|v| v.to_string()).unwrap_or_default();
        let resp = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n{payload}",
            payload.len()
        );
        w.write_all(resp.as_bytes()).await?;
        w.flush().await?;
    }
}

/// A JSON-RPC message (or batch) → the reply, None for notifications.
async fn rpc(app: &AppHandle, v: Value) -> Option<Value> {
    if let Some(batch) = v.as_array() {
        let mut out = Vec::new();
        for m in batch {
            if let Some(r) = Box::pin(rpc(app, m.clone())).await {
                out.push(r);
            }
        }
        return (!out.is_empty()).then_some(Value::Array(out));
    }
    log::debug!("opencode mcp: {}", v["method"].as_str().unwrap_or("?"));
    let id = v.get("id").cloned()?;
    let result = match v["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": v["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "pixie", "version": app.package_info().version.to_string() },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": all_tools(app).into_iter().map(|t| json!({ "name": t.name, "description": t.description, "inputSchema": t.input_schema })).collect::<Vec<_>>()
        })),
        "tools/call" => Ok(tool_call(&v["params"]).await),
        m => Err(json!({ "code": -32601, "message": format!("unknown method {m}") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

async fn tool_call(params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or("").to_string();
    let input = params.get("arguments").cloned().unwrap_or(json!({}));
    let session = params["_meta"]["ai.opencode/sessionID"].as_str().unwrap_or("").to_string();
    let found = {
        let turns = shared().turns.lock().unwrap();
        turns.get(&session).map(|t| (t.ctx.clone(), t.allowed.contains(&name), t.events.clone()))
    };
    let err = |m: String| json!({ "content": [{ "type": "text", "text": m }], "isError": true });
    let Some((ctx, allowed, events)) = found else {
        return err("This session isn't one of Pixie's turns.".into());
    };
    if !allowed {
        return err(format!("{name} isn't available here."));
    }
    touch();
    let _ = events.send(AgentEvent::ToolStart { name: name.clone(), input: input.clone() });
    let out = openclicky_tools::execute(&ctx, &name, &input).await;
    let summary = out.text.lines().next().unwrap_or("").chars().take(120).collect::<String>();
    let _ = events.send(AgentEvent::ToolEnd { name, ok: !out.is_error, summary });
    let mut content = vec![json!({ "type": "text", "text": if out.text.is_empty() { "done".to_string() } else { out.text } })];
    for (media, data) in out.images {
        content.push(json!({ "type": "image", "data": data, "mimeType": media }));
    }
    json!({ "content": content, "isError": out.is_error })
}

// ---- a turn ------------------------------------------------------------------------------------

/// For voice turns: the model's variant with the least thinking (none, minimal or low), so
/// speech starts sooner.
fn quick_variant(model: &str) -> Option<String> {
    let caps = shared().models.lock().unwrap();
    let variants = &caps.get(model)?.2;
    ["none", "minimal", "low"].iter().find(|v| variants.iter().any(|x| x == *v)).map(|v| v.to_string())
}

/// Messages → the prompt text (earlier conversation first) and the attached images.
fn flatten(messages: &[Message]) -> (String, Vec<Value>) {
    let text_of = |m: &Message| {
        m.content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text.clone()),
                Content::ToolResult { content, .. } => {
                    let t: String = content.iter().filter_map(|x| if let Content::Text { text } = x { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join(" ");
                    Some(format!("[tool result: {}]", t.chars().take(400).collect::<String>()))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let Some((last, earlier)) = messages.split_last() else { return (String::new(), vec![]) };
    let mut out = String::new();
    let lines: Vec<String> = earlier
        .iter()
        .filter_map(|m| {
            let t = text_of(m);
            (!t.trim().is_empty()).then(|| format!("{}: {}", if m.role == Role::User { "Them" } else { "You" }, t.trim()))
        })
        .collect();
    if !lines.is_empty() {
        out.push_str("EARLIER IN THIS CONVERSATION\n");
        out.push_str(&lines.join("\n"));
        out.push_str("\n\n");
    }
    out.push_str(&text_of(last));
    let files = last
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Image { source } => Some(json!({ "uri": format!("data:{};base64,{}", source.media_type, source.data), "name": "screen.jpg" })),
            _ => None,
        })
        .collect();
    (out, files)
}

fn record_usage(app: &AppHandle, model: &str, kind: &str, tokens: &Value) {
    let line = crate::usage::Line {
        ts: chrono::Utc::now().timestamp_millis(),
        provider: "opencode".into(),
        model: model.into(),
        kind: kind.into(),
        input: tokens["input"].as_u64().unwrap_or(0),
        output: tokens["output"].as_u64().unwrap_or(0) + tokens["reasoning"].as_u64().unwrap_or(0),
        cache_read: tokens["cache"]["read"].as_u64().unwrap_or(0),
        cache_write: tokens["cache"]["write"].as_u64().unwrap_or(0),
    };
    let path = app.state::<AppState>().data_dir.join("usage.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", serde_json::to_string(&line).unwrap_or_default());
    }
}

/// The friendly version of opencode's error.
fn explain(err: &Value) -> String {
    let kind = err["type"].as_str().unwrap_or("");
    let msg = err["message"].as_str().unwrap_or("opencode couldn't answer");
    if kind.contains("free-limit") {
        return "opencode's free models have hit their usage limit for now. Pick another model in Settings → Models, connect a provider in opencode, or add an API key.".into();
    }
    msg.to_string()
}

/// Run a turn (or a job run) on opencode: like `openclicky_tools::run`, but opencode runs the
/// tool loop and calls Pixie's tools back over MCP.
pub async fn run(
    app: &AppHandle,
    system: &str,
    messages: Vec<Message>,
    tools: Vec<ToolDef>,
    ctx: &ToolCtx,
    opts: &AgentOptions,
    on_event: &mut (dyn FnMut(AgentEvent) + Send),
) -> Result<AgentOutcome, LlmError> {
    let api = |e: String| LlmError::Api { status: 500, message: e };
    let (base, auth) = ensure(app).await.map_err(api)?;
    let model = model_for(app, opts.effort);
    let (provider, id) = split_model(&model);
    let mut model_ref = json!({ "providerID": provider, "id": id });
    if let Some(v) = quick_variant(&model).filter(|_| opts.effort == Effort::Fast) {
        model_ref["variant"] = json!(v);
    }
    let s = call(&base, &auth, reqwest::Method::POST, "/api/session", Some(json!({ "agent": "pixie", "model": model_ref })))
        .await
        .map_err(api)?;
    let sid = s["data"]["id"].as_str().ok_or_else(|| api("opencode didn't create a session".into()))?.to_string();
    call(&base, &auth, reqwest::Method::PUT, &format!("/api/experimental/session/{sid}/instructions/entries/pixie"), Some(json!({ "value": system })))
        .await
        .map_err(api)?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    shared().turns.lock().unwrap().insert(sid.clone(), Turn { ctx: ctx.clone(), allowed: tools.iter().map(|t| t.name.clone()).collect(), events: tx });
    let result = drive(app, &base, &auth, &sid, &model, &messages, opts, ctx, &mut rx, on_event).await;
    shared().turns.lock().unwrap().remove(&sid);
    touch();
    // Pixie keeps its own history; the session isn't needed again.
    let _ = call(&base, &auth, reqwest::Method::DELETE, &format!("/api/session/{sid}"), None).await;
    let (final_text, steps, hit_limit) = result?;
    let mut messages = messages;
    messages.push(Message::assistant(vec![Content::text(final_text.clone())]));
    Ok(AgentOutcome { messages, final_text, steps, hit_limit })
}

#[allow(clippy::too_many_arguments)]
async fn drive(
    app: &AppHandle,
    base: &str,
    auth: &str,
    sid: &str,
    model: &str,
    messages: &[Message],
    opts: &AgentOptions,
    ctx: &ToolCtx,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AgentEvent>,
    on_event: &mut (dyn FnMut(AgentEvent) + Send),
) -> Result<(String, usize, bool), LlmError> {
    let api = |e: String| LlmError::Api { status: 500, message: e };
    // Listen before sending so no event is missed.
    let mut events = http().get(format!("{base}/api/event")).header("authorization", auth).header("accept", "text/event-stream").send().await.map_err(|e| api(e.to_string()))?;
    let (text, files) = flatten(messages);
    let mut body = json!({ "text": text });
    if !files.is_empty() {
        body["files"] = json!(files);
    }
    call(base, auth, reqwest::Method::POST, &format!("/api/session/{sid}/prompt"), Some(body)).await.map_err(api)?;
    let kind = if opts.effort == Effort::Fast { "voice" } else { "job" };
    let mut buf = String::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut steps = 0usize;
    let mut usage = Value::Null;
    let mut hit_limit = false;
    let deadline = tokio::time::Instant::now() + if opts.effort == Effort::Fast { Duration::from_secs(180) } else { Duration::from_secs(3600) };
    let outcome: Result<(), LlmError> = loop {
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                let _ = call(base, auth, reqwest::Method::POST, &format!("/api/session/{sid}/interrupt"), Some(json!({}))).await;
                break Err(LlmError::Cancelled);
            }
            _ = tokio::time::sleep_until(deadline) => {
                let _ = call(base, auth, reqwest::Method::POST, &format!("/api/session/{sid}/interrupt"), Some(json!({}))).await;
                break Err(api("opencode took too long to answer".into()));
            }
            Some(ev) = rx.recv() => on_event(ev),
            chunk = events.chunk() => {
                let Ok(Some(chunk)) = chunk else { break Err(api("lost the connection to opencode".into())) };
                buf.push_str(&String::from_utf8_lossy(&chunk));
                let mut done = None;
                while let Some(nl) = buf.find('\n') {
                    let line: String = buf.drain(..=nl).collect();
                    let Some(data) = line.trim().strip_prefix("data:") else { continue };
                    let Ok(e) = serde_json::from_str::<Value>(data.trim()) else { continue };
                    let d = &e["data"];
                    if d["sessionID"].as_str() != Some(sid) {
                        continue;
                    }
                    match e["type"].as_str().unwrap_or("") {
                        "session.text.delta" => {
                            let delta = d["delta"].as_str().unwrap_or("").to_string();
                            let msg = d["assistantMessageID"].as_str().unwrap_or("").to_string();
                            match texts.last_mut() {
                                Some((m, t)) if *m == msg => t.push_str(&delta),
                                _ => texts.push((msg, delta.clone())),
                            }
                            on_event(AgentEvent::Text(delta));
                        }
                        "session.step.ended" => {
                            steps += 1;
                            on_event(AgentEvent::Step { n: steps });
                            if steps >= opts.max_steps && d["finish"].as_str() == Some("tool-calls") {
                                hit_limit = true;
                                let _ = call(base, auth, reqwest::Method::POST, &format!("/api/session/{sid}/interrupt"), Some(json!({}))).await;
                                done = Some(Ok(()));
                            }
                        }
                        "session.usage.updated" => usage = d["tokens"].clone(),
                        "session.execution.succeeded" => done = Some(Ok(())),
                        "session.execution.failed" => {
                            let status = d["error"]["status"].as_u64().unwrap_or(500) as u16;
                            done = Some(Err(LlmError::Api { status, message: explain(&d["error"]) }));
                        }
                        _ => {}
                    }
                    if done.is_some() {
                        break;
                    }
                }
                if let Some(d) = done {
                    // Tool events still queued.
                    while let Ok(ev) = rx.try_recv() {
                        on_event(ev);
                    }
                    break d;
                }
            }
        }
    };
    if !usage.is_null() {
        record_usage(app, model, kind, &usage);
    }
    outcome?;
    let final_text = texts.into_iter().rev().map(|(_, t)| t).find(|t| !t.trim().is_empty()).unwrap_or_default();
    Ok((final_text.trim().to_string(), steps.max(1), hit_limit))
}

/// The agent loop on whichever brain is chosen: opencode when it's the model, else the API
/// provider with Pixie's own tool loop.
#[allow(clippy::too_many_arguments)]
pub async fn run_agent(
    app: &AppHandle,
    provider: &dyn Provider,
    system: &str,
    messages: Vec<Message>,
    tools: Vec<ToolDef>,
    ctx: &ToolCtx,
    opts: &AgentOptions,
    on_event: &mut (dyn FnMut(AgentEvent) + Send),
) -> Result<AgentOutcome, LlmError> {
    if selected(app) {
        run(app, system, messages, tools, ctx, opts, on_event).await
    } else {
        openclicky_tools::run(provider, system, messages, tools, ctx, opts, on_event).await
    }
}

// ---- the provider (plain text calls: summaries, memory) -----------------------------------------

/// Pixie's provider when opencode is the model. Tool loops go through [`run_agent`]; this
/// answers the plain text calls (today's chat summary, learning memory notes).
pub struct OpencodeProvider;

impl OpencodeProvider {
    fn app() -> Option<&'static AppHandle> {
        app_handle().get()
    }
}

#[async_trait::async_trait]
impl Provider for OpencodeProvider {
    fn label(&self) -> String {
        let m = Self::app().map(|a| model_for(a, Effort::Fast)).unwrap_or_else(|| DEFAULT_MODEL.into());
        format!("opencode · {m}")
    }

    fn supports_images(&self) -> bool {
        self.sees_images_at(Effort::Fast)
    }

    fn sees_images_at(&self, effort: Effort) -> bool {
        let Some(app) = Self::app() else { return false };
        let m = model_for(app, effort);
        shared().models.lock().unwrap().get(&m).map(|c| c.0).unwrap_or(m.contains("flash") || m.contains("vl"))
    }

    async fn complete(&self, req: &Request, on_event: OnEvent<'_>) -> Result<Response, LlmError> {
        let app = Self::app().ok_or(LlmError::Decode("opencode isn't ready".into()))?;
        let api = |e: String| LlmError::Api { status: 500, message: e };
        let (base, auth) = ensure(app).await.map_err(api)?;
        let model = req.model.clone().unwrap_or_else(|| model_for(app, req.effort));
        let (provider, id) = split_model(&model);
        let (text, _) = flatten(&req.messages);
        let prompt = format!("{}\n\n{text}", req.system);
        let v = call(&base, &auth, reqwest::Method::POST, "/api/experimental/generate/text", Some(json!({ "prompt": prompt, "model": { "providerID": provider, "id": id } })))
            .await
            .map_err(api)?;
        let out = v["data"]["text"].as_str().unwrap_or("").to_string();
        on_event(StreamEvent::TextDelta(out.clone()));
        Ok(Response { content: vec![Content::text(out)], stop_reason: StopReason::EndTurn, usage: Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_earlier_turns_and_the_screenshot() {
        let msgs = vec![
            Message::user_text("q1"),
            Message::assistant(vec![Content::text("a1")]),
            Message::user(vec![Content::jpeg_base64("AAAA".into()), Content::text("RIGHT NOW\n…\n\nTHEIR REQUEST\nq2")]),
        ];
        let (text, files) = flatten(&msgs);
        assert!(text.starts_with("EARLIER IN THIS CONVERSATION\nThem: q1\nYou: a1\n\nRIGHT NOW"));
        assert!(text.ends_with("q2"));
        assert_eq!(files.len(), 1);
        assert!(files[0]["uri"].as_str().unwrap().starts_with("data:image/jpeg;base64,AAAA"));
        assert_eq!(split_model("opencode/glm-5.3-flash-free"), ("opencode".into(), "glm-5.3-flash-free".into()));
        assert_eq!(split_model("deepseek/deepseek-flash"), ("deepseek".into(), "deepseek-flash".into()));
    }

    #[test]
    fn free_limit_is_explained() {
        assert!(explain(&json!({ "type": "provider.free-limit", "message": "Free usage limit reached." })).contains("Settings → Models"));
        assert_eq!(explain(&json!({ "type": "x", "message": "boom" })), "boom");
    }
}
