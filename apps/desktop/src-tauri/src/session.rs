//! The voice loop (M2–M4): hold ⌃⌥ → mic + live transcript (and a screenshot in parallel) →
//! release → the model answers in a stream that is spoken sentence by sentence, while it
//! points at things, draws, writes on the panel's paper or hands work to a background job.
//! Hold fn+⌃ to dictate: the cleaned-up text is typed into the focused field.

use crate::host::AppHost;
use crate::panel::{self, PanelMode};
use crate::screen::{self, Capture};
use crate::speech::{self, SpeechStream};
use crate::{emit, overlay, AppState};
use openclicky_core::prompt::{self, Mode, PromptInput};
use openclicky_core::Event;
use openclicky_llm::{Content, Effort, Message, Request};
use openclicky_tools::{AgentEvent, AgentOptions, CancellationToken, ToolCtx};
use serde_json::json;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

/// Conversation memory resets after this long without a turn.
const HISTORY_TTL: Duration = Duration::from_secs(10 * 60);
const HISTORY_MAX: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listening {
    Talk,
    Dictate,
}

#[derive(Default)]
struct Inner {
    turn: u64,
    cancel: Option<CancellationToken>,
    history: Vec<Message>,
    last_turn: Option<Instant>,
    capture: Option<oneshot::Receiver<Capture>>,
    listening: Option<Listening>,
    speaking: HashSet<String>,
    answering: bool,
    /// Generation for the auto-collapse timer.
    collapse_gen: u64,
    hovered: bool,
}

#[derive(Default)]
pub struct Session {
    inner: Mutex<Inner>,
}

impl Session {
    pub fn speaking_started(&self, id: &str) {
        self.inner.lock().unwrap().speaking.insert(id.to_string());
    }
    pub fn speaking_cleared(&self) {
        self.inner.lock().unwrap().speaking.clear();
    }
    fn speaking_ended(&self, id: &str) -> bool {
        let mut i = self.inner.lock().unwrap();
        i.speaking.remove(id);
        i.speaking.is_empty()
    }
    pub fn is_speaking(&self) -> bool {
        !self.inner.lock().unwrap().speaking.is_empty()
    }
    fn busy(&self) -> bool {
        let i = self.inner.lock().unwrap();
        i.answering || !i.speaking.is_empty()
    }
    pub fn set_hovered(&self, h: bool) {
        self.inner.lock().unwrap().hovered = h;
    }
}

fn state(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}

/// Stop whatever is in flight (speech, model call, tools).
fn interrupt(app: &AppHandle) {
    let st = state(app);
    let cancel = {
        let mut i = st.session.inner.lock().unwrap();
        i.answering = false;
        i.cancel.take()
    };
    if let Some(c) = cancel {
        c.cancel();
    }
    if st.session.is_speaking() {
        speech::stop(app);
    }
}

fn begin_capture(app: &AppHandle, n: u64) -> oneshot::Receiver<Capture> {
    let (tx, rx) = oneshot::channel();
    let share = state(app).settings.lock().map(|s| s.share_screen).unwrap_or(true);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let c = if share { screen::capture(&app, n).await } else { Capture { error: Some("screen sharing is off in Settings".into()), ..Default::default() } };
        let _ = tx.send(c);
    });
    rx
}

fn start_listening(app: &AppHandle, kind: Listening) {
    interrupt(app);
    let st = state(app);
    let n = {
        let mut i = st.session.inner.lock().unwrap();
        i.turn += 1;
        i.listening = Some(kind);
        i.collapse_gen += 1;
        i.turn
    };
    if kind == Listening::Talk {
        let rx = begin_capture(app, n);
        st.session.inner.lock().unwrap().capture = Some(rx);
        overlay::clear(app);
    }
    panel::set_mode(app, if kind == Listening::Talk { PanelMode::Listening } else { PanelMode::Dictating });
    emit(app, Event::new("mic.start", json!({ "turn": n, "kind": if kind == Listening::Talk { "talk" } else { "dictate" } })));
    let locale = st.settings.lock().map(|s| s.locale.clone()).unwrap_or_default();
    let helper = st.helper.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = helper.call("listen_start", json!({ "locale": locale, "on_device": true }), Duration::from_secs(15)).await {
            log::warn!("listen_start: {e}");
            let still = app2.state::<AppState>().session.inner.lock().unwrap().turn == n;
            if still {
                show_error(&app2, &e, true);
            }
        }
    });
}

