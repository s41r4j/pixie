//! 3D explanations (Settings → Experimental → 3D explanations): a window with labelled shapes and
//! arrows they can spin, built by the `scene3d` tool. Marks in the spoken answer (`[[3d heart]]`)
//! turn to and light up one object as its sentence plays.
//!
//! The window is created when a scene is shown and freed when it's closed (its close button, the
//! model's `close`, or a click outside Pixie, as with the canvas). The scene itself is kept here
//! so the window can load it when it starts. Once closed, nothing keeps drawing and the rest of
//! the explanation doesn't bring it back until a new model is shown.

use pixie_core::sync::LockOk;
use crate::{emit, AppState};
use pixie_core::Event;
use pixie_llm::ToolDef;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const FLAG: &str = "3d";
pub const WINDOW: &str = "scene";
const SHAPES: &[&str] = &["box", "sphere", "cylinder", "cone", "torus", "capsule", "plane", "ring", "pyramid", "dome", "disc", "tube", "lathe", "extrude", "gear"];
const COLORS: &[&str] = &["white", "grey", "blue", "red", "green", "orange", "yellow", "pink", "purple", "teal", "brown", "black"];
const MATERIALS: &[&str] = &["matte", "metal", "glass", "glow"];
const MAX_OBJECTS: usize = 80;
/// Copies of one object are drawn in a single call, so hundreds are cheap; this is the most
/// across the whole scene.
const MAX_COPIES: usize = 600;

static SCENE: Mutex<Option<Value>> = Mutex::new(None);
/// They clicked away from the 3D view: marks and view changes leave it closed.
static DISMISSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// When the last model was shown, so the old window closing just as a new one opens doesn't put
/// the new one away.
static SHOWN: Mutex<Option<std::time::Instant>> = Mutex::new(None);

fn dismissed() -> bool {
    DISMISSED.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.lock().map(|s| s.experiment(FLAG)).unwrap_or(false)
}

pub fn tool() -> ToolDef {
    let points = |what: &str| json!({"type": "array", "items": {"type": "array", "items": {"type": "number"}}, "description": what});
    let obj = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "short unique name, used by [[3d name]] marks"},
            "shape": {"type": "string", "enum": SHAPES},
            "at": {"type": "array", "items": {"type": "number"}, "description": "[x, y, z], about -5..5; y is up. With parent: relative to the parent"},
            "size": {"type": "array", "items": {"type": "number"}, "description": "[w, h, d] or [diameter]; about 0.3..4. tube: [thickness]. extrude and gear: [_, height]"},
            "color": {"type": "string", "description": "white grey blue red green orange yellow pink purple teal brown black, or #rrggbb"},
            "label": {"type": "string", "description": "1-4 words shown next to it"},
            "rotate": {"type": "array", "items": {"type": "number"}, "description": "[x, y, z] degrees"},
            "opacity": {"type": "number", "description": "0.15..1; lower to see what's inside"},
            "material": {"type": "string", "enum": MATERIALS},
            "parent": {"type": "string", "description": "name of an earlier object this one is part of: it moves, turns and spins with it"},
            "path": points("tube: the points it runs through, [[x,y,z], …] (a vessel, wire, pipe, DNA strand); closed: true joins the ends"),
            "closed": {"type": "boolean"},
            "profile": points("lathe: the outline spun around the y axis, [[radius, y], …] bottom to top (a bottle, vase, bell, rocket body)"),
            "outline": points("extrude: a flat outline on the ground, [[x, z], …], pushed up by size[1] (a floor plan, a wing, a letter, a country)"),
            "teeth": {"type": "integer", "description": "gear: 6..40"},
            "copies": points("the same object again at each [x, y, z] (atoms, cells, bricks, seats); drawn in one go, so hundreds are fine"),
            "motion": {"type": "object", "description": "{spin: degrees per second, axis?: x|y|z} | {orbit: {around: name or [x,y,z], period: seconds}} | {pulse: {amount: 0.02..0.5, period: seconds}} (a heartbeat) | {bob: {height, period}}; may combine"}
        },
        "required": ["name", "shape"]
    });
    let link = json!({
        "type": "object",
        "properties": { "from": {"type": "string"}, "to": {"type": "string"}, "label": {"type": "string"}, "flow": {"type": "boolean", "description": "dots travel along it (blood, current, data)"} },
        "required": ["from", "to"]
    });
    ToolDef {
        name: "scene3d".into(),
        description: "A 3D model in its own window they can spin, for things easier to grasp in 3D (anatomy, molecules, machines, planets, buildings, how parts stack or fit). Objects are labelled shapes placed in space: box, sphere, cylinder, cone, pyramid, dome, disc, torus, capsule, plane, ring (rings and tori lie flat: a ring is an orbit); tube (through a path of points); lathe (a profile spun round: bottles, bells); extrude (a flat outline pushed up); gear. Build real things from parts: parent makes one object part of another, rotate tilts it, copies repeats it, material (matte, metal, glass, glow) and opacity set its look, motion makes it spin, orbit, pulse or bob. links are arrows between objects; flow: true sends dots along one. Build it, then explain it out loud with a [[3d name]] mark in each sentence so the view turns to that part as you say it. action=focus turns to one object; explode (on: true|false) pulls the parts apart to show how they fit; cut (on: true|false) slices the model open to show the inside; close closes the window.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["show", "focus", "explode", "cut", "close"]},
                "title": {"type": "string"},
                "objects": {"type": "array", "items": obj},
                "links": {"type": "array", "items": link},
                "name": {"type": "string", "description": "focus: the object"},
                "on": {"type": "boolean", "description": "explode, cut: on or off"}
            }
        }),
    }
}

