//! The app side of the tools: speech, pointer, Accessibility, plan/progress, asking, jobs,
//! the browser and connectors.

use crate::{browser, emit, jobs, overlay, speech, AppState};
use openclicky_core::prompt::Mode;
use openclicky_core::Event;
use openclicky_llm::ToolDef;
use openclicky_tools::{Host, ToolCtx, ToolOutput};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Copy to / read from the general pasteboard.
async fn clipboard(action: &str, text: &str) -> ToolOutput {
    #[cfg(not(target_os = "macos"))]
    {
        // Windows / Linux: the system clipboard directly (keeps every character intact).
        let (action, text) = (action.to_string(), text.to_string());
        return tauri::async_runtime::spawn_blocking(move || {
            let mut cb = match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(_) => return ToolOutput::err("Couldn't reach the clipboard."),
            };
            match action.as_str() {
                "copy" => match cb.set_text(text.clone()) {
                    Ok(()) => ToolOutput::ok(format!("Copied {} characters; they can paste with Ctrl+V.", text.chars().count())),
                    Err(_) => ToolOutput::err("Couldn't copy."),
                },
                "read" => match cb.get_text() {
                    Ok(t) if !t.trim().is_empty() => ToolOutput::ok(t.chars().take(20_000).collect::<String>()),
                    _ => ToolOutput::ok("The clipboard has no text."),
                },
                _ => ToolOutput::err("action must be copy or read"),
            }
        })
        .await
        .unwrap_or_else(|_| ToolOutput::err("Couldn't reach the clipboard."));
    }
    #[allow(unreachable_code)]
    clipboard_mac(action, text).await
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
async fn clipboard_mac(action: &str, text: &str) -> ToolOutput {
    use tokio::io::AsyncWriteExt;
    match action {
        "copy" => {
            let child = tokio::process::Command::new("/usr/bin/pbcopy").stdin(std::process::Stdio::piped()).spawn();
            let Ok(mut child) = child else { return ToolOutput::err("Couldn't reach the clipboard.") };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            match child.wait().await {
                Ok(s) if s.success() => ToolOutput::ok(format!("Copied {} characters; they can paste with ⌘V.", text.chars().count())),
                _ => ToolOutput::err("Couldn't copy."),
            }
        }
        "read" => match tokio::process::Command::new("/usr/bin/pbpaste").output().await {
            Ok(o) => {
                let t = String::from_utf8_lossy(&o.stdout);
                if t.trim().is_empty() {
                    ToolOutput::ok("The clipboard has no text.")
                } else {
                    ToolOutput::ok(t.chars().take(20_000).collect::<String>())
                }
            }
            Err(_) => ToolOutput::err("Couldn't read the clipboard."),
        },
        _ => ToolOutput::err("action must be copy or read."),
    }
}

