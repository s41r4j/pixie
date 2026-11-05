//! Connectors (M6): MCP servers from `<app_data>/mcp.json`, started at launch; their tools are
//! offered to voice and jobs as `mcp__<server>__<tool>`.

use crate::AppState;
use openclicky_llm::ToolDef;
use openclicky_mcp::{qualified, Client, Tool};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Manager};

struct Connected {
    client: Arc<Client>,
    tools: Vec<Tool>,
}

fn registry() -> &'static Mutex<BTreeMap<String, Connected>> {
    static R: OnceLock<Mutex<BTreeMap<String, Connected>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(BTreeMap::new()))
}

const EXAMPLE: &str = r#"{
  "mcpServers": {},
  "_examples": {
    "notion": { "command": "npx", "args": ["-y", "@notionhq/notion-mcp-server"], "env": { "NOTION_TOKEN": "ntn_…" } },
    "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "…" } },
    "filesystem": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "~/Documents"] }
  }
}
"#;

fn errors() -> &'static Mutex<BTreeMap<String, String>> {
    static E: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn config_path(app: &AppHandle) -> std::path::PathBuf {
    app.state::<AppState>().data_dir.join("mcp.json")
}

pub fn start(app: AppHandle) {
    let path = config_path(&app);
    if !path.exists() {
        let _ = std::fs::write(&path, EXAMPLE);
    }
    let servers = openclicky_mcp::load_config(&path);
    for (name, cfg) in servers {
        connect(name, cfg);
    }
}

fn connect(name: String, cfg: openclicky_mcp::ServerConfig) {
    errors().lock().unwrap().remove(&name);
    tauri::async_runtime::spawn(async move {
        let res = match Client::connect(&name, &cfg).await {
            Ok(client) => client.list_tools().await.map(|tools| (client, tools)),
            Err(e) => Err(e),
        };
        match res {
            Ok((client, tools)) => {
                log::info!("mcp {name}: {} tools", tools.len());
                registry().lock().unwrap().insert(name, Connected { client, tools });
            }
            Err(e) => {
                log::warn!("mcp {name}: {e}");
                errors().lock().unwrap().insert(name, e);
            }
        }
    });
}

/// Re-read mcp.json: drop servers that went away or changed, start the new ones.
pub fn reload(app: &AppHandle) {
    let servers = openclicky_mcp::load_config(&config_path(app));
    registry().lock().unwrap().clear(); // dropping a client stops its server
    errors().lock().unwrap().clear();
    for (name, cfg) in servers {
        connect(name, cfg);
    }
}

// ---- the gallery (Settings → Connectors) -------------------------------------------------

/// One-click MCP servers. `env` is (variable, label, placeholder); `{folder}` in args is asked for.
struct Preset {
    id: &'static str,
    name: &'static str,
    blurb: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    env: &'static [(&'static str, &'static str, &'static str)],
    /// Where to get the token, or a note on how sign-in works.
    help: &'static str,
    help_url: &'static str,
}

