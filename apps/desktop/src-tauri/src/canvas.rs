//! Diagrams and visual explanations on a real Excalidraw canvas the user can edit and keep.
//!
//! Each canvas is an `.excalidraw` file in `<app_data>/canvases/`. The model's `canvas` tool
//! queues an operation (draw shapes or a Mermaid diagram, read, clear); the
//! canvas window takes it, draws it element by element, saves, and reports back. Exports go to
//! Downloads and never overwrite.

use crate::AppState;
use openclicky_core::Event;
use openclicky_llm::ToolDef;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

fn dir(app: &AppHandle) -> PathBuf {
    app.state::<AppState>().data_dir.join("canvases")
}

/// Ids are generated here; refuse anything else so a path can't sneak in.
fn valid(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn file(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if !valid(id) {
        return Err("bad canvas id".into());
    }
    Ok(dir(app).join(format!("{id}.excalidraw")))
}

#[derive(Serialize, Clone)]
pub struct Op {
    pub req: u64,
    pub canvas: String,
    pub action: String,
    pub input: Value,
}

static OPS: Mutex<Vec<Op>> = Mutex::new(Vec::new());
type Reply = oneshot::Sender<Result<String, String>>;
static WAITING: Mutex<Option<HashMap<u64, Reply>>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);
/// The canvas the model is drawing on (the last one opened or drawn to).
static CURRENT: Mutex<Option<String>> = Mutex::new(None);
/// Deleted this run: a late autosave from the window must not bring them back.
static DELETED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn new_id() -> String {
    format!("{}-{:04x}", chrono::Local::now().format("%Y%m%d-%H%M%S"), rand::random::<u16>())
}

pub fn tool() -> ToolDef {
    ToolDef {
        name: "canvas".into(),
        description: "An Excalidraw canvas in its own window: real shapes they can move, edit and save (.excalidraw, PNG, SVG). Use it for any diagram, flow, sketch, layout or explanation that needs a picture. The window comes up in front of them; it goes away when they click outside it, and action=open brings it back.\n\
action=open shows a canvas: the one in use (or the last one), canvas=<id> or title=<words> to pick a saved one (see list), or new=true for a blank one. Use it whenever they ask to open, show or bring back the canvas.\n\
action=draw adds to the canvas in use and shows it (title names it, new=true starts a fresh one). Give either:\n\
• mermaid: a Mermaid flowchart / sequence / class diagram (best for flows and processes; it's laid out for you), or\n\
• elements: Excalidraw skeletons, coordinates in px, origin top-left, roughly 0–1200 × 0–800. Types: rectangle, ellipse, diamond (x, y, width, height, optional label: {text}), text (x, y, text, fontSize), arrow / line (x, y, width, height = the vector to the end; or start: {id} and end: {id} to connect shapes by their id, with label: {text}), frame (x, y, width, height, name, children: [ids]). Give shapes an id when arrows connect to them. Optional: strokeColor, backgroundColor (e.g. #a5d8ff, #b2f2bb, #ffec99, #ffc9c9, #d0bfff), fillStyle (solid, hachure), strokeStyle (dashed), roundness: {type: 3}.\n\
action=read returns what's on the canvas now (including their edits). action=clear empties it but keeps it.\n\
action=list shows their saved canvases (id, title, date, shapes).\n\
action=delete removes a canvas from Pixie (the file goes to the Trash): the one in use, canvas=<id>, or all=true for every one. Only when they ask to delete; clearing is not deleting.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["open", "draw", "read", "clear", "list", "delete"] },
                "canvas": { "type": "string", "description": "a canvas id from list" },
                "title": { "type": "string" },
                "new": { "type": "boolean" },
                "all": { "type": "boolean", "description": "delete: every canvas" },
                "mermaid": { "type": "string" },
                "elements": { "type": "array", "items": { "type": "object" } }
            },
            "required": ["action"]
        }),
    }
}

/// Open (or bring up) a canvas and make it current.
pub fn open(app: &AppHandle, id: &str) {
    *CURRENT.lock().unwrap() = Some(id.to_string());
    crate::windows::open_canvas(app, id);
}

fn exists(app: &AppHandle, id: &str) -> bool {
    file(app, id).map(|f| f.exists()).unwrap_or(false)
}

/// The canvas in use, if it's still there.
fn current(app: &AppHandle) -> Option<String> {
    let id = CURRENT.lock().unwrap().clone()?;
    (exists(app, &id) || crate::windows::canvas_visible(app)).then_some(id)
}