/// mac_step press / focus / menu / type / key / open / scroll / find. Every action that can
/// change the screen ends with the fresh screen state (ids stay stable for the request).
async fn mac_step(app: &AppHandle, action: &str, input: &Value) -> ToolOutput {
    let st = app.state::<AppState>();
    // type with x, y: click into that field first, so it lands in the right pane.
    let mut clicked = String::new();
    if action == "type" && input.get("x").and_then(Value::as_f64).is_some() && input.get("y").and_then(Value::as_f64).is_some() {
        let at = json!({ "x": input["x"], "y": input["y"] });
        match crate::buddy::act(app, "click", &at).await {
            Ok(r) => clicked = r.lines().next().unwrap_or("").to_string(),
            Err(e) => return ToolOutput::err(format!("Couldn't click into the field first: {e}")),
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    // A control id (e12) is Pixie's own: act on that control where it is now.
    let target = s(input, "target");
    let mut call = ("mac_step", input.clone());
    let mut at: Option<(f64, f64)> = None;
    if matches!(action, "press" | "focus" | "find") {
        if let Some(hit) = crate::screen::element(target) {
            let (f, label, raw) = match hit {
                Ok(h) => h,
                Err(e) => return ToolOutput::err(e),
            };
            let (x, y) = (f.x + f.w / 2.0, f.y + f.h / 2.0);
            at = Some((x, y));
            if action == "find" {
                return ToolOutput::ok(json!({ "id": target, "label": label, "frame": { "x": f.x, "y": f.y, "w": f.w, "h": f.h } }).to_string());
            }
            call = if cfg!(target_os = "macos") {
                (if action == "press" { "press_point" } else { "focus_point" }, json!({ "x": x, "y": y, "label": label }))
            } else {
                // The native layer still knows its own numbering from the latest snapshot.
                let mut v = input.clone();
                v["target"] = json!(raw);
                ("mac_step", v)
            };
        }
    }
    let r = st.helper.call(call.0, call.1, Duration::from_secs(10)).await;
    let v = match r {
        Ok(v) => v,
        Err(e) => return ToolOutput::err(e),
    };
    if action == "find" {
        return ToolOutput::ok(v.to_string());
    }
    if let Some(f) = v.get("frame").filter(|f| f.is_object()) {
        if at.is_none() {
            at = Some((f["x"].as_f64().unwrap_or(0.0) + f["w"].as_f64().unwrap_or(0.0) / 2.0, f["y"].as_f64().unwrap_or(0.0) + f["h"].as_f64().unwrap_or(0.0) / 2.0));
        }
    }
    let first = if action == "type" {
        let into = &v["into"];
        let f = &into["frame"];
        if f.is_object() {
            let _ = overlay::highlight(app, &json!({ "x": f["x"], "y": f["y"], "w": f["w"], "h": f["h"], "label": "typed here" }));
        }
        let place = if into.is_object() {
            let where_ = if f.is_object() { format!(" at ({}, {}, {}×{})", f["x"], f["y"], f["w"], f["h"]) } else { String::new() };
            format!("the focused {} \"{}\"{where_}", into["role"].as_str().unwrap_or("element").trim_start_matches("AX"), into["label"].as_str().unwrap_or(""))
        } else {
            "the focused field".into()
        };
        let pre = if clicked.is_empty() { String::new() } else { format!("{clicked}\n") };
        format!(
            "{pre}Pasted {} characters into {} in {}. If that isn't the field you meant (another pane, a side panel), say so, click the right field and type again; don't press return until it's in the right place.",
            v["inserted"],
            place,
            v["app"].as_str().unwrap_or("their app")
        )
    } else {
        // Show where it happened.
        if let Some(f) = v.get("frame").filter(|f| f.is_object()) {
            let _ = overlay::highlight(app, &json!({ "x": f["x"], "y": f["y"], "w": f["w"], "h": f["h"], "label": v.get("pressed").or(v.get("label")).cloned().unwrap_or(Value::Null) }));
        }
        let what = match action {
            "press" | "click" => format!("Pressed \"{}\".", v["pressed"].as_str().unwrap_or(target)),
            "focus" => format!("Focused \"{}\".", v["focused"].as_str().unwrap_or(target)),
            "menu" => format!("Chose {target}."),
            "key" => format!("Pressed {target}."),
            "open" => format!("Opened {target}."),
            "scroll" => format!("Scrolled {target}."),
            _ => v.to_string(),
        };
        what
    };
    let now = crate::screen::after_action(app, at, crate::screen::settle_for(action)).await;
    ToolOutput::ok(format!("{first}\n{now}"))
}

pub struct AppHost {
    pub app: AppHandle,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

#[async_trait::async_trait]
impl Host for AppHost {
    async fn call(&self, ctx: &ToolCtx, name: &str, input: &Value) -> Option<ToolOutput> {
        let app = &self.app;
        let st = app.state::<AppState>();
        let job = ctx.job_id.as_deref();
        let out: ToolOutput = match name {
            "highlight" => overlay::highlight(app, input).into(),
            "draw" => overlay::draw(app, input).into(),
            "canvas" => crate::canvas::call(app, input).await.into(),
            "scene3d" => crate::scene3d::call(app, input).await.into(),
            "pixie" => crate::myself::call(app, input).await.into(),
            "memory" => crate::memory::call(app, input).await.into(),
            "say" => {
                // A line may carry marks ([[circle e12]]) drawn as it's spoken.
                let (text, marks) = crate::narrate::split(s(input, "text"));
                let id = format!("say-{}", rand::random::<u32>());
                emit(app, Event::new("speech.segment", json!({ "id": id, "text": text, "job": job })));
                if st.settings.lock().map(|s| s.speak_answers).unwrap_or(true) {
                    crate::narrate::on_start_of(&id, marks);
                    speech::speak(app, &id, &text);
                } else {
                    crate::narrate::run(app, marks);
                }
                ToolOutput::ok("Said it.")
            }
            "mac_step" if matches!(s(input, "action"), "click" | "double_click" | "right_click" | "drag") => {
                if job.is_some() && ctx.mode != Mode::Live {
                    return Some(ToolOutput::err("Background jobs don't click around in their apps; ask them, or do it in voice."));
                }
                crate::buddy::act(app, s(input, "action"), input).await.into()
            }
            "mac_step" => {
                let action = s(input, "action");
                if job.is_some() && ctx.mode != Mode::Live && !matches!(action, "open") {
                    return Some(ToolOutput::err("Background jobs don't click around in their apps; start a live agent (start_job kind=live) for that."));
                }
                mac_step(app, action, input).await
            }
            "start_job" => match jobs::start(app, s(input, "title"), s(input, "instructions"), s(input, "kind")) {
                Ok(id) if s(input, "kind") == "live" => ToolOutput::ok(format!("Started live agent {id}. It will do it on their screen and explain as it goes; esc stops it. Say one short line, nothing more.")),
                Ok(id) => ToolOutput::ok(format!("Started job {id}. It's in the notch's Agents tab (hover the notch); tell them in a few words.")),
                Err(e) => ToolOutput::err(e),
            },
            "clipboard" => clipboard(s(input, "action"), s(input, "text")).await,
            "look" => {
                let full = input.get("full").and_then(Value::as_bool).unwrap_or(false);
                let width = if full { 4096 } else { crate::screen::IMAGE_WIDTH };
                // Only models that take images get the screenshot; the rest work from the lists.
                let effort = if ctx.mode == Mode::Live { openclicky_llm::Effort::Balanced } else { openclicky_llm::Effort::Fast };
                let vision = st.provider().is_some_and(|p| p.sees_images_at(effort));
                // Live agents stay on the screen of the app they work in, wherever the mouse goes.
                let want = match s(input, "display") {
                    "" if ctx.mode == Mode::Live => "window",
                    w => w,
                };
                // Another screen only when asked: one capture per screen named.
                let targets: Vec<(Option<(f64, f64)>, String)> = if want.trim().is_empty() {
                    vec![(None, String::new())]
                } else {
                    let ds = crate::screen::displays(app);
                    let window = st.last_capture.lock().unwrap().as_ref().and_then(|c| c.window_frame.clone());
                    match crate::screen::resolve(&ds, want, crate::macos::cursor(), window.as_ref()) {
                        Ok(v) if ds.len() > 1 => v.into_iter().map(|i| (crate::screen::center_of(&ds, i), crate::screen::name_of(&ds, i))).collect(),
                        Ok(_) => vec![(None, String::new())],
                        Err(e) => return Some(ToolOutput::err(e)),
                    }
                };
                let several = targets.len() > 1;
                let mut text = vec![];
                let mut images = vec![];
                let mut err = None;
                for (at, name) in &targets {
                    let cap = crate::screen::capture_on(app, 0, width, *at).await;
                    if let Some(e) = &cap.error {
                        err = Some(format!("Couldn't see the screen: {e}"));
                        continue;
                    }
                    let body = if full { cap.describe() } else { cap.describe_for("") };
                    text.push(if name.is_empty() { body } else { format!("SCREEN: {name}\n{body}") });
                    if let Some(b64) = cap.image_base64().filter(|_| vision) {
                        images.push(("image/jpeg".to_string(), b64));
                    }
                }
                if text.is_empty() {
                    ToolOutput::err(err.unwrap_or_else(|| "Couldn't see the screen.".into()))
                } else {
                    if several {
                        text.push(format!("(Pointing and clicking now use {}, the last screen above; ids are per screen.)", targets.last().map(|t| t.1.as_str()).unwrap_or("")));
                    }
                    let mut out = ToolOutput::ok(text.join("\n\n"));
                    out.images = images;
                    out
                }
            }
            "update_plan" => match job {
                Some(id) => jobs::set_plan(app, id, input).into(),
                None => ToolOutput::err("update_plan only works inside a job."),
            },
            "progress" => match job {
                Some(id) => jobs::set_progress(app, id, s(input, "note")).into(),
                None => ToolOutput::ok("ok"),
            },
            "ask_user" => match job {
                Some(id) => {
                    let choices = input
                        .get("choices")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                        .unwrap_or_default();
                    jobs::ask(app, id, s(input, "question"), choices).await.into()
                }
                None => ToolOutput::err("In voice, just ask them out loud."),
            },
            "browser" => browser::call(app, input).await.into(),
            other if other.starts_with("mcp__") => crate::connectors::call(app, other, input).await.into(),
            other if crate::integrations::handles(other) => crate::integrations::call(app, other, input).await.into(),
            other if crate::google::handles(other) => crate::google::call(app, other, input).await.into(),
            _ => return None,
        };
        Some(out)
    }

    fn note_changed(&self, ctx: &ToolCtx) {
        let src = std::fs::read_to_string(&ctx.note_path).unwrap_or_default();
        let doc = openclicky_core::note::parse(&src);
        match &ctx.job_id {
            Some(id) => {
                emit(&self.app, Event::with_job("job.note", id, json!({ "doc": doc })));
                self.app.state::<AppState>().play_quiet("pen-flick");
            }
            None => {
                emit(&self.app, Event::new("session.note", json!({ "doc": doc })));
            }
        }
    }

    fn extra_tools(&self, mode: Mode) -> Vec<ToolDef> {
        if matches!(mode, Mode::Job | Mode::Voice | Mode::Live) {
            let mut t = vec![crate::myself::tool()];
            if crate::memory::mode(&self.app) != "off" {
                t.push(crate::memory::tool());
            }
            t.extend(crate::connectors::tools(&self.app));
            t.extend(crate::integrations::tools(&self.app));
            t.extend(crate::google::tools(&self.app));
            t.push(crate::canvas::tool());
            if mode != Mode::Job && crate::scene3d::enabled(&self.app) {
                t.push(crate::scene3d::tool());
            }
            t
        } else {
            vec![crate::myself::tool()]
        }
    }
}
