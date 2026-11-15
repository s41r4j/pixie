//! Ready-made things for the canvas, laid out here so the model says *what* to draw and not
//! where every box goes: trees, mind maps, timelines, layered architecture, cycles, comparisons,
//! quadrants, charts from numbers, tables, sticky notes, code and interface wireframes.
//!
//! `canvas` action=draw takes `make`: one of these (or a list), each `{ "kind": …, … }`. The
//! result is plain Excalidraw skeletons (rectangles, ellipses, text, lines, arrows), so
//! everything stays editable. Parts keep the ids the model gave them, which is what
//! `[[canvas id]]` marks and action=edit point at.

use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const BLUE: &str = "#a5d8ff";
const GREEN: &str = "#b2f2bb";
const YELLOW: &str = "#ffec99";
const RED: &str = "#ffc9c9";
const PURPLE: &str = "#d0bfff";
const ORANGE: &str = "#ffd8a8";
const GREY: &str = "#e9ecef";
const INK: &str = "#1e1e1e";
const MUTED: &str = "#868e96";
const PALETTE: [&str; 6] = [BLUE, GREEN, YELLOW, RED, PURPLE, ORANGE];
/// Strong versions for lines and dots, in the same order as [`PALETTE`].
const STRONG: [&str; 6] = ["#1971c2", "#2f9e44", "#f08c00", "#e03131", "#6741d9", "#e8590c"];

/// The most parts one `make` may ask for (nodes, rows, bars…): more than this is unreadable.
const MAX_PARTS: usize = 80;

pub const KINDS: &[&str] = &["tree", "mindmap", "timeline", "layers", "cycle", "compare", "quadrant", "chart", "table", "notes", "code", "wireframe"];

/// A colour name from the model as a canvas fill (for action=edit).
pub fn colour(v: &Value) -> String {
    fill(v, BLUE)
}

/// A named colour or `#rrggbb` → a fill. Unknown names get the default.
fn fill(v: &Value, default: &'static str) -> String {
    match v.as_str().map(str::trim).unwrap_or("") {
        "blue" => BLUE.into(),
        "green" => GREEN.into(),
        "yellow" => YELLOW.into(),
        "red" | "pink" => RED.into(),
        "purple" | "violet" => PURPLE.into(),
        "orange" => ORANGE.into(),
        "grey" | "gray" => GREY.into(),
        "white" => "#ffffff".into(),
        c if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit()) => c.to_string(),
        _ => default.into(),
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Object(o) => ["label", "text", "name", "title"].iter().find_map(|k| o.get(*k).and_then(Value::as_str)).unwrap_or("").trim().to_string(),
        _ => String::new(),
    }
}

fn list<'a>(v: &'a Value, keys: &[&str]) -> &'a [Value] {
    keys.iter().find_map(|k| v.get(*k).and_then(Value::as_array)).map(Vec::as_slice).unwrap_or(&[])
}

/// About how wide text is in the canvas's hand-drawn font.
fn width_of(text: &str, size: f64) -> f64 {
    text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * size * 0.56
}

/// A box big enough for a label: wide labels wrap onto more lines.
fn box_for(label: &str) -> (f64, f64) {
    let w = (width_of(label, 20.0) + 36.0).clamp(120.0, 260.0);
    let lines = (width_of(label, 20.0) / (w - 28.0)).ceil().max(1.0) + label.matches('\n').count() as f64;
    (w, (34.0 + lines * 26.0).max(60.0))
}

/// Collects the shapes of one `make`, shifted to where it sits among the others.
struct Kit {
    out: Vec<Value>,
    prefix: String,
    n: usize,
    used: HashSet<String>,
    /// Parts worth pointing at: (id, label).
    parts: Vec<(String, String)>,
    dy: f64,
}

