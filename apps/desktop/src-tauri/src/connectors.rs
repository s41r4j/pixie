//! Connectors (M6): MCP servers configured in `<app_data>/mcp.json`, exposed as `mcp__<server>__<tool>`.
//! TODO(M6): filled in by the MCP host.

use openclicky_llm::ToolDef;
use serde_json::Value;
use tauri::AppHandle;

pub fn tools(_app: &AppHandle) -> Vec<ToolDef> {
    vec![]
}

pub async fn call(_app: &AppHandle, name: &str, _input: &Value) -> Result<String, String> {
    Err(format!("{name} isn't connected."))
}

pub fn start(_app: AppHandle) {}

pub fn connected_names() -> Vec<String> {
    vec![]
}