/// A list of points with `n` numbers each, clamped, at most `max` of them.
fn points(v: &Value, n: usize, lo: f64, hi: f64, max: usize) -> Vec<Vec<f64>> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let a: Vec<f64> = p.as_array()?.iter().filter_map(Value::as_f64).filter(|x| x.is_finite()).map(|x| x.clamp(lo, hi)).collect();
            (a.len() >= n).then(|| a[..n].to_vec())
        })
        .take(max)
        .collect()
}

fn colour(v: &Value) -> String {
    let c = v.as_str().unwrap_or("").trim().to_lowercase();
    let c = if c == "gray" { "grey".to_string() } else { c };
    let hex = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit());
    if COLORS.contains(&c.as_str()) || hex {
        c
    } else {
        "white".into()
    }
}

/// How an object moves, with every number in a range that looks right and can't run away.
fn motion(v: &Value, names: &[String]) -> Value {
    let mut out = json!({});
    let num = |v: &Value, lo: f64, hi: f64| v.as_f64().filter(|x| x.is_finite()).map(|x| x.clamp(lo, hi));
    if let Some(spin) = num(&v["spin"], -720.0, 720.0).filter(|s| s.abs() > 0.01) {
        out["spin"] = json!(spin);
        out["axis"] = json!(match v["axis"].as_str() { Some("x") => "x", Some("z") => "z", _ => "y" });
    }
    if v["orbit"].is_object() {
        let around = &v["orbit"]["around"];
        let centre = match around.as_str().map(|n| n.trim().to_lowercase()) {
            Some(n) if names.contains(&n) => json!(n),
            _ => json!(points(&json!([around]), 3, -20.0, 20.0, 1).into_iter().next().unwrap_or(vec![0.0, 0.0, 0.0])),
        };
        out["orbit"] = json!({ "around": centre, "period": num(&v["orbit"]["period"], 2.0, 600.0).unwrap_or(12.0) });
    }
    if v["pulse"].is_object() || v["pulse"].as_bool() == Some(true) {
        out["pulse"] = json!({ "amount": num(&v["pulse"]["amount"], 0.02, 0.5).unwrap_or(0.08), "period": num(&v["pulse"]["period"], 0.3, 20.0).unwrap_or(1.0) });
    }
    if v["bob"].is_object() || v["bob"].as_bool() == Some(true) {
        out["bob"] = json!({ "height": num(&v["bob"]["height"], 0.02, 3.0).unwrap_or(0.2), "period": num(&v["bob"]["period"], 0.5, 30.0).unwrap_or(3.0) });
    }
    out
}