impl Kit {
    /// The id the model gave (when it's usable and new), or a fresh one.
    fn id(&mut self, given: &Value) -> String {
        let want = given.as_str().map(str::trim).filter(|s| !s.is_empty() && s.len() <= 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        if let Some(w) = want {
            if self.used.insert(w.to_string()) {
                return w.to_string();
            }
        }
        loop {
            self.n += 1;
            let id = format!("{}{}", self.prefix, self.n);
            if self.used.insert(id.clone()) {
                return id;
            }
        }
    }

    fn push(&mut self, mut v: Value) {
        if let Some(y) = v.get("y").and_then(Value::as_f64) {
            v["y"] = json!((y + self.dy).round());
        }
        if let Some(x) = v.get("x").and_then(Value::as_f64) {
            v["x"] = json!(x.round());
        }
        self.out.push(v);
    }

    #[allow(clippy::too_many_arguments)]
    fn shape(&mut self, kind: &str, id: &str, x: f64, y: f64, w: f64, h: f64, label: &str, bg: &str) {
        let mut v = json!({ "type": kind, "id": id, "x": x, "y": y, "width": w.round(), "height": h.round(), "backgroundColor": bg, "fillStyle": "solid", "strokeColor": INK, "roughness": 1 });
        if kind == "rectangle" {
            v["roundness"] = json!({ "type": 3 });
        }
        if !label.is_empty() {
            v["label"] = json!({ "text": label, "fontSize": 20 });
            self.parts.push((id.to_string(), label.replace('\n', " ")));
        }
        self.push(v);
    }

    #[allow(clippy::too_many_arguments)]
    fn rect(&mut self, id: &str, x: f64, y: f64, w: f64, h: f64, label: &str, bg: &str) {
        self.shape("rectangle", id, x, y, w, h, label, bg)
    }

    /// A plain rectangle with no label, id or roundness of its own (bars, bands, frames).
    fn block(&mut self, x: f64, y: f64, w: f64, h: f64, bg: &str, stroke: &str) {
        self.push(json!({ "type": "rectangle", "x": x, "y": y, "width": w.round(), "height": h.round(), "backgroundColor": bg, "fillStyle": "solid", "strokeColor": stroke, "roughness": 1 }));
    }

    fn text(&mut self, x: f64, y: f64, text: &str, size: f64, color: &str) {
        if !text.is_empty() {
            self.push(json!({ "type": "text", "x": x, "y": y, "text": text, "fontSize": size, "strokeColor": color }));
        }
    }

    /// Text centred on `cx`.
    fn centred(&mut self, cx: f64, y: f64, text: &str, size: f64, color: &str) {
        self.text(cx - width_of(text, size) / 2.0, y, text, size, color);
    }

    /// A line through absolute points; `bg` fills it when it closes on itself.
    fn path(&mut self, points: &[(f64, f64)], stroke: &str, bg: Option<&str>, width: f64) {
        let Some(&(x0, y0)) = points.first() else { return };
        let rel: Vec<[f64; 2]> = points.iter().map(|(x, y)| [(x - x0).round(), (y - y0).round()]).collect();
        let mut v = json!({ "type": "line", "x": x0, "y": y0, "points": rel, "strokeColor": stroke, "strokeWidth": width, "roughness": 0 });
        if let Some(bg) = bg {
            v["backgroundColor"] = json!(bg);
            v["fillStyle"] = json!("solid");
        }
        self.push(v);
    }

    /// An arrow (or a plain line) between two shapes by id; the canvas window works out where
    /// it leaves each outline.
    fn link(&mut self, from: &str, to: &str, label: &str, arrow: bool) {
        let mut v = json!({ "type": if arrow { "arrow" } else { "line" }, "start": { "id": from }, "end": { "id": to }, "strokeColor": INK });
        if !label.is_empty() {
            v["label"] = json!({ "text": label, "fontSize": 16 });
        }
        self.out.push(v);
    }

    /// The lowest point drawn so far (for stacking the next `make` under this one).
    fn bottom(&self) -> f64 {
        self.out
            .iter()
            .filter_map(|v| {
                let y = v.get("y")?.as_f64()?;
                let h = v.get("height").and_then(Value::as_f64).unwrap_or(0.0);
                let pts = v.get("points").and_then(Value::as_array).map(|p| p.iter().filter_map(|q| q.get(1).and_then(Value::as_f64)).fold(0.0, f64::max)).unwrap_or(0.0);
                let text = v.get("text").and_then(Value::as_str).map(|t| t.lines().count() as f64 * v.get("fontSize").and_then(Value::as_f64).unwrap_or(20.0) * 1.3).unwrap_or(0.0);
                Some(y + h.max(pts).max(text))
            })
            .fold(0.0, f64::max)
    }
}

// ---- trees and mind maps ----

struct Node {
    id: String,
    label: String,
    bg: String,
    w: f64,
    h: f64,
    kids: Vec<usize>,
    /// Space this node and everything under it needs across the tree.
    span: f64,
}

/// Lay a forest out: `across` runs along siblings, `depth` away from the root. Returns each
/// node's centre in (across, depth).
fn layout(nodes: &mut [Node], roots: &[usize], down: bool, gap: f64) -> HashMap<usize, (f64, f64)> {
    fn span(nodes: &mut [Node], i: usize, down: bool, gap: f64) -> f64 {
        let kids = nodes[i].kids.clone();
        let own = if down { nodes[i].w } else { nodes[i].h };
        let below: f64 = kids.iter().map(|k| span(nodes, *k, down, gap)).sum::<f64>() + gap * kids.len().saturating_sub(1) as f64;
        nodes[i].span = own.max(below);
        nodes[i].span
    }
    fn place(nodes: &[Node], i: usize, start: f64, level: usize, gap: f64, out: &mut HashMap<usize, (f64, f64)>) {
        let n = &nodes[i];
        out.insert(i, (start + n.span / 2.0, level as f64));
        let below: f64 = n.kids.iter().map(|k| nodes[*k].span).sum::<f64>() + gap * n.kids.len().saturating_sub(1) as f64;
        let mut at = start + (n.span - below) / 2.0;
        for k in &n.kids {
            place(nodes, *k, at, level + 1, gap, out);
            at += nodes[*k].span + gap;
        }
    }
    let mut out = HashMap::new();
    let mut at = 0.0;
    for r in roots {
        let s = span(nodes, *r, down, gap);
        place(nodes, *r, at, 0, gap, &mut out);
        at += s + gap * 2.0;
    }
    out
}

fn tree(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let items = list(spec, &["nodes", "items"]);
    if items.is_empty() {
        return Err("tree needs nodes: [{id, label, parent}] (the root has no parent).".into());
    }
    if items.len() > MAX_PARTS {
        return Err(format!("tree: at most {MAX_PARTS} nodes."));
    }
    let down = !matches!(spec["direction"].as_str(), Some("right") | Some("horizontal"));
    let mut nodes: Vec<Node> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut parents: Vec<String> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let label = text_of(it);
        let given = it["id"].as_str().unwrap_or("").to_string();
        let id = k.id(&it["id"]);
        // Parents are looked up by the id the model wrote, or by label.
        for key in [given, label.to_lowercase()] {
            if !key.is_empty() {
                index.entry(key).or_insert(i);
            }
        }
        let (w, h) = box_for(&label);
        nodes.push(Node { id, label, bg: fill(&it["color"], BLUE), w, h, kids: vec![], span: 0.0 });
        parents.push(it["parent"].as_str().unwrap_or("").trim().to_string());
    }
    let mut roots = vec![];
    for (i, p) in parents.iter().enumerate() {
        match index.get(p.as_str()).or_else(|| index.get(&p.to_lowercase())).copied() {
            Some(pi) if pi != i && !p.is_empty() => nodes[pi].kids.push(i),
            _ => roots.push(i),
        }
    }
    // A loop of parents has no root: nothing would be drawn.
    if roots.is_empty() {
        return Err("tree: no node without a parent, so there's no root.".into());
    }
    let pos = layout(&mut nodes, &roots, down, if down { 36.0 } else { 22.0 });
    if pos.len() < nodes.len() {
        return Err("tree: some nodes are their own ancestors.".into());
    }
    let level = if down { nodes.iter().map(|n| n.h).fold(0.0, f64::max) + 70.0 } else { nodes.iter().map(|n| n.w).fold(0.0, f64::max) + 90.0 };
    for (i, n) in nodes.iter().enumerate() {
        let (across, depth) = pos[&i];
        let (x, y) = if down { (across - n.w / 2.0, depth * level) } else { (depth * level, across - n.h / 2.0) };
        let bg = if roots.contains(&i) && n.bg == BLUE { PURPLE.to_string() } else { n.bg.clone() };
        k.rect(&n.id, x, y, n.w, n.h, &n.label, &bg);
    }
    for n in &nodes {
        for kid in &n.kids {
            k.link(&n.id, &nodes[*kid].id, "", true);
        }
    }
    Ok(())
}

fn mindmap(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let centre = text_of(&spec["center"]).to_string();
    let centre = if centre.is_empty() { text_of(&spec["centre"]) } else { centre };
    let branches = list(spec, &["branches", "items"]);
    if centre.is_empty() || branches.is_empty() {
        return Err("mindmap needs center and branches: [{label, children: […]}].".into());
    }
    // One sideways tree per side; the left one is mirrored.
    fn add(k: &mut Kit, nodes: &mut Vec<Node>, v: &Value, bg: &str, depth: usize) -> usize {
        let label = text_of(v);
        let (w, h) = box_for(&label);
        let id = k.id(&v["id"]);
        let i = nodes.len();
        nodes.push(Node { id, label, bg: bg.to_string(), w, h: h.min(if depth > 0 { 52.0 } else { h }), kids: vec![], span: 0.0 });
        if depth < 3 {
            for c in list(v, &["children", "items"]) {
                if nodes.len() >= MAX_PARTS {
                    break;
                }
                let ci = add(k, nodes, c, bg, depth + 1);
                nodes[i].kids.push(ci);
            }
        }
        i
    }
    let (cw, ch) = box_for(&centre);
    let (cw, ch) = (cw + 40.0, ch + 30.0);
    let centre_id = k.id(&spec["id"]);
    let mut sides: [(Vec<Node>, Vec<usize>); 2] = [(vec![], vec![]), (vec![], vec![])];
    for (b, branch) in branches.iter().enumerate() {
        let side = &mut sides[b % 2];
        let root = add(k, &mut side.0, branch, PALETTE[b % PALETTE.len()], 0);
        side.1.push(root);
    }
    let heights: Vec<f64> = sides.iter_mut().map(|(nodes, roots)| if roots.is_empty() { 0.0 } else { layout(nodes, roots, false, 16.0).values().map(|p| p.0).fold(0.0, f64::max) }).collect();
    let tall = heights.iter().cloned().fold(ch, f64::max) + 60.0;
    let cy = tall / 2.0;
    // Leave room on the left for the mirrored side.
    let reach = |nodes: &[Node]| -> f64 {
        let widest = nodes.iter().map(|n| n.w).fold(0.0, f64::max);
        if nodes.is_empty() { 0.0 } else { 3.0 * (widest + 60.0) + widest + 80.0 }
    };
    let cx = reach(&sides[1].0) + cw / 2.0 + 80.0;
    k.shape("ellipse", &centre_id, cx - cw / 2.0, cy - ch / 2.0, cw, ch, &centre, PURPLE);
    for (s, (nodes, roots)) in sides.iter_mut().enumerate() {
        if roots.is_empty() {
            continue;
        }
        let pos = layout(nodes, roots, false, 16.0);
        let level = nodes.iter().map(|n| n.w).fold(0.0, f64::max) + 60.0;
        let top = cy - pos.values().map(|p| p.0).fold(0.0, f64::max) / 2.0 - 20.0;
        for (i, n) in nodes.iter().enumerate() {
            let (across, depth) = pos[&i];
            let out = cw / 2.0 + 70.0 + depth * level;
            let x = if s == 0 { cx + out } else { cx - out - n.w };
            k.rect(&n.id, x, top + across - n.h / 2.0, n.w, n.h, &n.label, &n.bg);
        }
        for r in roots.iter() {
            k.link(&centre_id, &nodes[*r].id, "", false);
        }
        for n in nodes.iter() {
            for kid in &n.kids {
                k.link(&n.id, &nodes[*kid].id, "", false);
            }
        }
    }
    Ok(())
}

