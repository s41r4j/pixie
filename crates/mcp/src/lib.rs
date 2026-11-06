//! MCP connector host (M6). Speaks the Model Context Protocol over stdio (newline-delimited
//! JSON-RPC 2.0) to servers listed in `mcp.json`, in Claude Desktop's format:
//!
//! ```json
//! { "mcpServers": { "notion": { "command": "npx", "args": ["-y", "@notionhq/notion-mcp-server"],
//!                               "env": { "NOTION_TOKEN": "…" } } } }
//! ```
//!
//! TODO: streamable-HTTP servers (`"url": …`) and OAuth; only stdio servers for now.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::oneshot;

pub const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ServerConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, ServerConfig>,
    #[serde(default)]
    servers: BTreeMap<String, ServerConfig>,
}

/// Parse `mcp.json` (accepts `mcpServers` or `servers`).
pub fn parse_config(src: &str) -> Result<BTreeMap<String, ServerConfig>, String> {
    let f: ConfigFile = serde_json::from_str(src).map_err(|e| format!("mcp.json: {e}"))?;
    let mut all = f.mcp_servers;
    all.extend(f.servers);
    Ok(all.into_iter().filter(|(_, c)| !c.disabled).collect())
}

pub fn load_config(path: &Path) -> BTreeMap<String, ServerConfig> {
    match std::fs::read_to_string(path) {
        Ok(src) => parse_config(&src).unwrap_or_else(|e| {
            log::warn!("{e}");
            BTreeMap::new()
        }),
        Err(_) => BTreeMap::new(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct Client {
    pub name: String,
    stdin: tokio::sync::Mutex<ChildStdin>,
    pending: Pending,
    next: AtomicU64,
    _child: Mutex<Child>,
    pub server_info: Mutex<Value>,
    /// The last lines the server wrote to stderr (why it failed, usually).
    stderr: Arc<Mutex<std::collections::VecDeque<String>>>,
}

/// The command to run: on Windows, npx / uvx / npm are `.cmd` scripts that `Command` won't find
/// by bare name.
fn program(command: &str) -> String {
    if cfg!(windows) && matches!(command, "npx" | "npm" | "uvx" | "pnpm" | "yarn" | "bunx") {
        format!("{command}.cmd")
    } else {
        command.to_string()
    }
}

impl Client {
    /// Spawn the server and run the `initialize` handshake.
    pub async fn connect(name: &str, cfg: &ServerConfig) -> Result<Arc<Self>, String> {
        if cfg.command.is_empty() {
            return Err(format!("{name}: only stdio servers (\"command\") are supported so far"));
        }
        let mut cmd = Command::new(program(&cfg.command));
        cmd.args(&cfg.args)
            .envs(&cfg.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        // GUI apps get a bare PATH; servers are usually npx/uvx from Homebrew.
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<std::path::PathBuf> = if cfg!(target_os = "macos") { vec!["/opt/homebrew/bin".into(), "/usr/local/bin".into()] } else { vec![] };
        dirs.extend(std::env::split_paths(&path));
        if let Ok(joined) = std::env::join_paths(dirs) {
            cmd.env("PATH", joined);
        }
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("{name}: {} isn't installed (or isn't on PATH)", cfg.command)
            } else {
                format!("{name}: couldn't start {}: {e}", cfg.command)
            }
        })?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr_tail: Arc<Mutex<std::collections::VecDeque<String>>> = Arc::default();
        if let Some(err) = child.stderr.take() {
            let tail = stderr_tail.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let mut t = tail.lock().unwrap();
                    t.push_back(line.chars().take(300).collect());
                    if t.len() > 12 {
                        t.pop_front();
                    }
                }
            });
        }
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = pending.clone();
        let server = name.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                let Some(id) = v.get("id").and_then(Value::as_u64) else { continue }; // notifications ignored
                if v.get("method").is_some() {
                    continue; // server→client requests (sampling, roots) not supported yet
                }
                if let Some(tx) = reader_pending.lock().unwrap().remove(&id) {
                    let r = match v.get("error") {
                        Some(e) => Err(e["message"].as_str().unwrap_or("error").to_string()),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = tx.send(r);
                }
            }
            log::info!("mcp {server}: closed");
            for (_, tx) in reader_pending.lock().unwrap().drain() {
                let _ = tx.send(Err("the connector stopped".into()));
            }
        });
        let client = Arc::new(Self {
            name: name.to_string(),
            stdin: tokio::sync::Mutex::new(stdin),
            pending,
            next: AtomicU64::new(1),
            _child: Mutex::new(child),
            server_info: Mutex::new(Value::Null),
            stderr: stderr_tail,
        });
        let init = match client
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": { "name": "openclicky", "version": env!("CARGO_PKG_VERSION") }
                }),
                Duration::from_secs(60),
            )
            .await
        {
            Ok(v) => v,
            Err(e) => {
                // Let the last of its output arrive, then say what it said.
                tokio::time::sleep(Duration::from_millis(300)).await;
                let said = client.stderr_tail();
                return Err(if said.is_empty() { e } else { format!("{e}. It said: {said}") });
            }
        };
        *client.server_info.lock().unwrap() = init.get("serverInfo").cloned().unwrap_or(Value::Null);
        client.notify("notifications/initialized", json!({})).await?;
        Ok(client)
    }

    /// What the server last wrote to stderr, most useful line last.
    pub fn stderr_tail(&self) -> String {
        let t = self.stderr.lock().unwrap();
        let useful: Vec<&String> = t.iter().filter(|l| !l.starts_with("npm warn") && !l.starts_with("npm WARN")).collect();
        useful.iter().rev().take(3).rev().map(|s| s.as_str()).collect::<Vec<_>>().join(" · ")
    }

    async fn write(&self, msg: Value) -> Result<(), String> {
        let mut line = msg.to_string();
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())
    }

    pub async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write(json!({ "jsonrpc": "2.0", "method": method, "params": params })).await
    }

    pub async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.write(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).await?;
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(r)) => r.map_err(|e| format!("{}: {e}", self.name)),
            Ok(Err(_)) => Err(format!("{}: no reply", self.name)),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("{}: {method} timed out", self.name))
            }
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<Tool>, String> {
        let mut out = vec![];
        let mut cursor: Option<String> = None;
        loop {
            let params = match &cursor {
                Some(c) => json!({ "cursor": c }),
                None => json!({}),
            };
            let r = self.request("tools/list", params, Duration::from_secs(30)).await?;
            for t in r["tools"].as_array().cloned().unwrap_or_default() {
                out.push(Tool {
                    name: t["name"].as_str().unwrap_or("").to_string(),
                    description: t["description"].as_str().unwrap_or("").to_string(),
                    input_schema: t.get("inputSchema").cloned().unwrap_or(json!({ "type": "object" })),
                });
            }
            cursor = r["nextCursor"].as_str().map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    /// Call a tool; the result content is flattened to text (images noted).
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<String, String> {
        let r = self
            .request("tools/call", json!({ "name": name, "arguments": args }), Duration::from_secs(120))
            .await?;
        let text = flatten(&r);
        if r["isError"].as_bool().unwrap_or(false) {
            Err(text)
        } else {
            Ok(text)
        }
    }
}

pub fn flatten(result: &Value) -> String {
    let mut parts = vec![];
    for c in result["content"].as_array().cloned().unwrap_or_default() {
        match c["type"].as_str().unwrap_or("") {
            "text" => parts.push(c["text"].as_str().unwrap_or("").to_string()),
            "image" => parts.push("[image]".into()),
            "resource" => parts.push(c["resource"]["text"].as_str().map(str::to_string).unwrap_or_else(|| format!("[resource {}]", c["resource"]["uri"]))),
            "resource_link" => parts.push(format!("[{}]({})", c["name"].as_str().unwrap_or("link"), c["uri"].as_str().unwrap_or(""))),
            _ => {}
        }
    }
    if parts.is_empty() {
        if let Some(s) = result.get("structuredContent") {
            return s.to_string();
        }
    }
    parts.join("\n")
}

/// Tool name as the model sees it: `mcp__<server>__<tool>`, ≤ 64 chars of [A-Za-z0-9_-].
pub fn qualified(server: &str, tool: &str) -> String {
    let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect::<String>();
    let mut n = format!("mcp__{}__{}", clean(server), clean(tool));
    n.truncate(64);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: &str = r#"
import json, sys
for line in sys.stdin:
    m = json.loads(line)
    if "id" not in m: continue
    if m["method"] == "initialize":
        r = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "1"}}
    elif m["method"] == "tools/list":
        r = {"tools": [{"name": "echo", "description": "Echo text", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}
    elif m["method"] == "tools/call":
        t = m["params"]["arguments"].get("text", "")
        r = {"content": [{"type": "text", "text": "echo: " + t}], "isError": t == "fail"}
    else:
        print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "error": {"code": -32601, "message": "nope"}}), flush=True); continue
    print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": r}), flush=True)