/// Which canvas an action means: canvas=<id>, a title, new=true, or the one in use.
fn pick(app: &AppHandle, input: &Value, action: &str) -> Result<String, String> {
    if let Some(id) = input.get("canvas").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        if valid(id) && (exists(app, id) || CURRENT.lock().unwrap().as_deref() == Some(id)) {
            return Ok(id.to_string());
        }
        return Err(format!("There's no canvas \"{id}\"; action=list shows them."));
    }
    if input.get("new").and_then(Value::as_bool).unwrap_or(false) && matches!(action, "open" | "draw") {
        return Ok(new_id());
    }
    if action == "open" {
        if let Some(t) = input.get("title").and_then(Value::as_str).map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()) {
            if let Some(i) = items(app).into_iter().find(|i| i.title.to_lowercase().contains(&t)) {
                return Ok(i.id);
            }
        }
    }
    if let Some(id) = current(app) {
        return Ok(id);
    }
    match action {
        // Bring back the last one they had, or a blank one.
        "open" => Ok(items(app).into_iter().next().map(|i| i.id).unwrap_or_else(new_id)),
        "draw" => Ok(new_id()),
        _ => Err("No canvas is open; use action=open or draw first.".into()),
    }
}

fn describe_item(i: &Item) -> String {
    let when = chrono::DateTime::from_timestamp(i.modified, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%b %-d %H:%M").to_string())
        .unwrap_or_default();
    format!("{} · \"{}\" · {when} · {} shape(s)", i.id, if i.title.is_empty() { "Untitled" } else { &i.title }, i.shapes)
}

/// The model's `canvas` tool.
pub async fn call(app: &AppHandle, input: &Value) -> Result<String, String> {
    let action = input.get("action").and_then(Value::as_str).unwrap_or("draw").to_string();
    match action.as_str() {
        "list" => {
            let all = items(app);
            let cur = current(app);
            return Ok(if all.is_empty() {
                "No saved canvases.".into()
            } else {
                all.iter()
                    .map(|i| format!("{}{}", describe_item(i), if cur.as_deref() == Some(i.id.as_str()) { " (in use)" } else { "" }))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        "delete" => {
            if input.get("all").and_then(Value::as_bool).unwrap_or(false) {
                let all = items(app);
                for i in &all {
                    remove(app, &i.id)?;
                }
                if let Some(id) = current(app) {
                    remove(app, &id)?;
                }
                return Ok(format!("Deleted {} canvas(es); the files are in the Trash.", all.len()));
            }
            let id = pick(app, input, "delete")?;
            let title = items(app).into_iter().find(|i| i.id == id).map(|i| i.title).unwrap_or_default();
            remove(app, &id)?;
            return Ok(format!("Deleted the canvas \"{}\"; the file is in the Trash.", if title.is_empty() { "Untitled" } else { &title }));
        }
        "open" => {
            let id = pick(app, input, "open")?;
            open(app, &id);
            let what = items(app)
                .into_iter()
                .find(|i| i.id == id)
                .map(|i| describe_item(&i))
                .unwrap_or_else(|| format!("{id} · a new, blank canvas"));
            return Ok(format!("The canvas is open in front of them: {what}."));
        }
        "draw" | "read" | "clear" => {}
        other => return Err(format!("Unknown canvas action \"{other}\".")),
    }
    let id = pick(app, input, &action)?;
    open(app, &id);
    let req = NEXT.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = oneshot::channel();
    WAITING.lock().unwrap().get_or_insert_with(HashMap::new).insert(req, tx);
    OPS.lock().unwrap().push(Op { req, canvas: id.clone(), action: action.clone(), input: input.clone() });
    crate::emit(app, Event::new("canvas.ops", json!({ "canvas": id })));
    if action != "read" {
        app.state::<AppState>().play("pencil-paper-bed");
    }
    let r = match tokio::time::timeout(Duration::from_secs(25), rx).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => Err("The canvas closed.".into()),
        Err(_) => {
            WAITING.lock().unwrap().get_or_insert_with(HashMap::new).remove(&req);
            Err("The canvas didn't answer in time.".into())
        }
    };
    if let Ok(mut bus) = app.state::<AppState>().audio.lock() {
        bus.stop_bed();
    }
    r
}

/// Move a file to the Trash (so a deleted canvas can still be fished back).
fn to_trash(f: &Path) -> Result<(), String> {
    if !f.exists() {
        return Ok(());
    }
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        // NSFileManager, not Finder scripting (that would ask for Automation access).
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx.delete(f).map_err(|e| format!("Couldn't move it to the Trash: {e}"))
}

/// Delete a canvas: its file goes to the Trash, and the window closes if it was showing it.
fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let f = file(app, id)?;
    DELETED.lock().unwrap().push(id.to_string());
    to_trash(&f)?;
    let _ = std::fs::remove_file(f.with_extension("excalidraw.tmp"));
    OPS.lock().unwrap().retain(|o| o.canvas != id);
    let was_current = {
        let mut cur = CURRENT.lock().unwrap();
        let hit = cur.as_deref() == Some(id);
        if hit {
            *cur = None;
        }
        hit
    };
    crate::emit(app, Event::new("canvas.deleted", json!({ "id": id })));
    if was_current {
        crate::windows::hide_canvas(app);
    }
    Ok(())
}

// ---- commands for the canvas window ----

#[derive(Serialize)]
pub struct Item {
    id: String,
    title: String,
    modified: i64,
    shapes: usize,
}

/// Saved canvases, newest first.
fn items(app: &AppHandle) -> Vec<Item> {
    let mut out: Vec<Item> = std::fs::read_dir(dir(app))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    let id = p.file_stem()?.to_string_lossy().to_string();
                    if p.extension()? != "excalidraw" || !valid(&id) {
                        return None;
                    }
                    let modified = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
                    let v = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
                    let title = v["appState"]["name"].as_str().unwrap_or("").to_string();
                    let shapes = v["elements"]
                        .as_array()
                        .map(|a| a.iter().filter(|e| !e["isDeleted"].as_bool().unwrap_or(false) && e["containerId"].is_null()).count())
                        .unwrap_or(0);
                    Some(Item { id, title, modified, shapes })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|i| std::cmp::Reverse(i.modified));
    out
}

#[tauri::command]
pub fn canvas_list(app: AppHandle) -> Vec<Item> {
    items(&app)
}

#[tauri::command]
pub fn canvas_load(app: AppHandle, id: String) -> Result<Option<String>, String> {
    let f = file(&app, &id)?;
    Ok(std::fs::read_to_string(f).ok())
}

#[tauri::command]
pub fn canvas_save(app: AppHandle, id: String, scene: String) -> Result<(), String> {
    let f = file(&app, &id)?;
    if DELETED.lock().unwrap().contains(&id) {
        return Ok(()); // deleted; a late autosave doesn't bring it back
    }
    std::fs::create_dir_all(dir(&app)).map_err(|e| e.to_string())?;
    // Write then rename, so a crash never leaves half a drawing.
    let tmp = f.with_extension("excalidraw.tmp");
    std::fs::write(&tmp, scene).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &f).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn canvas_delete(app: AppHandle, id: String) -> Result<(), String> {
    remove(&app, &id)
}

#[tauri::command]
pub fn canvas_open(app: AppHandle, id: Option<String>, fresh: Option<bool>) -> String {
    let id = match id.filter(|i| valid(i)) {
        Some(id) => id,
        None if fresh.unwrap_or(false) => new_id(),
        // The one in use, else the last one, else a blank one.
        None => pick(&app, &json!({}), "open").unwrap_or_else(|_| new_id()),
    };
    open(&app, &id);
    id
}

/// Operations waiting for this canvas (the window takes them on load and on `canvas.ops`).
#[tauri::command]
pub fn canvas_take_ops(id: String) -> Vec<Op> {
    *CURRENT.lock().unwrap() = Some(id.clone());
    let mut ops = OPS.lock().unwrap();
    let (mine, rest): (Vec<Op>, Vec<Op>) = ops.drain(..).partition(|o| o.canvas == id);
    *ops = rest;
    mine
}

/// The window finished an operation.
#[tauri::command]
pub fn canvas_done(req: u64, ok: bool, text: String) {
    if let Some(tx) = WAITING.lock().unwrap().get_or_insert_with(HashMap::new).remove(&req) {
        let _ = tx.send(if ok { Ok(text) } else { Err(text) });
    }
}

/// Save an export to Downloads (`data` is text, or base64 for PNG).
#[tauri::command]
pub fn canvas_export(name: String, ext: String, data: String) -> Result<String, String> {
    use base64::Engine;
    let ext = match ext.as_str() {
        "png" | "svg" | "excalidraw" => ext,
        _ => return Err("unknown format".into()),
    };
    let bytes = if ext == "png" { base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| e.to_string())? } else { data.into_bytes() };
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '-' }).collect::<String>().trim().chars().take(80).collect();
    let stem = if clean.is_empty() { "Pixie drawing".to_string() } else { clean };
    let downloads = openclicky_tools::guard::home().join("Downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let dest = openclicky_tools::guard::unique_destination(&downloads.join(format!("{stem}.{ext}")));
    std::fs::write(&dest, bytes).map_err(|e| e.to_string())?;
    // Not revealed here: Finder taking focus would close the canvas. The window offers "Show".
    Ok(dest.display().to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_cannot_escape_the_folder() {
        assert!(super::valid("20261106-203000-0a1b"));
        assert!(!super::valid("../secrets"));
        assert!(!super::valid("a/b"));
        assert!(!super::valid(""));
    }
}