// ---- sequences in time and space ----

fn timeline(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let events = list(spec, &["events", "items"]);
    if events.is_empty() || events.len() > 24 {
        return Err("timeline needs 1 to 24 events: [{when, label}].".into());
    }
    let step = 230.0;
    let axis = 190.0;
    k.path(&[(0.0, axis), (step * events.len() as f64, axis)], INK, None, 2.0);
    for (i, e) in events.iter().enumerate() {
        let cx = step * i as f64 + step / 2.0;
        let label = text_of(e);
        let when = text_of(&e["when"]);
        let when = if when.is_empty() { text_of(&e["date"]) } else { when };
        let (w, h) = box_for(&label);
        let (w, h) = (w.min(200.0), h.max(64.0));
        let up = i % 2 == 0;
        let id = k.id(&e["id"]);
        let y = if up { axis - 46.0 - h } else { axis + 46.0 };
        k.rect(&id, cx - w / 2.0, y, w, h, &label, &fill(&e["color"], PALETTE[i % PALETTE.len()]));
        k.path(&[(cx, if up { axis - 46.0 } else { axis + 46.0 }), (cx, axis)], MUTED, None, 1.0);
        k.push(json!({ "type": "ellipse", "x": cx - 7.0, "y": axis - 7.0, "width": 14, "height": 14, "backgroundColor": INK, "fillStyle": "solid", "strokeColor": INK }));
        k.centred(cx, if up { axis + 14.0 } else { axis - 34.0 }, &when, 16.0, INK);
    }
    Ok(())
}

fn layers(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let rows = list(spec, &["layers", "items"]);
    if rows.is_empty() || rows.len() > 10 {
        return Err("layers needs 1 to 10 layers: [{name, items: […]}], top first.".into());
    }
    let sized: Vec<Vec<(String, f64, f64, &Value)>> = rows.iter().map(|r| list(r, &["items", "parts"]).iter().take(8).map(|it| { let l = text_of(it); let (w, h) = box_for(&l); (l, w, h, it) }).collect()).collect();
    let name_w = rows.iter().map(|r| width_of(&text_of(r), 18.0)).fold(0.0, f64::max) + 40.0;
    let inner = sized.iter().map(|r| r.iter().map(|b| b.1 + 24.0).sum::<f64>()).fold(240.0, f64::max);
    let width = name_w + inner + 24.0;
    let mut y = 0.0;
    let mut bands: Vec<(f64, f64)> = vec![];
    for (i, (row, boxes)) in rows.iter().zip(&sized).enumerate() {
        let h = boxes.iter().map(|b| b.2).fold(60.0, f64::max) + 40.0;
        let band = k.id(&row["id"]);
        k.push(json!({ "type": "rectangle", "id": band, "x": 0.0, "y": y, "width": width.round(), "height": h.round(), "backgroundColor": fill(&row["color"], GREY), "fillStyle": "solid", "strokeColor": MUTED, "strokeStyle": "dashed", "roundness": { "type": 3 }, "roughness": 0 }));
        let name = text_of(row);
        k.parts.push((band.clone(), name.clone()));
        k.text(16.0, y + h / 2.0 - 12.0, &name, 18.0, INK);
        let used: f64 = boxes.iter().map(|b| b.1 + 24.0).sum();
        let mut x = name_w + (inner - used) / 2.0 + 12.0;
        for (label, w, bh, it) in boxes {
            let id = k.id(&it["id"]);
            k.rect(&id, x, y + (h - bh) / 2.0, *w, *bh, label, &fill(&it["color"], PALETTE[i % PALETTE.len()]));
            x += w + 24.0;
        }
        bands.push((y, h));
        y += h + 44.0;
    }
    if spec["arrows"].as_bool() != Some(false) {
        for pair in bands.windows(2) {
            let from = pair[0].0 + pair[0].1 + 6.0;
            k.push(json!({ "type": "arrow", "x": width / 2.0, "y": from, "width": 0, "height": (pair[1].0 - 6.0 - from).round(), "points": [[0, 0], [0, (pair[1].0 - 6.0 - from).round()]], "strokeColor": INK }));
        }
    }
    Ok(())
}

fn cycle(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let items = list(spec, &["items", "steps"]);
    if items.len() < 2 || items.len() > 12 {
        return Err("cycle needs 2 to 12 items.".into());
    }
    let n = items.len() as f64;
    let r = (n * 62.0).max(190.0);
    let (cx, cy) = (r + 140.0, r + 60.0);
    let mut ids = vec![];
    for (i, it) in items.iter().enumerate() {
        let label = text_of(it);
        let (w, h) = box_for(&label);
        let a = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::TAU / n;
        let id = k.id(&it["id"]);
        k.rect(&id, cx + r * a.cos() - w / 2.0, cy + r * a.sin() - h / 2.0, w, h, &label, &fill(&it["color"], PALETTE[i % PALETTE.len()]));
        ids.push(id);
    }
    for i in 0..ids.len() {
        let (a, b) = (ids[i].clone(), ids[(i + 1) % ids.len()].clone());
        k.link(&a, &b, "", true);
    }
    let title = text_of(&spec["title"]);
    k.centred(cx, cy - 12.0, &title, 22.0, INK);
    Ok(())
}

fn compare(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let cols = list(spec, &["columns", "items"]);
    if cols.is_empty() || cols.len() > 5 {
        return Err("compare needs 1 to 5 columns: [{title, points: […]}].".into());
    }
    let w = 280.0;
    let wrap = |s: &str| -> String {
        // About 26 characters a line at this width.
        let mut out = String::new();
        let mut line = 0;
        for word in s.split_whitespace() {
            if line > 0 && line + word.chars().count() > 24 {
                out.push_str("\n   ");
                line = 0;
            } else if line > 0 {
                out.push(' ');
            }
            out.push_str(word);
            line += word.chars().count() + 1;
        }
        out
    };
    let bodies: Vec<String> = cols.iter().map(|c| list(c, &["points", "items"]).iter().take(10).map(|p| format!("•  {}", wrap(&text_of(p)))).collect::<Vec<_>>().join("\n")).collect();
    let h = bodies.iter().map(|b| b.lines().count()).max().unwrap_or(1) as f64 * 23.0 + 100.0;
    for (i, (c, body)) in cols.iter().zip(&bodies).enumerate() {
        let x = i as f64 * (w + 30.0);
        let id = k.id(&c["id"]);
        let title = text_of(&c["title"]);
        let title = if title.is_empty() { text_of(c) } else { title };
        k.block(x, 0.0, w, h, "#ffffff", INK);
        k.rect(&id, x, 0.0, w, 56.0, &title, &fill(&c["color"], PALETTE[i % PALETTE.len()]));
        k.text(x + 16.0, 74.0, body, 17.0, INK);
    }
    Ok(())
}

fn quadrant(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let items = list(spec, &["items", "points"]);
    if items.len() > 30 {
        return Err("quadrant: at most 30 items.".into());
    }
    let s = 520.0;
    let (ox, oy) = (0.0, 34.0);
    let names = list(spec, &["quadrants"]);
    for (i, bg) in [YELLOW, GREEN, GREY, BLUE].iter().enumerate() {
        // Top-left, top-right, bottom-left, bottom-right.
        let (x, y) = (ox + (i % 2) as f64 * s / 2.0, oy + (i / 2) as f64 * s / 2.0);
        k.block(x, y, s / 2.0, s / 2.0, bg, MUTED);
        if let Some(n) = names.get(i) {
            k.text(x + 10.0, y + 8.0, &text_of(n), 16.0, MUTED);
        }
    }
    k.centred(ox + s / 2.0, oy + s + 12.0, &text_of(&spec["x_axis"]), 18.0, INK);
    k.text(0.0, 0.0, &text_of(&spec["y_axis"]), 18.0, INK);
    for (i, it) in items.iter().enumerate() {
        let x = it["x"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);
        let y = it["y"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);
        let (px, py) = (ox + x * s, oy + (1.0 - y) * s);
        let id = k.id(&it["id"]);
        let label = text_of(it);
        k.push(json!({ "type": "ellipse", "id": id, "x": px - 9.0, "y": py - 9.0, "width": 18, "height": 18, "backgroundColor": STRONG[i % STRONG.len()], "fillStyle": "solid", "strokeColor": INK }));
        k.parts.push((id, label.clone()));
        k.text(px + 14.0, py - 11.0, &label, 16.0, INK);
    }
    Ok(())
}

