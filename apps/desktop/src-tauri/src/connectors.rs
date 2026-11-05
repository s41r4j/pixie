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

pub fn start(app: AppHandle) {
    let path = app.state::<AppState>().data_dir.join("mcp.json");
    if !path.exists() {
        let _ = std::fs::write(&path, EXAMPLE);
    }
    let servers = openclicky_mcp::load_config(&path);
    for (name, cfg) in servers {
        tauri::async_runtime::spawn(async move {
            match Client::connect(&name, &cfg).await {
                Ok(client) => match client.list_tools().await {
                    Ok(tools) => {
                        log::info!("mcp {name}: {} tools", tools.len());
                        registry().lock().unwrap().insert(name, Connected { client, tools });
                    }
                    Err(e) => log::warn!("mcp {name}: {e}"),
                },
                Err(e) => log::warn!("mcp {name}: {e}"),
            }
        });
    }
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
