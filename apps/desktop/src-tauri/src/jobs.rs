//! Background jobs (M5): a workspace per job, the tool loop, plan/progress events, the note,
//! asking the user, and the final report. Jobs survive restarts as `state.json`.
//!
//! ```text
//! <app_data>/jobs/<id>/
//!   note.md  output/  tmp/  inbox/  AGENTS.md  state.json
//! ```

use crate::host::AppHost;
use crate::{emit, panel, session, AppState};
use openclicky_core::model::{JobState, PlanStep, StepStatus};
use openclicky_core::prompt::{self, Mode, PromptInput};
use openclicky_core::report::{self, Report};
use openclicky_core::Event;
use openclicky_llm::{Content, Effort, LlmError, Message};
use openclicky_tools::{AgentEvent, AgentOptions, CancellationToken, ToolCtx};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub choices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobView {
    pub id: String,
    pub title: String,
    pub state: JobState,
    pub instructions: String,
    pub cwd: PathBuf,
    pub created_at: i64,
    pub updated_at: i64,
    pub plan: Vec<PlanStep>,
    pub progress: String,
    pub question: Option<Question>,
    pub report: Option<Report>,
    pub error: Option<String>,
    pub unread: bool,
    pub steps: usize,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    view: JobView,
    messages: Vec<Message>,
}

struct Entry {
    view: JobView,
    messages: Vec<Message>,
    cancel: CancellationToken,
    answer: Option<oneshot::Sender<String>>,
}

#[derive(Default)]
pub struct Jobs {
    map: Mutex<HashMap<String, Entry>>,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn jobs_dir(data: &Path) -> PathBuf {
    data.join("jobs")
}

/// Durable facts from `LEARNED` blocks.
pub fn memory(data: &Path) -> Vec<String> {
    std::fs::read_to_string(data.join("memory.md"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- ").map(str::to_string))
        .rev()
        .take(30)
        .collect()
}

fn remember(data: &Path, facts: &[String]) {
    if facts.is_empty() {
        return;
    }
    let mut cur = std::fs::read_to_string(data.join("memory.md")).unwrap_or_else(|_| "# What openclicky knows about you\n\n".into());
    let date = chrono::Local::now().format("%Y-%m-%d");
    for f in facts {
        if !cur.contains(f.as_str()) {
            cur.push_str(&format!("- {f} ({date})\n"));
        }
    }
    let _ = std::fs::write(data.join("memory.md"), cur);
}

pub fn helper_hint(st: &AppState) -> String {
    if st.helper.path.is_some() {
        "`openclicky-helper pdf in.html out.pdf [letter|a4]` turns an HTML file into a paginated PDF at real paper size (CSS px = PDF points; design for 612 px wide on Letter, 595 on A4, use your own padding); `openclicky-helper text file.pdf` extracts text from PDF/Word/RTF. python3 and the usual macOS command-line tools are available.".into()
    } else {
        "python3 and the usual macOS command-line tools are available.".into()
    }
}

/// Images are dropped from saved history (they're large and the files are on disk).
fn slim(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| Message {
            role: m.role,
            content: m
                .content
                .iter()
                .map(|c| match c {
                    Content::Image { .. } => Content::text("[image]"),
                    Content::ToolResult { tool_use_id, content, is_error } => Content::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.iter().map(|c| if matches!(c, Content::Image { .. }) { Content::text("[image]") } else { c.clone() }).collect(),
                        is_error: *is_error,
                    },
                    other => other.clone(),
                })
                .collect(),
        })
        .collect()
}

impl Jobs {
    pub fn list(&self) -> Vec<JobView> {
        let mut v: Vec<JobView> = self.map.lock().unwrap().values().map(|e| e.view.clone()).collect();
        v.sort_by_key(|j| std::cmp::Reverse(j.created_at));
        v
    }

    pub fn get(&self, id: &str) -> Option<JobView> {
        self.map.lock().unwrap().get(id).map(|e| e.view.clone())
    }

    fn update<F: FnOnce(&mut Entry)>(&self, app: &AppHandle, id: &str, f: F) -> Option<JobView> {
        let view = {
            let mut m = self.map.lock().unwrap();
            let e = m.get_mut(id)?;
            f(e);
            e.view.updated_at = now_ms();
            let saved = Saved { view: e.view.clone(), messages: slim(&e.messages) };
            if let Ok(s) = serde_json::to_string(&saved) {
                let _ = std::fs::write(e.view.cwd.join("state.json"), s);
            }
            e.view.clone()
        };
        emit(app, Event::with_job("job.updated", id, json!({ "job": view })));
        crate::shelf::refresh(app);
        Some(view)
    }

