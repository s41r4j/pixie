//! Locating the resources root and serving slots to the webview as `slot://localhost/<kind>/<slot>`.
//! The UI references media by slot name only; this is the single place that touches paths.

use crate::AppState;
use openclicky_core::resources::{Manifest, ResourceKind, ResourceStore};
use std::borrow::Cow;
use std::path::PathBuf;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime, UriSchemeContext};

/// Resolution order: `$OPENCLICKY_RESOURCES` → bundled `Contents/Resources/resources` →
/// the repo's `resources/` (dev builds).
fn resources_root<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    if let Some(p) = std::env::var_os("OPENCLICKY_RESOURCES") {
        return PathBuf::from(p);
    }
    if let Ok(dir) = app.path().resource_dir() {
        let bundled = dir.join("resources");
        if bundled.join("manifest.json").is_file() {
            return bundled;
        }
    }
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../resources"))
}

/// Load the store, falling back to an empty manifest so the app always starts.
pub fn load_store<R: Runtime>(app: &AppHandle<R>) -> ResourceStore {
    let root = resources_root(app);
    match ResourceStore::load(&root) {
        Ok(store) => {
            let missing = store.missing();
            if missing.is_empty() {
                log::info!("resources: {} ready", root.display());
            } else {
                log::warn!(
                    "resources: {} file(s) missing under {} (run `npm run placeholders`)",
                    missing.len(),
                    root.display()
                );
            }
            store
        }
        Err(e) => {
            log::error!("resources: {e}; continuing without media");
            let empty = Manifest { version: 1, ..Default::default() };
            ResourceStore::from_manifest(root, empty).expect("empty manifest is valid")
        }
    }
}

fn content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => "image/png",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn respond(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(body))
        .expect("valid response")
}

pub fn slot_protocol<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path().trim_start_matches('/');
    let Some((kind, slot)) = path.split_once('/') else {
        return respond(StatusCode::BAD_REQUEST, "text/plain", b"expected /<kind>/<slot>".to_vec());
    };
    let state = ctx.app_handle().state::<AppState>();
    let file = ResourceKind::parse(kind).and_then(|k| state.store.resolve(k, slot));
    match file.map(|p| (std::fs::read(&p), p)) {
        Some((Ok(bytes), p)) => respond(StatusCode::OK, content_type(&p), bytes),
        _ => respond(StatusCode::NOT_FOUND, "text/plain", format!("no slot {path}").into_bytes()),
    }
}