async fn stop_listening(app: &AppHandle) -> String {
    let helper = state(app).helper.clone();
    match helper.call("listen_stop", json!({}), Duration::from_secs(5)).await {
        Ok(v) => v["text"].as_str().unwrap_or("").trim().to_string(),
        Err(e) => {
            log::warn!("listen_stop: {e}");
            String::new()
        }
    }
}

pub fn talk_down(app: &AppHandle) {
    start_listening(app, Listening::Talk);
}

/// `tap`: released quickly — just open the panel to type, or close it if it was open.
pub fn talk_up(app: &AppHandle, tap: bool, was_open: bool) {
    let st = state(app);
    let (n, rx) = {
        let mut i = st.session.inner.lock().unwrap();
        i.listening = None;
        (i.turn, i.capture.take())
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let text = stop_listening(&app).await;
        if tap && text.is_empty() {
            if was_open {
                panel::hide(&app);
            } else {
                panel::set_mode(&app, PanelMode::Idle);
                schedule_collapse(&app, Duration::from_secs(6));
            }
            return;
        }
        if text.is_empty() {
            emit(&app, Event::new("turn.empty", json!({ "turn": n })));
            app.state::<AppState>().play("nope");
            panel::set_mode(&app, PanelMode::Answer);
            schedule_collapse(&app, Duration::from_millis(1800));
            return;
        }
        run_turn(app, n, text, rx).await;
    });
}

/// Typed question from the panel.
pub fn ask_text(app: &AppHandle, text: String) {
    interrupt(app);
    let st = state(app);
    let n = {
        let mut i = st.session.inner.lock().unwrap();
        i.turn += 1;
        i.collapse_gen += 1;
        i.turn
    };
    let rx = begin_capture(app, n);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { run_turn(app, n, text, Some(rx)).await });
}

pub fn escape(app: &AppHandle) {
    let listening = state(app).session.inner.lock().unwrap().listening.take();
    if listening.is_some() {
        state(app).helper.send("listen_cancel", json!({}));
    }
    interrupt(app);
    overlay::clear(app);
    panel::hide(app);
}

pub fn dictate_down(app: &AppHandle) {
    if state(app).session.inner.lock().unwrap().listening == Some(Listening::Talk) {
        return;
    }
    start_listening(app, Listening::Dictate);
}

pub fn dictate_up(app: &AppHandle) {
    {
        let st = state(app);
        let mut i = st.session.inner.lock().unwrap();
        if i.listening != Some(Listening::Dictate) {
            return;
        }
        i.listening = None;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let raw = stop_listening(&app).await;
        if raw.is_empty() {
            panel::hide(&app);
            return;
        }
        emit(&app, Event::new("dictation.raw", json!({ "text": raw })));
        let text = clean_dictation(&app, &raw).await.unwrap_or(raw);
        let st = app.state::<AppState>();
        match st.helper.call("insert_text", json!({ "text": text }), Duration::from_secs(5)).await {
            Ok(_) => {
                emit(&app, Event::new("dictation.done", json!({ "text": text })));
                st.play("type-keys");
                panel::set_mode(&app, PanelMode::Toast);
                schedule_collapse(&app, Duration::from_millis(1400));
            }
            Err(e) => show_error(&app, &e, false),
        }
    });
}

/// Fix punctuation and drop filler words; falls back to the raw text quickly.
async fn clean_dictation(app: &AppHandle, raw: &str) -> Option<String> {
    let provider = state(app).provider()?;
    if raw.split_whitespace().count() < 4 {
        return None;
    }
    let mut req = Request::new(
        "You clean up dictated text. Fix punctuation and capitalisation, remove filler words (um, uh, like, you know) and false starts, and apply spoken corrections (\"no, I mean…\"). Keep their words and meaning; don't add anything. Reply with the cleaned text only.",
        vec![Message::user_text(raw)],
    );
    req.max_tokens = 800;
    req.effort = Effort::Fast;
    let mut sink = |_| {};
    let fut = provider.complete(&req, &mut sink);
    match tokio::time::timeout(Duration::from_secs(4), fut).await {
        Ok(Ok(r)) => Some(r.text().trim().to_string()).filter(|t| !t.is_empty()),
        _ => None,
    }
}

