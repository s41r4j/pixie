//! Locating the resources root and serving slots to the webview as `slot://localhost/<kind>/<slot>`.
//! The UI references media by slot name only; this is the single place that touches paths.

use crate::AppState;
use openclicky_core::resources::{Manifest, ResourceKind, ResourceStore};
use std::borrow::Cow;
use std::path::PathBuf;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime, UriSchemeContext};

/// Where the active resources came from (shown in the setup window).
#[derive(Clone, Copy, Debug, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackSource {
    Env,
    Local,
    Bundled,
    Repo,
}

/// The per-user local pack: `~/Library/Application Support/com.openclicky.app/resources`.
/// Filled by `npm run assets:local`; never part of the repo or the bundle.
pub fn local_pack_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("resources"))
}

/// Resolution order: `$OPENCLICKY_RESOURCES` → local pack → bundled
/// `Contents/Resources/resources` → the repo's `resources/` (dev builds).
fn resources_root<R: Runtime>(app: &AppHandle<R>) -> (PathBuf, PackSource) {
    if let Some(p) = std::env::var_os("OPENCLICKY_RESOURCES") {
        return (PathBuf::from(p), PackSource::Env);
    }
    if let Some(local) = local_pack_dir(app) {
        if local.join("manifest.json").is_file() {
            return (local, PackSource::Local);
        }
    }
    if let Ok(dir) = app.path().resource_dir() {
        let bundled = dir.join("resources");
        if bundled.join("manifest.json").is_file() {
            return (bundled, PackSource::Bundled);
        }
    }
    (PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../resources")), PackSource::Repo)
}

/// Shipped design tokens; used when the active pack has no readable `theme.json`.
const DEFAULT_THEME: &str = include_str!("../../../../resources/theme.json");

/// Design tokens from `<root>/theme.json` (BUILD-SPEC §8), exposed to the UI as CSS vars.
pub fn load_theme(root: &std::path::Path) -> serde_json::Value {
    std::fs::read_to_string(root.join("theme.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::from_str(DEFAULT_THEME).expect("shipped theme.json is valid"))
}

/// Load the store, falling back to an empty manifest so the app always starts.
pub fn load_store<R: Runtime>(app: &AppHandle<R>) -> (ResourceStore, PackSource) {
    let (root, source) = resources_root(app);
    let store = match ResourceStore::load(&root) {
        Ok(store) => {
            let missing = store.missing();
            if missing.is_empty() {
                log::info!("resources: {} ready ({source:?})", root.display());
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
    };
    (store, source)
}

fn content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff2" => "font/woff2",
        "jpg" | "jpeg" => "image/jpeg",
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