const PRESETS: &[Preset] = &[
    Preset {
        id: "github", name: "GitHub", blurb: "Issues, pull requests, code search and files in your repos.",
        command: "npx", args: &["-y", "@modelcontextprotocol/server-github"],
        env: &[("GITHUB_PERSONAL_ACCESS_TOKEN", "Personal access token", "github_pat_…")],
        help: "Create a fine-grained token with the repos it may read.", help_url: "https://github.com/settings/personal-access-tokens/new",
    },
    Preset {
        id: "notion", name: "Notion", blurb: "Search, read and add pages in the workspaces you share with it.",
        command: "npx", args: &["-y", "@notionhq/notion-mcp-server"],
        env: &[("NOTION_TOKEN", "Integration secret", "ntn_…")],
        help: "Make an internal integration, then share pages with it.", help_url: "https://www.notion.so/profile/integrations",
    },
    Preset {
        id: "slack", name: "Slack", blurb: "Read channels and threads, post when you ask.",
        command: "npx", args: &["-y", "@modelcontextprotocol/server-slack"],
        env: &[("SLACK_BOT_TOKEN", "Bot token", "xoxb-…"), ("SLACK_TEAM_ID", "Workspace ID", "T01234567")],
        help: "Create a Slack app with channels:history, channels:read, chat:write and users:read.", help_url: "https://api.slack.com/apps",
    },
    Preset {
        id: "linear", name: "Linear", blurb: "Find, create and update issues and projects.",
        command: "npx", args: &["-y", "mcp-remote", "https://mcp.linear.app/sse"],
        env: &[],
        help: "No token: Linear opens in your browser to sign in the first time.", help_url: "https://linear.app/docs/mcp",
    },
    Preset {
        id: "atlassian", name: "Jira & Confluence", blurb: "Search and update Jira issues and Confluence pages.",
        command: "npx", args: &["-y", "mcp-remote", "https://mcp.atlassian.com/v1/sse"],
        env: &[],
        help: "No token: Atlassian opens in your browser to sign in the first time.", help_url: "https://support.atlassian.com/rovo/docs/getting-started-with-the-atlassian-remote-mcp-server/",
    },
    Preset {
        id: "brave", name: "Brave Search", blurb: "Private web and news search with fresh results.",
        command: "npx", args: &["-y", "@modelcontextprotocol/server-brave-search"],
        env: &[("BRAVE_API_KEY", "API key", "BSA…")],
        help: "Free tier available.", help_url: "https://brave.com/search/api/",
    },
    Preset {
        id: "files", name: "A folder", blurb: "Read and write files in one folder you choose (beyond Pixie's own file tools).",
        command: "npx", args: &["-y", "@modelcontextprotocol/server-filesystem", "{folder}"],
        env: &[],
        help: "Pick the folder below.", help_url: "",
    },
];

fn read_file(app: &AppHandle) -> Value {
    std::fs::read_to_string(config_path(app)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({ "mcpServers": {} }))
}

fn write_file(app: &AppHandle, v: &Value) -> Result<(), String> {
    let s = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(config_path(app), s).map_err(|e| e.to_string())
}

fn has_node() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    ["/opt/homebrew/bin", "/usr/local/bin"].iter().map(|s| s.to_string()).chain(std::env::split_paths(&path).map(|p| p.display().to_string())).any(|d| {
        let p = std::path::Path::new(&d);
        p.join("npx").exists() || p.join("npx.cmd").exists()
    })
}

/// Presets + everything in mcp.json, with on/off and connection state. Secrets are masked.
pub fn gallery(app: &AppHandle) -> Value {
    let file = read_file(app);
    let servers = file["mcpServers"].as_object().cloned().unwrap_or_default();
    let live = registry().lock().unwrap().iter().map(|(k, c)| (k.clone(), c.tools.len())).collect::<BTreeMap<_, _>>();
    let errs = errors().lock().unwrap().clone();
    let state = |name: &str, cfg: Option<&Value>| -> Value {
        let added = cfg.is_some();
        let on = cfg.map(|c| !c["disabled"].as_bool().unwrap_or(false)).unwrap_or(false);
        json!({
            "added": added, "on": on,
            "tools": live.get(name).copied(),
            "error": errs.get(name),
            "env": cfg.and_then(|c| c["env"].as_object()).map(|e| e.iter().map(|(k, v)| (k.clone(), json!(mask(v.as_str().unwrap_or(""))))).collect::<serde_json::Map<_, _>>()),
            "args": cfg.map(|c| c["args"].clone()),
        })
    };
    let presets: Vec<Value> = PRESETS
        .iter()
        .map(|p| {
            let mut v = json!({
                "id": p.id, "name": p.name, "blurb": p.blurb, "help": p.help, "help_url": p.help_url,
                "fields": p.env.iter().map(|(k, l, ph)| json!({ "key": k, "label": l, "placeholder": ph })).collect::<Vec<_>>(),
                "folder": p.args.contains(&"{folder}"),
            });
            if let (Some(o), Value::Object(st)) = (v.as_object_mut(), state(p.id, servers.get(p.id))) {
                o.extend(st);
            }
            v
        })
        .collect();
    let custom: Vec<Value> = servers
        .iter()
        .filter(|(k, _)| !PRESETS.iter().any(|p| p.id == k.as_str()))
        .map(|(k, c)| {
            let mut v = json!({ "id": k, "name": k, "command": c["command"], "url": c["url"] });
            if let (Some(o), Value::Object(st)) = (v.as_object_mut(), state(k, Some(c))) {
                o.extend(st);
            }
            v
        })
        .collect();
    json!({ "presets": presets, "custom": custom, "node": has_node(), "path": config_path(app) })
}