// ---- numbers ----

fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches('%').replace(',', "").parse().ok())).filter(|n| n.is_finite())
}

/// A short way to write a value on a chart.
fn short(n: f64) -> String {
    let a = n.abs();
    if a >= 1e9 {
        format!("{:.1}B", n / 1e9)
    } else if a >= 1e6 {
        format!("{:.1}M", n / 1e6)
    } else if a >= 1e4 {
        format!("{:.0}k", n / 1e3)
    } else if (n - n.round()).abs() < 1e-9 {
        format!("{n:.0}")
    } else {
        format!("{n:.1}")
    }
}

/// A round top for the value axis and the step between its gridlines.
fn scale(max: f64) -> (f64, f64) {
    if max <= 0.0 {
        return (1.0, 0.25);
    }
    let mag = 10f64.powf(max.log10().floor());
    let top = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0].iter().map(|m| m * mag).find(|t| *t >= max).unwrap_or(10.0 * mag);
    (top, top / 4.0)
}

fn chart(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let kind = spec["type"].as_str().or(spec["chart"].as_str()).unwrap_or("bar");
    let labels: Vec<String> = list(spec, &["labels", "categories"]).iter().map(text_of).collect();
    // One series from `values`, or several from `series: [{name, values}]`.
    let mut series: Vec<(String, Vec<f64>)> = list(spec, &["series"]).iter().map(|s| (text_of(s), list(s, &["values", "data"]).iter().filter_map(number).collect())).collect();
    if series.is_empty() {
        series.push((String::new(), list(spec, &["values", "data"]).iter().filter_map(number).collect()));
    }
    series.retain(|s| !s.1.is_empty());
    let title = text_of(&spec["title"]);
    k.text(0.0, 0.0, &title, 24.0, INK);
    let top = if title.is_empty() { 10.0 } else { 54.0 };
    if kind == "scatter" {
        return scatter(k, spec, top);
    }
    let Some(longest) = series.iter().map(|s| s.1.len()).max() else {
        return Err("chart needs values: [numbers] (or series: [{name, values}]) and labels.".into());
    };
    if longest > 40 || series.len() > 6 {
        return Err("chart: at most 40 values and 6 series.".into());
    }
    if kind == "pie" {
        return pie(k, &labels, &series[0].1, top);
    }
    if !matches!(kind, "bar" | "line") {
        return Err(format!("chart type \"{kind}\": use bar, line, pie or scatter."));
    }
    let (w, h, left) = ((longest as f64 * 78.0).clamp(360.0, 900.0), 300.0, 64.0);
    let max = series.iter().flat_map(|s| s.1.iter()).cloned().fold(0.0, f64::max);
    if series.iter().flat_map(|s| s.1.iter()).any(|v| *v < 0.0) {
        return Err("chart: negative values aren't drawn yet; shift them or use a table.".into());
    }
    let (ceiling, step) = scale(max);
    let y_of = |v: f64| top + h - (v / ceiling) * h;
    for i in 0..=4 {
        let v = step * i as f64;
        k.path(&[(left, y_of(v)), (left + w, y_of(v))], if i == 0 { INK } else { "#dee2e6" }, None, 1.0);
        let t = short(v);
        k.text(left - 12.0 - width_of(&t, 14.0), y_of(v) - 9.0, &t, 14.0, MUTED);
    }
    k.path(&[(left, top), (left, top + h)], INK, None, 1.0);
    let slot = w / longest as f64;
    for (i, l) in labels.iter().take(longest).enumerate() {
        let short_label: String = if l.chars().count() > 12 { l.chars().take(11).collect::<String>() + "…" } else { l.clone() };
        k.centred(left + slot * (i as f64 + 0.5), top + h + 10.0, &short_label, 14.0, INK);
    }
    for (s, (_, values)) in series.iter().enumerate() {
        if kind == "bar" {
            let bw = (slot * 0.72 / series.len() as f64).min(64.0);
            let group = bw * series.len() as f64;
            for (i, v) in values.iter().enumerate() {
                let x = left + slot * (i as f64 + 0.5) - group / 2.0 + bw * s as f64;
                k.block(x, y_of(*v), bw, (top + h - y_of(*v)).max(1.0), PALETTE[s % PALETTE.len()], INK);
                if series.len() == 1 {
                    k.centred(x + bw / 2.0, y_of(*v) - 22.0, &short(*v), 14.0, INK);
                }
            }
        } else {
            let pts: Vec<(f64, f64)> = values.iter().enumerate().map(|(i, v)| (left + slot * (i as f64 + 0.5), y_of(*v))).collect();
            k.path(&pts, STRONG[s % STRONG.len()], None, 3.0);
            for (x, y) in &pts {
                k.push(json!({ "type": "ellipse", "x": x - 5.0, "y": y - 5.0, "width": 10, "height": 10, "backgroundColor": STRONG[s % STRONG.len()], "fillStyle": "solid", "strokeColor": STRONG[s % STRONG.len()], "roughness": 0 }));
            }
        }
    }
    legend(k, &series.iter().map(|s| s.0.clone()).collect::<Vec<_>>(), left + w + 24.0, top, kind == "line");
    k.centred(left + w / 2.0, top + h + 38.0, &text_of(&spec["x_label"]), 16.0, MUTED);
    k.text(0.0, top - 26.0, &text_of(&spec["y_label"]), 16.0, MUTED);
    Ok(())
}

fn legend(k: &mut Kit, names: &[String], x: f64, y: f64, strong: bool) {
    if names.iter().all(|n| n.is_empty()) {
        return;
    }
    for (i, n) in names.iter().enumerate() {
        let c = if strong { STRONG[i % STRONG.len()] } else { PALETTE[i % PALETTE.len()] };
        k.block(x, y + i as f64 * 28.0, 16.0, 16.0, c, INK);
        k.text(x + 24.0, y + i as f64 * 28.0 - 3.0, n, 16.0, INK);
    }
}

fn pie(k: &mut Kit, labels: &[String], values: &[f64], top: f64) -> Result<(), String> {
    let total: f64 = values.iter().filter(|v| **v > 0.0).sum();
    if total <= 0.0 || values.len() > 12 {
        return Err("pie needs 1 to 12 positive values.".into());
    }
    let r = 150.0;
    let (cx, cy) = (r + 20.0, top + r + 10.0);
    let mut a = -std::f64::consts::FRAC_PI_2;
    for (i, v) in values.iter().enumerate().filter(|(_, v)| **v > 0.0) {
        let sweep = v / total * std::f64::consts::TAU;
        // A slice is a filled outline: the centre, points along the arc, back to the centre.
        let steps = (sweep / 0.12).ceil().max(2.0) as usize;
        let mut pts = vec![(cx, cy)];
        pts.extend((0..=steps).map(|s| { let t = a + sweep * s as f64 / steps as f64; (cx + r * t.cos(), cy + r * t.sin()) }));
        pts.push((cx, cy));
        k.path(&pts, INK, Some(PALETTE[i % PALETTE.len()]), 1.0);
        a += sweep;
    }
    for (i, v) in values.iter().enumerate() {
        let name = labels.get(i).cloned().unwrap_or_default();
        k.block(2.0 * r + 70.0, top + 10.0 + i as f64 * 30.0, 16.0, 16.0, PALETTE[i % PALETTE.len()], INK);
        k.text(2.0 * r + 94.0, top + 7.0 + i as f64 * 30.0, &format!("{name}  {:.0}%", v.max(0.0) / total * 100.0), 16.0, INK);
    }
    Ok(())
}