    /// Load jobs from disk (most recent 30). Jobs that were running are marked stopped.
    pub fn load(&self, data: &Path) {
        let dir = jobs_dir(data);
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        let mut loaded: Vec<Saved> = rd
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path().join("state.json")).ok())
            .filter_map(|s| serde_json::from_str::<Saved>(&s).ok())
            .collect();
        loaded.sort_by_key(|s| std::cmp::Reverse(s.view.created_at));
        let mut m = self.map.lock().unwrap();
        for mut s in loaded.into_iter().take(30) {
            if matches!(s.view.state, JobState::Running | JobState::WaitingOnUser | JobState::Offered) {
                s.view.state = JobState::Failed;
                s.view.error = Some("openclicky was closed while this ran. Send a message to pick it up again.".into());
                s.view.question = None;
            }
            m.insert(s.view.id.clone(), Entry { view: s.view, messages: s.messages, cancel: CancellationToken::new(), answer: None });
        }
    }
}

fn new_id() -> String {
    let t = chrono::Local::now().format("%Y%m%d-%H%M%S");
    format!("{t}-{:04x}", rand::random::<u16>())
}

/// Start a job; returns its id. The note is never blank: it starts with the title.
pub fn start(app: &AppHandle, title: &str, instructions: &str) -> Result<String, String> {
    let st = app.state::<AppState>();
    let id = new_id();
    let cwd = jobs_dir(&st.data_dir).join(&id);
    for sub in ["output", "tmp", "inbox"] {
        std::fs::create_dir_all(cwd.join(sub)).map_err(|e| e.to_string())?;
    }
    let title = if title.trim().is_empty() { "New job".to_string() } else { title.trim().to_string() };
    let _ = std::fs::write(cwd.join("note.md"), format!("# {title}\n\n## Working on it\n- [ ] Read the brief {{now}}\n"));
    let _ = std::fs::write(
        cwd.join("AGENTS.md"),
        format!("# {title}\n\nStarted {}.\n\n## Brief\n{instructions}\n\n## Notes\n", chrono::Local::now().format("%Y-%m-%d %H:%M")),
    );
    let view = JobView {
        id: id.clone(),
        title,
        state: JobState::Running,
        instructions: instructions.to_string(),
        cwd: cwd.clone(),
        created_at: now_ms(),
        updated_at: now_ms(),
        plan: vec![],
        progress: "Getting started".into(),
        question: None,
        report: None,
        error: None,
        unread: true,
        steps: 0,
    };
    st.jobs.map.lock().unwrap().insert(
        id.clone(),
        Entry { view: view.clone(), messages: vec![Message::user_text(instructions)], cancel: CancellationToken::new(), answer: None },
    );
    st.play("paper-place");
    st.jobs.update(app, &id, |_| {});
    drive(app.clone(), id.clone());
    Ok(id)
}

/// A follow-up from the note window or a tapped `::ask` button.
pub fn message(app: &AppHandle, id: &str, text: &str) -> Result<(), String> {
    let st = app.state::<AppState>();
    let running = st.jobs.get(id).map(|j| matches!(j.state, JobState::Running)).ok_or("No such job.")?;
    if running {
        return Err("It's still working; you can answer when it asks, or stop it first.".into());
    }
    st.jobs
        .update(app, id, |e| {
            e.messages.push(Message::user_text(text));
            e.view.state = JobState::Running;
            e.view.error = None;
            e.view.report = None;
            e.view.progress = "Picking it up".into();
            e.cancel = CancellationToken::new();
        })
        .ok_or("No such job.")?;
    drive(app.clone(), id.to_string());
    Ok(())
}

pub fn answer(app: &AppHandle, id: &str, text: &str) -> Result<(), String> {
    let st = app.state::<AppState>();
    let tx = st.jobs.map.lock().unwrap().get_mut(id).and_then(|e| e.answer.take());
    match tx {
        Some(tx) => {
            let _ = tx.send(text.to_string());
            st.jobs.update(app, id, |e| {
                e.view.question = None;
                e.view.state = JobState::Running;
            });
            Ok(())
        }
        None => message(app, id, text),
    }
}