"#;

    #[tokio::test]
    async fn talks_to_a_stdio_server() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        std::fs::write(&script, FAKE).unwrap();
        let cfg = ServerConfig { command: "python3".into(), args: vec![script.display().to_string()], env: Default::default(), url: None, disabled: false };
        let c = Client::connect("fake", &cfg).await.unwrap();
        assert_eq!(c.server_info.lock().unwrap()["name"], "fake");
        let tools = c.list_tools().await.unwrap();
        assert_eq!(tools[0].name, "echo");
        assert_eq!(c.call_tool("echo", json!({"text": "hi"})).await.unwrap(), "echo: hi");
        assert!(c.call_tool("echo", json!({"text": "fail"})).await.is_err());
        assert!(c.request("bogus", json!({}), Duration::from_secs(5)).await.unwrap_err().contains("nope"));
    }

    #[test]
    fn config_and_names() {
        let c = parse_config(r#"{"mcpServers":{"a":{"command":"npx","args":["x"]},"b":{"command":"y","disabled":true}},"servers":{"c":{"url":"https://x"}}}"#).unwrap();
        assert_eq!(c.keys().cloned().collect::<Vec<_>>(), vec!["a", "c"]);
        assert_eq!(qualified("my server", "get.page"), "mcp__my_server__get_page");
        assert!(qualified("s", &"x".repeat(100)).len() <= 64);
    }
}