fn scatter(k: &mut Kit, spec: &Value, top: f64) -> Result<(), String> {
    let pts: Vec<(f64, f64, String)> = list(spec, &["points", "items"]).iter().filter_map(|p| Some((number(&p["x"])?, number(&p["y"])?, text_of(p)))).collect();
    if pts.is_empty() || pts.len() > MAX_PARTS {
        return Err(format!("scatter needs 1 to {MAX_PARTS} points: [{{x, y, label}}]."));
    }
    let (w, h, left) = (520.0, 320.0, 64.0);
    let range = |f: fn(&(f64, f64, String)) -> f64| -> (f64, f64) {
        let (lo, hi) = pts.iter().map(f).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let pad = ((hi - lo) * 0.08).max(1e-9);
        (lo - pad, hi + pad)
    };
    let ((x0, x1), (y0, y1)) = (range(|p| p.0), range(|p| p.1));
    k.path(&[(left, top), (left, top + h), (left + w, top + h)], INK, None, 1.0);
    for (i, (x, y, label)) in pts.iter().enumerate() {
        let (px, py) = (left + (x - x0) / (x1 - x0) * w, top + h - (y - y0) / (y1 - y0) * h);
        k.push(json!({ "type": "ellipse", "x": px - 6.0, "y": py - 6.0, "width": 12, "height": 12, "backgroundColor": STRONG[i % STRONG.len()], "fillStyle": "solid", "strokeColor": INK, "roughness": 0 }));
        k.text(px + 10.0, py - 9.0, label, 14.0, INK);
    }
    for (v, x) in [(x0, left), (x1, left + w)] {
        k.centred(x, top + h + 8.0, &short(v), 14.0, MUTED);
    }
    for (v, y) in [(y0, top + h), (y1, top)] {
        let t = short(v);
        k.text(left - 10.0 - width_of(&t, 14.0), y - 9.0, &t, 14.0, MUTED);
    }
    k.centred(left + w / 2.0, top + h + 32.0, &text_of(&spec["x_label"]), 16.0, MUTED);
    k.text(0.0, top - 26.0, &text_of(&spec["y_label"]), 16.0, MUTED);
    Ok(())
}

// ---- text in boxes ----

fn table(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let cols: Vec<String> = list(spec, &["columns", "headers"]).iter().map(text_of).collect();
    let rows: Vec<Vec<String>> = list(spec, &["rows"]).iter().map(|r| r.as_array().map(|a| a.iter().map(text_of).collect()).unwrap_or_default()).collect();
    let n = cols.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if n == 0 || n > 8 || rows.len() > 30 {
        return Err("table needs columns: […] and rows: [[…]] (at most 8 columns, 30 rows).".into());
    }
    let cell = |r: &[String], c: usize| r.get(c).cloned().unwrap_or_default();
    let widths: Vec<f64> = (0..n).map(|c| rows.iter().map(|r| width_of(&cell(r, c), 16.0)).fold(width_of(&cell(&cols, c), 18.0), f64::max).clamp(60.0, 320.0) + 28.0).collect();
    let total: f64 = widths.iter().sum();
    let (head, rh) = (if cols.is_empty() { 0.0 } else { 46.0 }, 38.0);
    let title = text_of(&spec["title"]);
    k.text(0.0, 0.0, &title, 24.0, INK);
    let top = if title.is_empty() { 0.0 } else { 46.0 };
    let height = head + rh * rows.len() as f64;
    if head > 0.0 {
        k.block(0.0, top, total, head, &fill(&spec["color"], BLUE), INK);
    }
    k.block(0.0, top, total, height, "transparent", INK);
    let mut x = 0.0;
    for (c, w) in widths.iter().enumerate() {
        if c > 0 {
            k.path(&[(x, top), (x, top + height)], MUTED, None, 1.0);
        }
        k.text(x + 14.0, top + 12.0, &cell(&cols, c), 18.0, INK);
        for (r, row) in rows.iter().enumerate() {
            let t: String = cell(row, c);
            let t = if t.chars().count() > 38 { t.chars().take(37).collect::<String>() + "…" } else { t };
            k.text(x + 14.0, top + head + rh * r as f64 + 9.0, &t, 16.0, INK);
        }
        x += w;
    }
    for r in 1..rows.len() {
        let y = top + head + rh * r as f64;
        k.path(&[(0.0, y), (total, y)], "#dee2e6", None, 1.0);
    }
    Ok(())
}

fn notes(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let items = list(spec, &["items", "notes"]);
    if items.is_empty() || items.len() > 30 {
        return Err("notes needs 1 to 30 items.".into());
    }
    let per_row = spec["columns"].as_u64().unwrap_or(if items.len() <= 4 { items.len() as u64 } else { 4 }).clamp(1, 6) as usize;
    for (i, it) in items.iter().enumerate() {
        let id = k.id(&it["id"]);
        let (x, y) = ((i % per_row) as f64 * 220.0, (i / per_row) as f64 * 180.0);
        k.push(json!({ "type": "rectangle", "id": id, "x": x, "y": y, "width": 196, "height": 156, "backgroundColor": fill(&it["color"], YELLOW), "fillStyle": "solid", "strokeColor": "#f08c00", "roughness": 1, "label": { "text": text_of(it), "fontSize": 18, "strokeColor": INK } }));
        k.parts.push((id, text_of(it)));
    }
    Ok(())
}

fn code(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let text = spec["text"].as_str().or(spec["code"].as_str()).unwrap_or("").trim_end().replace('\t', "  ");
    if text.trim().is_empty() || text.lines().count() > 40 {
        return Err("code needs text (at most 40 lines).".into());
    }
    let title = text_of(&spec["title"]);
    let lines = text.lines().count() as f64;
    // The code font is narrower than the hand-drawn one.
    let w = (text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * 9.7 + 44.0).max(width_of(&title, 14.0) + 100.0).max(240.0);
    let h = lines * 20.0 + 62.0;
    let id = k.id(&spec["id"]);
    k.push(json!({ "type": "rectangle", "id": id, "x": 0.0, "y": 0.0, "width": w.round(), "height": h.round(), "backgroundColor": "#1e1e1e", "fillStyle": "solid", "strokeColor": "#1e1e1e", "roundness": { "type": 3 }, "roughness": 0 }));
    k.parts.push((id, if title.is_empty() { "code".into() } else { title.clone() }));
    for (i, c) in ["#ff6b6b", "#ffd43b", "#51cf66"].iter().enumerate() {
        k.push(json!({ "type": "ellipse", "x": 16.0 + i as f64 * 18.0, "y": 14.0, "width": 10, "height": 10, "backgroundColor": c, "fillStyle": "solid", "strokeColor": c, "roughness": 0 }));
    }
    k.text(84.0, 9.0, &title, 14.0, "#adb5bd");
    k.push(json!({ "type": "text", "x": 22.0, "y": 42.0, "text": text, "fontSize": 16, "fontFamily": 8, "strokeColor": "#f1f3f5" }));
    Ok(())
}