/// Keep what the window can draw: known shapes, numbers in range, at most 80 objects and 600
/// copies, parents that exist (and come first, so there are no loops), links between objects
/// that exist. A shape missing what it needs falls back to a plain one rather than failing.
pub fn sanitize(input: &Value) -> Result<Value, String> {
    let mut names: Vec<String> = vec![];
    let mut objects = vec![];
    let mut copies_left = MAX_COPIES;
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
                1 if k == "size" => vec![v[0]; n],
                _ => (0..n).map(|i| v.get(i).copied().unwrap_or(if k == "size" { v[0] } else { 0.0 })).collect(),
            }
        };
        let path = points(&o["path"], 3, -20.0, 20.0, 40);
        let profile = points(&o["profile"], 2, -12.0, 12.0, 24);
        let outline = points(&o["outline"], 2, -12.0, 12.0, 40);
        let shape = match shape {
            "tube" if path.len() < 2 => "cylinder",
            "lathe" if profile.len() < 2 => "sphere",
            "extrude" if outline.len() < 3 => "box",
            s if SHAPES.contains(&s) => s,
            _ => "box",
        };
        let copies = points(&o["copies"], 3, -20.0, 20.0, copies_left.min(400));
        copies_left -= copies.len();
        let parent = o.get("parent").and_then(Value::as_str).map(|p| p.trim().to_lowercase()).filter(|p| names.contains(p)).unwrap_or_default();
        let material = o.get("material").and_then(Value::as_str).filter(|m| MATERIALS.contains(m)).unwrap_or("matte");
        let mut out = json!({
            "name": name,
            "shape": shape,
            "at": nums("at", 3, 0.0, -20.0, 20.0),
            // A tube's size is its thickness: thin unless they say otherwise.
            "size": nums("size", 3, if shape == "tube" { 0.2 } else { 1.0 }, 0.02, 12.0),
            "color": colour(&o["color"]),
            "label": o.get("label").and_then(Value::as_str).unwrap_or(""),
            "rotate": nums("rotate", 3, 0.0, -360.0, 360.0),
            "opacity": o.get("opacity").and_then(Value::as_f64).filter(|x| x.is_finite()).map(|x| x.clamp(0.15, 1.0)).unwrap_or(if material == "glass" { 0.35 } else { 1.0 }),
            "material": material,
            "parent": parent,
            "motion": motion(&o["motion"], &names),
        });
        match shape {
            "tube" => {
                out["path"] = json!(path);
                out["closed"] = json!(o["closed"].as_bool().unwrap_or(false));
            }
            "lathe" => out["profile"] = json!(profile.into_iter().map(|p| vec![p[0].abs(), p[1]]).collect::<Vec<_>>()),
            "extrude" => out["outline"] = json!(outline),
            "gear" => out["teeth"] = json!(o["teeth"].as_u64().unwrap_or(12).clamp(6, 40)),
            _ => {}
        }
        if !copies.is_empty() {
            out["copies"] = json!(copies);
        }
        names.push(name.to_lowercase());
        objects.push(out);
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
        .map(|l| json!({ "from": l["from"], "to": l["to"], "label": l["label"].as_str().unwrap_or(""), "flow": l["flow"].as_bool().unwrap_or(false) }))
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
        // Pull the parts apart, or slice the model open; `on: false` puts it back.
        view @ ("explode" | "cut") => {
            if SCENE.lock_ok().is_none() {
                return Err("No 3D view is open.".into());
            }
            let on = input.get("on").and_then(Value::as_bool).unwrap_or(true);
            if dismissed() {
                return Ok("They put the 3D view away; leave it closed unless they ask for it.".into());
            }
            if app.get_webview_window(WINDOW).is_none() {
                open(app);
            }
            emit(app, Event::new("scene3d.view", json!({ view: on })));
            Ok(match (view, on) {
                ("explode", true) => "Pulled the parts apart.",
                ("explode", false) => "Put the parts back together.",
                (_, true) => "Sliced the model open.",
                (_, false) => "Closed the model up again.",
            }
            .into())
        }
        _ => {
            let scene = sanitize(input)?;
            let n = names(&scene);
            *SCENE.lock_ok() = Some(scene.clone());
            DISMISSED.store(false, std::sync::atomic::Ordering::SeqCst);
            *SHOWN.lock_ok() = Some(std::time::Instant::now());
            open(app);
            emit(app, Event::new("scene3d.show", scene));
            Ok(format!("Showing {} object(s) in 3D: {}. Use [[3d name]] in your sentences to turn to each.", n.len(), n.join(", ")))
        }
    }
}

/// Turn the view to one object and light it up.
pub fn focus(app: &AppHandle, name: &str, label: &str) -> Result<String, String> {
    let scene = SCENE.lock_ok().clone().ok_or("No 3D view is open.")?;
    let all = names(&scene);
    let want = name.trim().to_lowercase();
    let hit = all.iter().find(|n| n.to_lowercase() == want).or_else(|| all.iter().find(|n| n.to_lowercase().contains(&want) && !want.is_empty()));
    let Some(hit) = hit else {
        return Err(format!("No object called {name}; there are: {}.", all.join(", ")));
    };
    if dismissed() {
        return Ok("The 3D view was put away.".into());
    }
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

/// The 3D window lost focus: if they clicked outside Pixie (another app, the desktop), it
/// closes and stays closed for the rest of this explanation. The notch or a Pixie sheet keeps it.
pub fn blurred(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let Some(w) = h.get_webview_window(WINDOW) else { return };
            if !w.is_focused().unwrap_or(false) && !crate::macos::app_active_in(&h) {
                DISMISSED.store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = w.close();
            }
        });
    });
}

