//! 3D explanations (Settings → Experimental → 3D explanations): a window with labelled shapes and
//! arrows they can spin, built by the `scene3d` tool. Marks in the spoken answer (`[[3d heart]]`)
//! turn to and light up one object as its sentence plays.
//!
//! The window is created when a scene is shown and freed when it's closed. The scene itself is
//! kept here so the window can load it when it starts.

use crate::{emit, AppState};
use openclicky_core::Event;
use openclicky_llm::ToolDef;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const FLAG: &str = "3d";
pub const WINDOW: &str = "scene";
const SHAPES: &[&str] = &["box", "sphere", "cylinder", "cone", "torus", "capsule", "plane", "ring"];
const MAX_OBJECTS: usize = 40;

static SCENE: Mutex<Option<Value>> = Mutex::new(None);

pub fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.lock().map(|s| s.experiment(FLAG)).unwrap_or(false)
}

pub fn tool() -> ToolDef {
    let obj = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "short unique name, used by [[3d name]] marks"},
            "shape": {"type": "string", "enum": SHAPES},
            "at": {"type": "array", "items": {"type": "number"}, "description": "[x, y, z], about -5..5; y is up"},
            "size": {"type": "array", "items": {"type": "number"}, "description": "[w, h, d] or [diameter]; about 0.3..4"},
            "color": {"type": "string", "enum": ["white", "grey", "blue", "red", "green", "orange", "yellow", "pink", "purple"]},
            "label": {"type": "string", "description": "1-4 words shown next to it"}
        },
        "required": ["name", "shape"]
    });
    let link = json!({
        "type": "object",
        "properties": { "from": {"type": "string"}, "to": {"type": "string"}, "label": {"type": "string"} },
        "required": ["from", "to"]
    });
    ToolDef {
        name: "scene3d".into(),
        description: "A 3D model in its own window they can spin: labelled shapes (box, sphere, cylinder, cone, torus, capsule, plane, ring; rings and tori lie flat, so a ring is an orbit) placed in space, with arrows between them. For things that are easier to grasp in 3D (anatomy, molecules, machines, planets, buildings, how parts stack). Build it, then explain it out loud with a [[3d name]] mark in each sentence so the view turns to that part as you say it. action=focus turns to one object; close closes the window.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["show", "focus", "close"]},
                "title": {"type": "string"},
                "objects": {"type": "array", "items": obj},
                "links": {"type": "array", "items": link},
                "name": {"type": "string", "description": "focus: the object"}
            }
        }),
    }
}

/// Keep what the window can draw: known shapes, numbers in range, at most 40 objects, links
/// between objects that exist.
pub fn sanitize(input: &Value) -> Result<Value, String> {
    let mut names = vec![];
    let mut objects = vec![];
    for o in input.get("objects").and_then(Value::as_array).into_iter().flatten().take(MAX_OBJECTS) {
        let name = o.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let shape = o.get("shape").and_then(Value::as_str).unwrap_or("box");
        if name.is_empty() || names.contains(&name.to_lowercase()) {
            continue;
        }
        let nums = |k: &str, n: usize, def: f64, lo: f64, hi: f64| -> Vec<f64> {
            let v: Vec<f64> = o.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).map(|x| x.clamp(lo, hi)).collect()).unwrap_or_default();
            match v.len() {
                0 => vec![def; n],
                1 => vec![v[0]; n],
                _ => (0..n).map(|i| v.get(i).copied().unwrap_or(if k == "at" { 0.0 } else { v[0] })).collect(),
            }
        };
        names.push(name.to_lowercase());
        objects.push(json!({
            "name": name,
            "shape": if SHAPES.contains(&shape) { shape } else { "box" },
            "at": nums("at", 3, 0.0, -20.0, 20.0),
            "size": nums("size", 3, 1.0, 0.05, 12.0),
            "color": o.get("color").and_then(Value::as_str).unwrap_or("white"),
            "label": o.get("label").and_then(Value::as_str).unwrap_or(""),
        }));
    }
    if objects.is_empty() {
        return Err("scene3d needs at least one object with a name and a shape.".into());
    }
    let links: Vec<Value> = input
        .get("links")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|l| {
            let has = |k: &str| l.get(k).and_then(Value::as_str).map(|n| names.contains(&n.trim().to_lowercase())).unwrap_or(false);
            has("from") && has("to")
        })
        .take(60)
        .cloned()
        .collect();
    Ok(json!({ "title": input.get("title").and_then(Value::as_str).unwrap_or(""), "objects": objects, "links": links }))
}