fn wireframe(k: &mut Kit, spec: &Value) -> Result<(), String> {
    let parts = list(spec, &["parts", "items"]);
    if parts.len() > 16 {
        return Err("wireframe: at most 16 parts.".into());
    }
    let phone = matches!(spec["device"].as_str(), Some("phone") | Some("mobile"));
    let w = if phone { 320.0 } else { 760.0 };
    let pad = 24.0;
    let bar = if phone { 44.0 } else { 40.0 };
    let frame = k.id(&spec["id"]);
    let title = text_of(&spec["title"]);
    // The parts first, to know how tall the frame is.
    let mut y = bar + 20.0;
    let mut drawn: Vec<Value> = vec![];
    let mut inner = Kit { out: vec![], prefix: format!("{}w", k.prefix), n: 0, used: std::mem::take(&mut k.used), parts: vec![], dy: 0.0 };
    for p in parts {
        let label = text_of(p);
        let kind = p["kind"].as_str().or(p["type"].as_str()).unwrap_or("text");
        let id = inner.id(&p["id"]);
        let iw = w - pad * 2.0;
        match kind {
            "nav" | "navbar" | "tabs" => {
                inner.block(pad, y, iw, 40.0, GREY, MUTED);
                inner.text(pad + 12.0, y + 9.0, &label, 16.0, INK);
                for i in 0..3 {
                    inner.block(pad + iw - 62.0 * (i as f64 + 1.0), y + 12.0, 48.0, 16.0, "#ffffff", MUTED);
                }
                y += 58.0;
            }
            "heading" | "title" => {
                inner.text(pad, y, &label, 26.0, INK);
                y += 48.0;
            }
            "button" => {
                let bw = (width_of(&label, 18.0) + 48.0).clamp(110.0, iw);
                inner.push(json!({ "type": "rectangle", "id": id, "x": pad, "y": y, "width": bw.round(), "height": 44, "backgroundColor": fill(&p["color"], BLUE), "fillStyle": "solid", "strokeColor": INK, "roundness": { "type": 3 }, "label": { "text": label, "fontSize": 18 } }));
                inner.parts.push((id, label));
                y += 62.0;
            }
            "input" | "field" | "search" => {
                inner.push(json!({ "type": "rectangle", "id": id, "x": pad, "y": y, "width": iw, "height": 42, "backgroundColor": "#ffffff", "fillStyle": "solid", "strokeColor": MUTED, "roundness": { "type": 3 }, "roughness": 0 }));
                inner.text(pad + 12.0, y + 11.0, &label, 16.0, MUTED);
                inner.parts.push((id, label));
                y += 60.0;
            }
            "image" | "picture" | "video" | "map" => {
                let ih = if phone { 150.0 } else { 190.0 };
                inner.push(json!({ "type": "rectangle", "id": id, "x": pad, "y": y, "width": iw, "height": ih, "backgroundColor": GREY, "fillStyle": "solid", "strokeColor": MUTED, "roughness": 0 }));
                inner.path(&[(pad, y), (pad + iw, y + ih)], MUTED, None, 1.0);
                inner.path(&[(pad + iw, y), (pad, y + ih)], MUTED, None, 1.0);
                inner.centred(pad + iw / 2.0, y + ih / 2.0 - 10.0, &label, 16.0, INK);
                inner.parts.push((id, label));
                y += ih + 18.0;
            }
            "list" => {
                let rows = list(p, &["items"]);
                let count = if rows.is_empty() { 3 } else { rows.len().min(6) };
                for i in 0..count {
                    inner.push(json!({ "type": "ellipse", "x": pad, "y": y + 6.0, "width": 22, "height": 22, "backgroundColor": GREY, "fillStyle": "solid", "strokeColor": MUTED, "roughness": 0 }));
                    match rows.get(i) {
                        Some(r) => inner.text(pad + 34.0, y + 6.0, &text_of(r), 16.0, INK),
                        None => inner.block(pad + 34.0, y + 12.0, iw * 0.6, 10.0, GREY, GREY),
                    }
                    inner.path(&[(pad, y + 38.0), (pad + iw, y + 38.0)], "#dee2e6", None, 1.0);
                    y += 44.0;
                }
                y += 10.0;
            }
            "card" => {
                inner.push(json!({ "type": "rectangle", "id": id, "x": pad, "y": y, "width": iw, "height": 96, "backgroundColor": "#ffffff", "fillStyle": "solid", "strokeColor": INK, "roundness": { "type": 3 }, "label": { "text": label, "fontSize": 18 } }));
                inner.parts.push((id, label));
                y += 112.0;
            }
            // Body text: the words when given, grey lines standing in for them otherwise.
            _ => {
                if label.is_empty() {
                    for (i, f) in [1.0, 0.92, 0.6].iter().enumerate() {
                        inner.block(pad, y + i as f64 * 18.0, iw * f, 9.0, GREY, GREY);
                    }
                    y += 66.0;
                } else {
                    inner.text(pad, y, &label, 16.0, INK);
                    y += 24.0 * label.lines().count() as f64 + 14.0;
                }
            }
        }
        drawn.append(&mut inner.out);
    }
    k.used = std::mem::take(&mut inner.used);
    let h = (y + 16.0).max(if phone { 420.0 } else { 300.0 });
    k.push(json!({ "type": "rectangle", "id": frame, "x": 0.0, "y": 0.0, "width": w, "height": h.round(), "backgroundColor": "#ffffff", "fillStyle": "solid", "strokeColor": INK, "strokeWidth": 2, "roundness": { "type": 3 }, "roughness": 0 }));
    k.parts.push((frame, if title.is_empty() { "wireframe".into() } else { title.clone() }));
    k.path(&[(0.0, bar), (w, bar)], INK, None, 1.0);
    if phone {
        k.centred(w / 2.0, 12.0, &title, 16.0, INK);
    } else {
        for i in 0..3 {
            k.push(json!({ "type": "ellipse", "x": 14.0 + i as f64 * 20.0, "y": 13.0, "width": 12, "height": 12, "backgroundColor": GREY, "fillStyle": "solid", "strokeColor": MUTED, "roughness": 0 }));
        }
        k.text(86.0, 10.0, &title, 16.0, INK);
    }
    for v in drawn {
        k.push(v);
    }
    k.parts.append(&mut inner.parts);
    Ok(())
}

/// What `make` drew: the shapes, and the parts worth pointing at (id, label).
pub struct Made {
    pub elements: Vec<Value>,
    pub parts: Vec<(String, String)>,
}