pub fn cancel(app: &AppHandle, id: &str) {
    let st = app.state::<AppState>();
    let mut m = st.jobs.map.lock().unwrap();
    if let Some(e) = m.get_mut(id) {
        e.cancel.cancel();
        e.answer.take();
    }
}

pub fn dismiss(app: &AppHandle, id: &str) {
    let st = app.state::<AppState>();
    st.jobs.update(app, id, |e| e.view.unread = false);
}

pub fn delete(app: &AppHandle, id: &str) {
    cancel(app, id);
    let st = app.state::<AppState>();
    if let Some(e) = st.jobs.map.lock().unwrap().remove(id) {
        let _ = std::fs::remove_dir_all(&e.view.cwd);
    }
    emit(app, Event::with_job("job.removed", id, json!({})));
    crate::shelf::refresh(app);
}

// ---- host-side tools -------------------------------------------------------------------

pub fn set_plan(app: &AppHandle, id: &str, input: &Value) -> Result<String, String> {
    let items = input.get("plan").and_then(Value::as_array).ok_or("plan must be a list of steps.")?;
    let plan: Vec<PlanStep> = items
        .iter()
        .take(5)
        .map(|s| PlanStep {
            label: s["label"].as_str().unwrap_or("").to_string(),
            status: match s["status"].as_str().unwrap_or("pending") {
                "done" | "completed" => StepStatus::Done,
                "in_progress" | "active" => StepStatus::InProgress,
                _ => StepStatus::Pending,
            },
        })
        .collect();
    if plan.is_empty() {
        return Err("The plan needs at least one step.".into());
    }
    let n_active = plan.iter().filter(|s| s.status == StepStatus::InProgress).count();
    let st = app.state::<AppState>();
    st.jobs.update(app, id, |e| e.view.plan = plan.clone());
    if plan.iter().any(|s| s.status == StepStatus::Done) {
        st.play("badge-pop");
    }
    Ok(if n_active > 1 { "Plan saved (tip: keep exactly one step in_progress).".into() } else { "Plan saved.".into() })
}

pub fn set_progress(app: &AppHandle, id: &str, note: &str) -> Result<String, String> {
    app.state::<AppState>().jobs.update(app, id, |e| e.view.progress = note.to_string());
    Ok("Shown.".into())
}

/// Blocks the tool call until they answer (or the job is stopped).
pub async fn ask(app: &AppHandle, id: &str, question: &str, choices: Vec<String>) -> Result<String, String> {
    let st = app.state::<AppState>();
    let (tx, rx) = oneshot::channel();
    let cancel = {
        let mut m = st.jobs.map.lock().unwrap();
        let e = m.get_mut(id).ok_or("No such job.")?;
        e.answer = Some(tx);
        e.cancel.clone()
    };
    let view = st.jobs.update(app, id, |e| {
        e.view.state = JobState::WaitingOnUser;
        e.view.question = Some(Question { question: question.to_string(), choices: choices.clone() });
        e.view.unread = true;
    });
    st.play("agent-needs-you");
    if let Some(v) = view {
        toast(app, &format!("{} needs you", v.title), "needs", Some(id));
    }
    tokio::select! {
        _ = cancel.cancelled() => Err("Stopped.".into()),
        r = rx => r.map(|a| format!("They answered: {a}")).map_err(|_| "No answer.".to_string()),
    }
}

pub fn toast(app: &AppHandle, text: &str, kind: &str, job: Option<&str>) {
    emit(app, Event::new("toast", json!({ "text": text, "kind": kind, "job": job })));
    if panel::mode(app) == panel::PanelMode::Hidden {
        panel::set_mode(app, panel::PanelMode::Toast);
        session::schedule_collapse(app, Duration::from_millis(3600));
    }
}

fn friendly_progress(name: &str, input: &Value) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("").chars().take(48).collect::<String>();
    Some(match name {
        "web_search" => format!("Searching “{}”", s("query")),
        "fetch_url" => {
            let u = s("url");
            let host = u.split('/').nth(2).unwrap_or(&u).to_string();
            format!("Reading {host}")
        }
        "bash" => "Running a command".into(),
        "write" => format!("Writing {}", s("path").rsplit('/').next().unwrap_or("")),
        "device_find_files" => format!("Looking for “{}”", s("query")),
        "device_read_file" => format!("Reading {}", s("path").rsplit('/').next().unwrap_or("")),
        "device_save_file" => "Saving to your Mac".into(),
        "browser" => format!("In your browser: {}", s("operation")),
        _ => return None,
    })
}

