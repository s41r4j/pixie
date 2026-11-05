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
    use tokio::io::AsyncWriteExt;
    match action {
        "copy" => {
            let (cmd, args): (&str, &[&str]) = if cfg!(windows) {
                ("powershell", &["-NoProfile", "-Command", "$input | Set-Clipboard"])
            } else {
                ("/usr/bin/pbcopy", &[])
            };
            let child = tokio::process::Command::new(cmd).args(args).stdin(std::process::Stdio::piped()).spawn();
            let Ok(mut child) = child else { return ToolOutput::err("Couldn't reach the clipboard.") };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            match child.wait().await {
                Ok(s) if s.success() => ToolOutput::ok(format!("Copied {} characters; they can paste with {}.", text.chars().count(), if cfg!(windows) { "Ctrl+V" } else { "⌘V" })),
                _ => ToolOutput::err("Couldn't copy."),
            }
        }
        "read" => match if cfg!(windows) {
            tokio::process::Command::new("powershell").args(["-NoProfile", "-Command", "Get-Clipboard -Raw"]).output().await
        } else {
            tokio::process::Command::new("/usr/bin/pbpaste").output().await
        } {
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
            "say" => {
                let text = s(input, "text");
                let id = format!("say-{}", rand::random::<u32>());
                emit(app, Event::new("speech.segment", json!({ "id": id, "text": text, "job": job })));
                if st.settings.lock().map(|s| s.speak_answers).unwrap_or(true) {
                    speech::speak(app, &id, text);
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
                match st.helper.call("mac_step", input.clone(), Duration::from_secs(10)).await {
                    Ok(v) => {
                        // Show where it happened.
                        if let Some(f) = v.get("frame") {
                            let _ = overlay::highlight(app, &json!({ "x": f["x"], "y": f["y"], "w": f["w"], "h": f["h"], "label": v.get("pressed").or(v.get("label")).cloned().unwrap_or(Value::Null) }));
                        }
                        ToolOutput::ok(v.to_string())
                    }
                    Err(e) => ToolOutput::err(e),
                }
            }
            "start_job" => match jobs::start(app, s(input, "title"), s(input, "instructions"), s(input, "kind")) {
                Ok(id) if s(input, "kind") == "live" => ToolOutput::ok(format!("Started live agent {id}. It will do it on their screen and explain as it goes; esc stops it. Say one short line, nothing more.")),
                Ok(id) => ToolOutput::ok(format!("Started job {id}. It's in the notch's Agents tab (hover the notch); tell them in a few words.")),
                Err(e) => ToolOutput::err(e),
            },
            "clipboard" => clipboard(s(input, "action"), s(input, "text")).await,
            "look" => {
                let cap = crate::screen::capture(app, 0).await;
                if let Some(e) = &cap.error {
                    ToolOutput::err(format!("Couldn't see the screen: {e}"))
                } else {
                    let mut out = ToolOutput::ok(cap.describe());
                    if let Some(b64) = cap.image_base64() {
                        out.images.push(("image/jpeg".into(), b64));
                    }
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
            let mut t = crate::connectors::tools(&self.app);
            t.extend(crate::integrations::tools(&self.app));
            t.extend(crate::google::tools(&self.app));
            t
        } else {
            vec![]
        }
    }
}