pub fn show_error(app: &AppHandle, message: &str, from_mic: bool) {
    emit(app, Event::new("turn.error", json!({ "message": message, "mic": from_mic })));
    state(app).play("nope");
    panel::set_mode(app, PanelMode::Answer);
    schedule_collapse(app, Duration::from_secs(7));
}

fn environment(app: &AppHandle, cwd: &std::path::Path, capture: Option<&Capture>) -> prompt::Environment {
    let st = state(app);
    let s = st.settings.lock().unwrap().clone();
    prompt::Environment {
        cwd: cwd.display().to_string(),
        os: st.os_version.clone(),
        now: chrono::Local::now().format("%A %-d %B %Y, %H:%M").to_string(),
        user_name: if s.user_name.is_empty() { st.full_name.clone() } else { s.user_name },
        screenshot: capture.map(|c| c.path.is_some()).unwrap_or(false),
        app: capture.map(|c| c.app.clone()).unwrap_or_default(),
        window: capture.map(|c| c.window.clone()).unwrap_or_default(),
        connected_apps: st.connected_apps(),
        browser_connected: crate::browser::connected(app),
        skills: st.skills.clone(),
        memory: crate::jobs::memory(&st.data_dir),
        helper: crate::jobs::helper_hint(&st),
    }
}

/// One spoken turn.
async fn run_turn(app: AppHandle, n: u64, text: String, capture: Option<oneshot::Receiver<Capture>>) {
    let st = app.state::<AppState>();
    let cancel = CancellationToken::new();
    {
        let mut i = st.session.inner.lock().unwrap();
        if i.turn != n {
            return; // superseded
        }
        i.cancel = Some(cancel.clone());
        i.answering = true;
        if i.last_turn.map(|t| t.elapsed() > HISTORY_TTL).unwrap_or(false) {
            i.history.clear();
        }
        i.last_turn = Some(Instant::now());
    }
    panel::set_mode(&app, PanelMode::Thinking);
    emit(&app, Event::new("turn.start", json!({ "turn": n, "text": text })));

    let Some(provider) = st.provider() else {
        finish_turn(&app, n, false);
        show_error(&app, "I need a model key first. Open Settings → AI and paste a DeepSeek or Claude key.", false);
        return;
    };

    let cap = match capture {
        Some(rx) => tokio::time::timeout(Duration::from_secs(4), rx).await.ok().and_then(Result::ok),
        None => None,
    };
    let session_dir = st.data_dir.join("session");
    let _ = std::fs::create_dir_all(&session_dir);
    let note_path = session_dir.join("note.md");
    // A fresh page for every message.
    let _ = std::fs::write(&note_path, "");
    emit(&app, Event::new("session.note", json!({ "doc": null })));

    let mode = Mode::Voice;
    let system = prompt::assemble(&PromptInput {
        mode,
        env: environment(&app, &session_dir, cap.as_ref()),
        task: cap.as_ref().map(Capture::describe).unwrap_or_default(),
    });
    let mut content = vec![];
    if provider.supports_images() {
        if let Some(b64) = cap.as_ref().and_then(Capture::image_base64) {
            content.push(Content::jpeg_base64(b64));
        }
    }
    content.push(Content::text(text.clone()));
    let mut messages = st.session.inner.lock().unwrap().history.clone();
    messages.push(Message::user(content));

    let host = Arc::new(AppHost { app: app.clone() });
    let ctx = ToolCtx {
        mode,
        cwd: session_dir.clone(),
        note_path,
        job_id: None,
        host: host.clone(),
        cancel: cancel.clone(),
        helper: st.helper.path.clone(),
    };
    let mut tools = openclicky_tools::defs::for_mode(mode, crate::browser::connected(&app));
    tools.extend(openclicky_tools::Host::extra_tools(host.as_ref(), mode));
    let opts = AgentOptions { max_steps: 8, effort: Effort::Fast, max_tokens: 1500 };

    let mut voice = SpeechStream::new(&app, n);
    let app_ev = app.clone();
    let mut first = true;
    let result = openclicky_tools::run(provider.as_ref(), &system, messages, tools, &ctx, &opts, &mut |e| match e {
        AgentEvent::Text(d) => {
            if first {
                first = false;
                panel::set_mode(&app_ev, PanelMode::Answer);
            }
            emit(&app_ev, Event::new("turn.delta", json!({ "turn": n, "text": d })));
            voice.push(&d);
        }
        AgentEvent::ToolStart { name, input } => {
            emit(&app_ev, Event::new("turn.tool", json!({ "turn": n, "name": name, "input": input })));
        }
        AgentEvent::ToolEnd { name, ok, summary } => {
            emit(&app_ev, Event::new("turn.tool_done", json!({ "turn": n, "name": name, "ok": ok, "summary": summary })));
        }
        AgentEvent::Step { .. } => voice.flush(),
    })
    .await;
    voice.flush();

    match result {
        Ok(out) => {
            {
                let mut i = st.session.inner.lock().unwrap();
                i.history.push(Message::user_text(text));
                let reply = if out.final_text.trim().is_empty() { "(pointed it out on screen)".to_string() } else { out.final_text.clone() };
                i.history.push(Message::assistant(vec![Content::text(reply)]));
                let excess = i.history.len().saturating_sub(HISTORY_MAX);
                i.history.drain(..excess);
            }
            emit(&app, Event::new("turn.done", json!({ "turn": n, "text": out.final_text })));
            panel::set_mode(&app, PanelMode::Answer);
            finish_turn(&app, n, voice.spoke_anything());
        }
        Err(openclicky_llm::LlmError::Cancelled) => finish_turn(&app, n, false),
        Err(e) => {
            finish_turn(&app, n, false);
            show_error(&app, &e.to_string(), false);
        }
    }
}