/// Turn `make` (one spec or a list) into shapes, stacked one under the other. `taken` are ids
/// already on the canvas, so new parts don't collide with them.
pub fn make(spec: &Value, taken: &HashSet<String>, serial: u64) -> Result<Made, String> {
    let specs: Vec<&Value> = match spec {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![spec],
        _ => return Err(format!("make takes an object with a kind ({}), or a list of them.", KINDS.join(", "))),
    };
    if specs.is_empty() || specs.len() > 6 {
        return Err("make: 1 to 6 things at a time.".into());
    }
    let mut all = Made { elements: vec![], parts: vec![] };
    let mut used = taken.clone();
    let mut dy = 0.0;
    for (i, s) in specs.iter().enumerate() {
        let kind = s["kind"].as_str().unwrap_or("").to_lowercase();
        let mut k = Kit { out: vec![], prefix: format!("k{serial}{}", (b'a' + i as u8) as char), n: 0, used: std::mem::take(&mut used), parts: vec![], dy };
        match kind.as_str() {
            "tree" | "org" | "hierarchy" => tree(&mut k, s),
            "mindmap" | "mind map" | "mind_map" => mindmap(&mut k, s),
            "timeline" => timeline(&mut k, s),
            "layers" | "architecture" | "stack" => layers(&mut k, s),
            "cycle" | "loop" => cycle(&mut k, s),
            "compare" | "comparison" | "columns" => compare(&mut k, s),
            "quadrant" | "matrix" => quadrant(&mut k, s),
            "chart" | "bar" | "line" | "pie" | "scatter" => {
                if kind == "chart" {
                    chart(&mut k, s)
                } else {
                    let mut with_type = (*s).clone();
                    with_type["type"] = json!(kind);
                    chart(&mut k, &with_type)
                }
            }
            "table" => table(&mut k, s),
            "notes" | "stickies" | "sticky" => notes(&mut k, s),
            "code" => code(&mut k, s),
            "wireframe" | "mockup" | "ui" => wireframe(&mut k, s),
            other => Err(format!("make kind \"{other}\": use one of {}.", KINDS.join(", "))),
        }?;
        // Start each thing at the left edge, whatever room its layout left there.
        let left = k.out.iter().filter_map(|v| v.get("x").and_then(Value::as_f64)).fold(f64::MAX, f64::min);
        if left.is_finite() && left != f64::MAX && left.abs() > 0.5 {
            for v in k.out.iter_mut() {
                if let Some(x) = v.get("x").and_then(Value::as_f64) {
                    v["x"] = json!((x - left).round());
                }
            }
        }
        dy = k.bottom() + 90.0;
        used = k.used;
        all.elements.append(&mut k.out);
        all.parts.append(&mut k.parts);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn made(spec: Value) -> Made {
        make(&spec, &HashSet::new(), 1).expect("make")
    }

    fn boxes(m: &Made) -> Vec<(String, f64, f64, f64, f64)> {
        m.elements
            .iter()
            .filter(|e| e["label"].is_object() && e["type"] == "rectangle")
            .map(|e| (e["id"].as_str().unwrap().to_string(), e["x"].as_f64().unwrap(), e["y"].as_f64().unwrap(), e["width"].as_f64().unwrap(), e["height"].as_f64().unwrap()))
            .collect()
    }

    fn overlap(a: &(String, f64, f64, f64, f64), b: &(String, f64, f64, f64, f64)) -> bool {
        a.1 < b.1 + b.3 && b.1 < a.1 + a.3 && a.2 < b.2 + b.4 && b.2 < a.2 + a.4
    }

    fn links_are_real(m: &Made) {
        let ids: HashSet<&str> = m.elements.iter().filter_map(|e| e["id"].as_str()).collect();
        for e in m.elements.iter().filter(|e| e["start"].is_object()) {
            assert!(ids.contains(e["start"]["id"].as_str().unwrap()) && ids.contains(e["end"]["id"].as_str().unwrap()), "{e}");
        }
    }

    #[test]
    fn a_tree_puts_children_under_parents_without_overlap() {
        let m = made(json!({ "kind": "tree", "nodes": [
            { "id": "ceo", "label": "CEO" }, { "id": "cto", "label": "CTO", "parent": "ceo" }, { "id": "cfo", "label": "CFO", "parent": "ceo" },
            { "label": "Platform", "parent": "cto" }, { "label": "Apps", "parent": "cto" }, { "label": "A very long team name that wraps", "parent": "cfo" }
        ] }));
        let b = boxes(&m);
        assert_eq!(b.len(), 6);
        for (i, x) in b.iter().enumerate() {
            for y in &b[i + 1..] {
                assert!(!overlap(x, y), "{x:?} over {y:?}");
            }
        }
        let y = |id: &str| b.iter().find(|n| n.0 == id).unwrap().2;
        assert!(y("ceo") < y("cto") && (y("cto") - y("cfo")).abs() < 1.0);
        assert_eq!(m.elements.iter().filter(|e| e["type"] == "arrow").count(), 5);
        links_are_real(&m);
        // The ids the model gave are kept, so it can point at them.
        assert!(m.parts.iter().any(|p| p.0 == "ceo" && p.1 == "CEO"));
    }

    #[test]
    fn a_sideways_tree_grows_to_the_right() {
        let m = made(json!({ "kind": "tree", "direction": "right", "nodes": [{ "id": "a", "label": "Start" }, { "id": "b", "label": "Next", "parent": "a" }, { "id": "c", "label": "Other", "parent": "a" }] }));
        let b = boxes(&m);
        let at = |id: &str| b.iter().find(|n| n.0 == id).unwrap().clone();
        assert!(at("a").1 < at("b").1 && (at("b").1 - at("c").1).abs() < 1.0 && !overlap(&at("b"), &at("c")));
    }

    #[test]
    fn a_tree_with_no_root_or_a_loop_is_refused() {
        assert!(make(&json!({ "kind": "tree", "nodes": [{ "id": "a", "label": "A", "parent": "b" }, { "id": "b", "label": "B", "parent": "a" }] }), &HashSet::new(), 1).is_err());
        assert!(make(&json!({ "kind": "tree", "nodes": [] }), &HashSet::new(), 1).is_err());
    }

    #[test]
    fn a_mind_map_spreads_to_both_sides() {
        let m = made(json!({ "kind": "mindmap", "center": "Launch", "branches": [
            { "label": "Marketing", "children": ["Blog", "Video", { "label": "Social", "children": ["X", "LinkedIn"] }] },
            { "label": "Product", "children": ["Pricing", "Docs"] }, { "label": "Sales", "children": ["Demo"] }
        ] }));
        let centre = m.elements.iter().find(|e| e["type"] == "ellipse").unwrap();
        let cx = centre["x"].as_f64().unwrap() + centre["width"].as_f64().unwrap() / 2.0;
        let b = boxes(&m);
        assert_eq!(b.len(), 11);
        assert!(b.iter().any(|n| n.1 > cx) && b.iter().any(|n| n.1 + n.3 < cx));
        assert!(b.iter().all(|n| n.1 >= 0.0 && n.2 >= 0.0), "nothing off the top-left: {b:?}");
        for (i, x) in b.iter().enumerate() {
            for y in &b[i + 1..] {
                assert!(!overlap(x, y), "{x:?} over {y:?}");
            }
        }
        links_are_real(&m);
    }

    #[test]
    fn charts_come_from_the_numbers() {
        let bar = made(json!({ "kind": "chart", "type": "bar", "title": "Sales", "labels": ["Q1", "Q2", "Q3"], "values": [10, 40, 25] }));
        let bars: Vec<f64> = bar.elements.iter().filter(|e| e["type"] == "rectangle" && e["backgroundColor"] == BLUE).map(|e| e["height"].as_f64().unwrap()).collect();
        assert_eq!(bars.len(), 3);
        assert!(bars[1] > bars[2] && bars[2] > bars[0], "{bars:?}");
        assert!((bars[1] / bars[0] - 4.0).abs() < 0.1, "heights follow the values");
        let line = made(json!({ "kind": "line", "labels": ["a", "b"], "series": [{ "name": "x", "values": [1, 2] }, { "name": "y", "values": [2, 1] }] }));
        assert_eq!(line.elements.iter().filter(|e| e["type"] == "line" && e["strokeWidth"] == 3.0).count(), 2);
        let pie = made(json!({ "kind": "chart", "type": "pie", "labels": ["A", "B"], "values": [3, 1] }));
        assert_eq!(pie.elements.iter().filter(|e| e["type"] == "line" && e["backgroundColor"].is_string()).count(), 2);
        assert!(pie.elements.iter().any(|e| e["text"].as_str().is_some_and(|t| t.contains("75%"))));
        let dots = made(json!({ "kind": "scatter", "points": [{ "x": 1, "y": 2, "label": "a" }, { "x": 3, "y": 1, "label": "b" }] }));
        assert_eq!(dots.elements.iter().filter(|e| e["type"] == "ellipse").count(), 2);
        assert!(make(&json!({ "kind": "chart", "values": [] }), &HashSet::new(), 1).is_err());
        assert!(make(&json!({ "kind": "chart", "values": [1, -2] }), &HashSet::new(), 1).is_err());
    }

    #[test]
    fn the_value_axis_ends_on_a_round_number() {
        assert_eq!(scale(37.0), (40.0, 10.0));
        assert_eq!(scale(310.0), (400.0, 100.0));
        assert_eq!(scale(100.0), (100.0, 25.0));
        assert_eq!(scale(0.0), (1.0, 0.25));
        assert_eq!(short(1_250_000.0), "1.2M");
        assert_eq!(short(42.0), "42");
    }

    #[test]
    fn every_kind_draws_something_and_ids_never_collide() {
        let specs = json!([
            { "kind": "timeline", "events": [{ "when": "2021", "label": "Founded" }, { "when": "2023", "label": "Series A" }] },
            { "kind": "layers", "layers": [{ "name": "Web", "items": ["React", "Next"] }, { "name": "Data", "items": ["Postgres"] }] },
            { "kind": "cycle", "items": ["Plan", "Build", "Ship"] },
            { "kind": "compare", "columns": [{ "title": "Pros", "points": ["Fast", "Cheap"] }, { "title": "Cons", "points": ["New"] }] },
            { "kind": "quadrant", "x_axis": "Effort", "y_axis": "Impact", "items": [{ "label": "Search", "x": 0.2, "y": 0.9 }] },
            { "kind": "table", "columns": ["Plan", "Price"], "rows": [["Free", "$0"], ["Pro", "$12"]] }
        ]);
        let m = made(specs);
        let ids: Vec<&str> = m.elements.iter().filter_map(|e| e["id"].as_str()).collect();
        assert_eq!(ids.len(), ids.iter().collect::<HashSet<_>>().len(), "duplicate ids");
        links_are_real(&m);
        // Each one sits below the one before.
        let founded = m.elements.iter().find(|e| e["label"]["text"] == "Founded").unwrap()["y"].as_f64().unwrap();
        let react = m.elements.iter().find(|e| e["label"]["text"] == "React").unwrap()["y"].as_f64().unwrap();
        let plan = m.elements.iter().find(|e| e["label"]["text"] == "Plan").unwrap()["y"].as_f64().unwrap();
        assert!(founded < react && react < plan);
        let more = made(json!([
            { "kind": "notes", "items": ["One", "Two", "Three"] },
            { "kind": "code", "title": "main.rs", "text": "fn main() {\n    println!(\"hi\");\n}" },
            { "kind": "wireframe", "device": "phone", "title": "Sign in", "parts": [{ "kind": "heading", "label": "Welcome" }, { "kind": "input", "label": "Email" }, { "kind": "button", "id": "go", "label": "Continue" }, { "kind": "list" }, { "kind": "image", "label": "Photo" }] }
        ]));
        assert!(more.parts.iter().any(|p| p.0 == "go"));
        assert!(more.elements.len() > 20);
    }

    #[test]
    fn ids_on_the_canvas_already_are_not_reused() {
        let taken: HashSet<String> = ["ceo".to_string()].into();
        let m = make(&json!({ "kind": "tree", "nodes": [{ "id": "ceo", "label": "CEO" }, { "label": "CTO", "parent": "ceo" }] }), &taken, 7).unwrap();
        assert!(m.elements.iter().all(|e| e["id"] != "ceo"));
        // The child still found its parent by the id the model wrote.
        assert_eq!(m.elements.iter().filter(|e| e["type"] == "arrow").count(), 1);
    }

    /// Not a check: writes what each kind draws to `$PIXIE_KIT_DUMP/<name>.json`, to look at
    /// in the canvas preview (`?preview=canvas&draw=/<name>.json`).
    #[test]
    #[ignore]
    fn dump_samples() {
        let Ok(dir) = std::env::var("PIXIE_KIT_DUMP") else { return };
        let samples = [
            ("kit-plan", json!([
                { "kind": "mindmap", "center": "Product launch", "branches": [
                    { "label": "Marketing", "children": ["Blog post", "Launch video", { "label": "Social", "children": ["X", "LinkedIn"] }] },
                    { "label": "Product", "children": ["Pricing page", "Docs", "Onboarding"] },
                    { "label": "Sales", "children": ["Demo script", "Outreach list"] },
                    { "label": "Support", "children": ["Help articles", "Status page"] } ] },
                { "kind": "timeline", "events": [{ "when": "Mar 3", "label": "Private beta" }, { "when": "Apr 14", "label": "Pricing final" }, { "when": "May 6", "label": "Public launch" }, { "when": "Jun 1", "label": "First review" }] }
            ])),
            ("kit-numbers", json!([
                { "kind": "chart", "type": "bar", "title": "Revenue by quarter ($k)", "labels": ["Q1", "Q2", "Q3", "Q4"], "series": [{ "name": "2024", "values": [120, 150, 170, 210] }, { "name": "2025", "values": [160, 190, 240, 310] }] },
                { "kind": "chart", "type": "line", "title": "Weekly active users", "labels": ["W1", "W2", "W3", "W4", "W5", "W6"], "values": [1200, 1350, 1300, 1700, 2100, 2600] },
                { "kind": "chart", "type": "pie", "title": "Where time goes", "labels": ["Meetings", "Deep work", "Email", "Other"], "values": [35, 40, 15, 10] },
                { "kind": "table", "title": "Plans", "columns": ["Plan", "Price", "Seats", "Support"], "rows": [["Free", "$0", "1", "Community"], ["Pro", "$12 / month", "5", "Email"], ["Team", "$40 / month", "Unlimited", "Priority"]] }
            ])),
            ("kit-system", json!([
                { "kind": "layers", "layers": [{ "name": "Clients", "items": ["Web app", "iOS", "Android"] }, { "name": "API", "items": ["Gateway", "Auth", "Billing"] }, { "name": "Data", "items": ["Postgres", "Redis", "S3"] }] },
                { "kind": "tree", "nodes": [{ "id": "ceo", "label": "CEO" }, { "id": "cto", "label": "CTO", "parent": "ceo" }, { "id": "cfo", "label": "CFO", "parent": "ceo" }, { "id": "coo", "label": "COO", "parent": "ceo" }, { "label": "Platform", "parent": "cto" }, { "label": "Apps", "parent": "cto" }, { "label": "Finance", "parent": "cfo" }, { "label": "People", "parent": "coo" }, { "label": "Support", "parent": "coo" }] },
                { "kind": "cycle", "title": "Each sprint", "items": ["Plan", "Build", "Review", "Ship", "Learn"] }
            ])),
            ("kit-ui", json!([
                { "kind": "wireframe", "device": "browser", "title": "pixie.app/pricing", "parts": [{ "kind": "nav", "label": "Pixie" }, { "kind": "heading", "label": "Simple pricing" }, { "kind": "text" }, { "kind": "card", "label": "Pro · $12 a month" }, { "kind": "button", "label": "Start free trial" }] },
                { "kind": "wireframe", "device": "phone", "title": "Sign in", "parts": [{ "kind": "heading", "label": "Welcome back" }, { "kind": "input", "label": "Email" }, { "kind": "input", "label": "Password" }, { "kind": "button", "label": "Continue" }, { "kind": "list", "items": ["Sign in with Google", "Sign in with Apple"] }] },
                { "kind": "compare", "columns": [{ "title": "Build it", "points": ["Fits us exactly", "Three months of work", "We own the upkeep"] }, { "title": "Buy it", "points": ["Live next week", "$400 a month", "Their roadmap, not ours"] }] },
                { "kind": "quadrant", "x_axis": "Effort →", "y_axis": "Impact ↑", "quadrants": ["Quick wins", "Big bets", "Fill-ins", "Avoid"], "items": [{ "label": "Search", "x": 0.2, "y": 0.85 }, { "label": "Redesign", "x": 0.8, "y": 0.8 }, { "label": "Dark mode", "x": 0.3, "y": 0.3 }, { "label": "Rewrite", "x": 0.9, "y": 0.2 }] },
                { "kind": "notes", "items": ["Ask legal about the new terms", "Budget sign-off by Friday", "Book the launch venue"] },
                { "kind": "code", "title": "retry.ts", "text": "async function retry<T>(fn: () => Promise<T>, tries = 3) {\n  for (let i = 0; ; i++) {\n    try { return await fn(); }\n    catch (e) { if (i === tries - 1) throw e; }\n  }\n}" }
            ])),
        ];
        for (name, spec) in samples {
            let m = make(&spec, &HashSet::new(), 1).expect(name);
            let input = json!({ "action": "draw", "title": name, "elements": m.elements });
            std::fs::write(std::path::Path::new(&dir).join(format!("{name}.json")), input.to_string()).unwrap();
        }
    }

    #[test]
    fn unknown_kinds_and_bad_shapes_say_what_to_send() {
        let e = make(&json!({ "kind": "hologram" }), &HashSet::new(), 1).err().unwrap();
        assert!(e.contains("tree") && e.contains("chart"));
        assert!(make(&json!("tree"), &HashSet::new(), 1).is_err());
    }
}