fn names(scene: &Value) -> Vec<String> {
    scene["objects"].as_array().into_iter().flatten().filter_map(|o| o["name"].as_str().map(String::from)).collect()
}

pub async fn call(app: &AppHandle, input: &Value) -> Result<String, String> {
    match input.get("action").and_then(Value::as_str).unwrap_or("show") {
        "close" => {
            close(app);
            Ok("Closed the 3D view.".into())
        }
        "focus" => focus(app, input.get("name").and_then(Value::as_str).unwrap_or(""), ""),
        _ => {
            let scene = sanitize(input)?;
            let n = names(&scene);
            *SCENE.lock().unwrap() = Some(scene.clone());
            open(app);
            emit(app, Event::new("scene3d.show", scene));
            Ok(format!("Showing {} object(s) in 3D: {}. Use [[3d name]] in your sentences to turn to each.", n.len(), n.join(", ")))
        }
    }
}

/// Turn the view to one object and light it up.
pub fn focus(app: &AppHandle, name: &str, label: &str) -> Result<String, String> {
    let scene = SCENE.lock().unwrap().clone().ok_or("No 3D view is open.")?;
    let all = names(&scene);
    let want = name.trim().to_lowercase();
    let hit = all.iter().find(|n| n.to_lowercase() == want).or_else(|| all.iter().find(|n| n.to_lowercase().contains(&want) && !want.is_empty()));
    let Some(hit) = hit else {
        return Err(format!("No object called {name}; there are: {}.", all.join(", ")));
    };
    if app.get_webview_window(WINDOW).is_none() {
        open(app);
    }
    emit(app, Event::new("scene3d.focus", json!({ "name": hit, "label": label })));
    Ok(format!("Turned to {hit}."))
}

fn open(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(WINDOW) {
            let _ = w.show();
            let _ = w.unminimize();
            crate::macos::present(&w);
            return;
        }
        let builder = WebviewWindowBuilder::new(&handle, WINDOW, WebviewUrl::App("index.html".into()))
            .title("Pixie · 3D")
            .inner_size(900.0, 680.0)
            .min_inner_size(420.0, 340.0)
            .visible(true)
            .focused(true);
        #[cfg(target_os = "macos")]
        let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
        match builder.build() {
            Ok(w) => crate::macos::present(&w),
            Err(e) => log::error!("3d window: {e}"),
        }
    });
}

fn close(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(WINDOW) {
            let _ = w.close();
        }
    });
}

/// The window asks for the scene when it starts.
#[tauri::command]
pub fn scene3d_get() -> Option<Value> {
    SCENE.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_are_cleaned() {
        let s = sanitize(&json!({
            "title": "Heart",
            "objects": [
                {"name": "Left atrium", "shape": "sphere", "at": [1, 2], "size": [1.2]},
                {"name": "left atrium", "shape": "box"},
                {"name": "Aorta", "shape": "blob", "at": [0, 99, 0]},
                {"shape": "box"}
            ],
            "links": [{"from": "left atrium", "to": "Aorta"}, {"from": "Aorta", "to": "Lungs"}]
        }))
        .unwrap();
        let o = s["objects"].as_array().unwrap();
        assert_eq!(o.len(), 2, "duplicates and nameless objects are dropped");
        assert_eq!(o[0]["at"], json!([1.0, 2.0, 0.0]));
        assert_eq!(o[0]["size"], json!([1.2, 1.2, 1.2]));
        assert_eq!(o[1]["shape"], "box");
        assert_eq!(o[1]["at"][1], 20.0);
        assert_eq!(s["links"].as_array().unwrap().len(), 1);
        assert!(sanitize(&json!({"objects": []})).is_err());
    }
}