fn finish_turn(app: &AppHandle, n: u64, spoke: bool) {
    let st = state(app);
    {
        let mut i = st.session.inner.lock().unwrap();
        if i.turn == n {
            i.answering = false;
            i.cancel = None;
        }
    }
    if !spoke && !st.session.is_speaking() {
        schedule_collapse(app, Duration::from_secs(8));
    }
}

/// Collapse the panel after `after`, unless something new happened or it's hovered.
pub fn schedule_collapse(app: &AppHandle, after: Duration) {
    let st = state(app);
    let gen = {
        let mut i = st.session.inner.lock().unwrap();
        i.collapse_gen += 1;
        i.collapse_gen
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(after).await;
        let st = app.state::<AppState>();
        let (same, hovered, busy, listening) = {
            let i = st.session.inner.lock().unwrap();
            (i.collapse_gen == gen, i.hovered, i.answering || !i.speaking.is_empty(), i.listening.is_some())
        };
        if !same || listening || busy {
            return;
        }
        if hovered {
            schedule_collapse(&app, Duration::from_secs(3));
            return;
        }
        if !matches!(panel::mode(&app), PanelMode::Typing) {
            panel::hide(&app);
        }
    });
}

/// A queued sentence that won't be spoken (synthesis failed or was superseded).
pub fn speech_failed(app: &AppHandle, id: &str) {
    let st = app.state::<AppState>();
    let all_done = st.session.speaking_ended(id);
    emit(app, Event::new("speech.end", json!({ "id": id, "cancelled": true })));
    if all_done && !st.session.busy() {
        schedule_collapse(app, Duration::from_secs(7));
    }
}

/// Route helper events (mic level, partial transcript, speech progress) to the UI.
pub fn spawn_event_pump(app: AppHandle) {
    let mut rx = state(&app).helper.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            let (name, data) = match rx.recv().await {
                Ok(v) => v,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            };
            match name.as_str() {
                "level" => emit(&app, Event::new("mic.level", data)),
                "partial" => emit(&app, Event::new("mic.partial", data)),
                "speak_start" => emit(&app, Event::new("speech.start", data)),
                "speak_word" => emit(&app, Event::new("speech.word", data)),
                "speak_end" => {
                    let id = data["id"].as_str().unwrap_or("").to_string();
                    let st = app.state::<AppState>();
                    let all_done = st.session.speaking_ended(&id);
                    emit(&app, Event::new("speech.end", data));
                    if all_done && !st.session.busy() {
                        schedule_collapse(&app, Duration::from_secs(7));
                    }
                }
                "restarted" => {
                    let st = app.state::<AppState>();
                    st.session.speaking_cleared();
                }
                _ => {}
            }
        }
    });
}
