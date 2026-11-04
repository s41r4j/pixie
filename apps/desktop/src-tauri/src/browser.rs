//! Bridge to the user's Chrome through the openclicky extension (M7). See `native-host`.
//! TODO(M7): filled in by the native-messaging bridge.

use serde_json::Value;
use tauri::AppHandle;

pub fn connected(_app: &AppHandle) -> bool {
    false
}

pub async fn call(_app: &AppHandle, _input: &Value) -> Result<String, String> {
    Err("Their browser isn't connected. Ask them to install the openclicky extension from Settings → Connectors.".into())
}

pub fn start(_app: AppHandle) {}