/// The window is gone (they closed it, clicked away, or the model closed it): the rest of this
/// explanation leaves it closed; only a new model opens it again.
pub fn closed() {
    let fresh = SHOWN.lock_ok().is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(1500));
    if !fresh {
        DISMISSED.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The window asks for the scene when it starts.
#[tauri::command]
pub fn scene3d_get() -> Option<Value> {
    SCENE.lock_ok().clone()
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
            "links": [{"from": "left atrium", "to": "Aorta", "flow": true}, {"from": "Aorta", "to": "Lungs"}]
        }))
        .unwrap();
        let o = s["objects"].as_array().unwrap();
        assert_eq!(o.len(), 2, "duplicates and nameless objects are dropped");
        assert_eq!(o[0]["at"], json!([1.0, 2.0, 0.0]));
        assert_eq!(o[0]["size"], json!([1.2, 1.2, 1.2]));
        assert_eq!(o[1]["shape"], "box");
        assert_eq!(o[1]["at"][1], 20.0);
        assert_eq!(s["links"].as_array().unwrap().len(), 1);
        assert_eq!(s["links"][0]["flow"], true);
        assert!(sanitize(&json!({"objects": []})).is_err());
    }

    #[test]
    fn richer_shapes_keep_what_they_need_and_fall_back_without_it() {
        let s = sanitize(&json!({ "objects": [
            {"name": "vessel", "shape": "tube", "path": [[0, 0, 0], [1, 2, 0], [2, 2, 99]], "size": [0.2], "color": "#C0392B", "material": "glow"},
            {"name": "broken tube", "shape": "tube", "path": [[0, 0, 0]]},
            {"name": "bottle", "shape": "lathe", "profile": [[-1, 0], [1, 1], [0.3, 2]], "material": "glass"},
            {"name": "floor", "shape": "extrude", "outline": [[0, 0], [4, 0], [4, 3]], "opacity": 0.01},
            {"name": "cog", "shape": "gear", "teeth": 99, "color": "chartreuse", "material": "chrome"}
        ] }))
        .unwrap();
        let o = s["objects"].as_array().unwrap();
        assert_eq!(o[0]["path"][2], json!([2.0, 2.0, 20.0]));
        assert_eq!(o[0]["color"], "#c0392b");
        assert_eq!(o[1]["shape"], "cylinder", "a tube with one point is a plain cylinder");
        assert_eq!(o[2]["profile"][0], json!([1.0, 0.0]), "a radius is never negative");
        assert_eq!(o[2]["opacity"], 0.35, "glass is see-through unless told otherwise");
        assert_eq!(o[3]["opacity"], 0.15);
        assert_eq!((o[4]["teeth"].as_u64(), o[4]["color"].as_str(), o[4]["material"].as_str()), (Some(40), Some("white"), Some("matte")));
    }

    #[test]
    fn parents_copies_and_motion_stay_within_bounds() {
        let many: Vec<Value> = (0..500).map(|i| json!([i, 0, 0])).collect();
        let s = sanitize(&json!({ "objects": [
            {"name": "sun", "shape": "sphere", "motion": {"spin": 5000, "pulse": true}},
            {"name": "earth", "shape": "sphere", "parent": "Sun", "motion": {"orbit": {"around": "sun", "period": 0.1}, "bob": {"height": 99}}},
            {"name": "moon", "shape": "sphere", "parent": "mars", "motion": {"orbit": {"around": [1, 2, 3]}}},
            {"name": "atoms", "shape": "sphere", "copies": many.clone()},
            {"name": "more", "shape": "sphere", "copies": many}
        ] }))
        .unwrap();
        let o = s["objects"].as_array().unwrap();
        assert_eq!(o[0]["motion"], json!({ "spin": 720.0, "axis": "y", "pulse": { "amount": 0.08, "period": 1.0 } }));
        assert_eq!(o[1]["parent"], "sun");
        assert_eq!(o[1]["motion"]["orbit"], json!({ "around": "sun", "period": 2.0 }));
        assert_eq!(o[1]["motion"]["bob"]["height"], 3.0);
        assert_eq!(o[2]["parent"], "", "a parent that doesn't exist (or comes later) is dropped");
        assert_eq!(o[2]["motion"]["orbit"]["around"], json!([1.0, 2.0, 3.0]));
        assert_eq!(o[3]["copies"].as_array().unwrap().len(), 400);
        assert_eq!(o[4]["copies"].as_array().unwrap().len(), 200, "600 copies in all");
    }
}