/// Run (or continue) the job's loop in the background.
fn drive(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        let st = app.state::<AppState>();
        let Some(provider) = st.provider() else {
            st.jobs.update(&app, &id, |e| {
                e.view.state = JobState::Failed;
                e.view.error = Some("No model key yet. Add one in Settings → AI, then send a message to retry.".into());
            });
            return;
        };
        let Some((view, messages, cancel)) = st.jobs.map.lock().unwrap().get(&id).map(|e| (e.view.clone(), e.messages.clone(), e.cancel.clone())) else {
            return;
        };
        let host = Arc::new(AppHost { app: app.clone() });
        let ctx = ToolCtx {
            mode: Mode::Job,
            cwd: view.cwd.clone(),
            note_path: view.cwd.join("note.md"),
            job_id: Some(id.clone()),
            host: host.clone(),
            cancel: cancel.clone(),
            helper: st.helper.path.clone(),
        };
        let env = prompt::Environment {
            cwd: view.cwd.display().to_string(),
            os: st.os_version.clone(),
            now: chrono::Local::now().format("%A %-d %B %Y, %H:%M").to_string(),
            user_name: st.settings.lock().map(|s| s.user_name.clone()).ok().filter(|n| !n.is_empty()).unwrap_or_else(|| st.full_name.clone()),
            screenshot: false,
            app: String::new(),
            window: String::new(),
            connected_apps: st.connected_apps(),
            browser_connected: crate::browser::connected(&app),
            skills: st.skills.clone(),
            memory: memory(&st.data_dir),
            helper: helper_hint(&st),
        };
        let task = format!("Title: {}\n\n{}", view.title, view.instructions);
        let system = prompt::assemble(&PromptInput { mode: Mode::Job, env, task });
        let mut tools = openclicky_tools::defs::for_mode(Mode::Job, crate::browser::connected(&app));
        tools.extend(openclicky_tools::Host::extra_tools(host.as_ref(), Mode::Job));
        let opts = AgentOptions { max_steps: 60, effort: Effort::Deep, max_tokens: 16_000 };

        let app_ev = app.clone();
        let id_ev = id.clone();
        let result = openclicky_tools::run(provider.as_ref(), &system, messages, tools, &ctx, &opts, &mut |e| match e {
            AgentEvent::ToolStart { name, input } => {
                if let Some(p) = friendly_progress(&name, &input) {
                    let st = app_ev.state::<AppState>();
                    st.jobs.update(&app_ev, &id_ev, |e| e.view.progress = p);
                }
            }
            AgentEvent::Step { n } => {
                let st = app_ev.state::<AppState>();
                st.jobs.update(&app_ev, &id_ev, |e| e.view.steps = n);
            }
            _ => {}
        })
        .await;

        match result {
            Ok(out) => {
                let rep = report::parse(&out.final_text);
                remember(&st.data_dir, &rep.learned);
                let title = rep.done_title.clone().unwrap_or_else(|| view.title.clone());
                let v = st.jobs.update(&app, &id, |e| {
                    e.messages = out.messages.clone();
                    e.view.state = JobState::Done;
                    e.view.title = title.clone();
                    e.view.progress = String::new();
                    for s in e.view.plan.iter_mut() {
                        s.status = StepStatus::Done;
                    }
                    e.view.report = Some(rep.clone());
                    e.view.unread = true;
                });
                st.play("agent-done");
                if v.is_some() {
                    toast(&app, &format!("{title} is ready"), "done", Some(&id));
                }
            }
            Err(LlmError::Cancelled) => {
                st.jobs.update(&app, &id, |e| {
                    e.view.state = JobState::Failed;
                    e.view.error = Some("Stopped.".into());
                    e.view.question = None;
                });
            }
            Err(err) => {
                st.play("nope");
                st.jobs.update(&app, &id, |e| {
                    e.view.state = JobState::Failed;
                    e.view.error = Some(err.to_string());
                });
                toast(&app, &format!("{} hit a problem", view.title), "error", Some(&id));
            }
        }
    });
}

/// Parsed note for the UI.
pub fn note_doc(app: &AppHandle, id: &str) -> Option<Value> {
    let st = app.state::<AppState>();
    let cwd = st.jobs.get(id)?.cwd;
    let src = std::fs::read_to_string(cwd.join("note.md")).unwrap_or_default();
    serde_json::to_value(openclicky_core::note::parse(&src)).ok()
}