fn mask(v: &str) -> String {
    if v.is_empty() {
        String::new()
    } else if v.chars().count() <= 8 {
        "••••".into()
    } else {
        format!("{}••••{}", v.chars().take(4).collect::<String>(), v.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<String>())
    }
}

/// Add or update a preset (values left masked keep the stored secret), or switch one on/off.
pub fn save(app: &AppHandle, id: &str, values: &Value, on: bool) -> Result<(), String> {
    let mut file = read_file(app);
    if !file["mcpServers"].is_object() {
        file["mcpServers"] = json!({});
    }
    let existing = file["mcpServers"].get(id).cloned();
    let entry = if let Some(p) = PRESETS.iter().find(|p| p.id == id) {
        let mut env = serde_json::Map::new();
        for (k, label, _) in p.env {
            let given = values.get(*k).and_then(Value::as_str).unwrap_or("").trim().to_string();
            let old = existing.as_ref().and_then(|e| e["env"][*k].as_str()).unwrap_or("").to_string();
            let v = if given.is_empty() || given.contains('•') { old } else { given };
            if v.is_empty() && on {
                return Err(format!("{} needs its {}.", p.name, label.to_lowercase()));
            }
            env.insert(k.to_string(), json!(v));
        }
        let folder = values.get("folder").and_then(Value::as_str).map(str::to_string).or_else(|| {
            existing.as_ref().and_then(|e| e["args"].as_array()).and_then(|a| a.last()).and_then(Value::as_str).map(str::to_string)
        });
        let args: Vec<String> = p
            .args
            .iter()
            .map(|a| if *a == "{folder}" { folder.clone().unwrap_or_default() } else { a.to_string() })
            .collect();
        if p.args.contains(&"{folder}") && folder.as_deref().unwrap_or("").is_empty() {
            return Err("Pick a folder first.".into());
        }
        json!({ "command": p.command, "args": args, "env": env, "disabled": !on })
    } else {
        // A custom server already in mcp.json: only on/off from here.
        let mut e = existing.ok_or("No such connector.")?;
        e["disabled"] = json!(!on);
        e
    };
    file["mcpServers"][id] = entry;
    write_file(app, &file)?;
    reload(app);
    Ok(())
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut file = read_file(app);
    if let Some(o) = file["mcpServers"].as_object_mut() {
        o.remove(id);
    }
    write_file(app, &file)?;
    reload(app);
    Ok(())
}

pub fn connected_names() -> Vec<String> {
    registry().lock().unwrap().keys().cloned().collect()
}

pub fn tools(_app: &AppHandle) -> Vec<ToolDef> {
    let r = registry().lock().unwrap();
    r.iter()
        .flat_map(|(server, c)| {
            c.tools.iter().map(move |t| ToolDef {
                name: qualified(server, &t.name),
                description: format!("[{server}] {}", t.description.chars().take(900).collect::<String>()),
                input_schema: if t.input_schema.get("type").is_some() { t.input_schema.clone() } else { json!({ "type": "object" }) },
            })
        })
        .collect()
}

pub async fn call(_app: &AppHandle, name: &str, input: &Value) -> Result<String, String> {
    let target = {
        let r = registry().lock().unwrap();
        r.iter().find_map(|(server, c)| {
            c.tools.iter().find(|t| qualified(server, &t.name) == name).map(|t| (c.client.clone(), t.name.clone()))
        })
    };
    let (client, tool) = target.ok_or_else(|| format!("{name} isn't connected any more."))?;
    client.call_tool(&tool, input.clone()).await.map(|t| if t.chars().count() > 60_000 { format!("{}…[cut]", t.chars().take(60_000).collect::<String>()) } else { t })
}
