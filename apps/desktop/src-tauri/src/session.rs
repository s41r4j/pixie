//! The voice loop (M2–M4): hold ⌃⌥ → mic + live transcript (and a screenshot in parallel) →
//! release → the model answers in a stream that is spoken sentence by sentence, while it
//! points at things, draws, writes on the panel's paper or hands work to a background job.
//! Hold fn+⌃ to dictate: the cleaned-up text is typed into the focused field.

use pixie_core::sync::LockOk;
use crate::host::AppHost;
use crate::panel::{self, PanelMode};
use crate::screen::{self, Capture};
use crate::speech::{self, SpeechStream};
use crate::{emit, overlay, AppState};
use pixie_core::prompt::{self, Mode, PromptInput};
use pixie_core::Event;
use pixie_llm::{Content, Effort, Message, Request};
use pixie_tools::{AgentEvent, AgentOptions, CancellationToken, ToolCtx};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

/// Turns sent to the model in full; older ones go as a short summary.
const KEEP_TURNS: usize = 12;
const HISTORY_MAX: usize = KEEP_TURNS * 2;
/// A gap this long gets a time note, so an old "this" isn't mixed up with the screen now.
const GAP_NOTE: Duration = Duration::from_secs(10 * 60);

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
    /// What the Chat tab shows (questions and answers, newest last).
    log: Vec<ChatEntry>,
    /// The saved conversation the log belongs to.
    chat_id: Option<String>,
    /// The chat day the conversation belongs to (see `chats::day_of`).
    day: String,
    /// Summary / memory bookkeeping saved with the chat.
    meta: crate::chats::Meta,
    /// They pressed New chat: don't carry on with today's earlier chat.
    fresh: bool,
}

/// Drop a "(Tools used for this answer: …)" note a model copied from older history.
pub fn strip_tool_note(s: &str) -> String {
    let mut s = match s.find("(Tools used for this answer") {
        Some(i) => {
            let end = s[i..].find(')').map(|j| i + j + 1).unwrap_or(s.len());
            format!("{}{}", &s[..i], &s[end..])
        }
        None => s.to_string(),
    };
    // Its second half, when speech split it at the full stop.
    for tail in ["A new request needs its own calls.)", "A new request needs its own calls."] {
        s = s.replace(tail, "");
    }
    s.trim().to_string()
}

/// The provider answering now ("deepseek", "claude", "openai", "opencode").
fn active_provider(app: &AppHandle) -> String {
    let st = state(app);
    let s = st.settings.lock().map(|s| s.clone()).unwrap_or_default();
    if s.provider == "opencode" {
        return "opencode".into();
    }
    crate::config::provider_config(&st.data_dir, &s).map(|c| c.kind.id().to_string()).unwrap_or_default()
}

/// Has this turn already been retried after a model fix? (Only once, never a loop.)
fn retried(app: &AppHandle, n: u64) -> bool {
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let _ = app;
    LAST.swap(n, std::sync::atomic::Ordering::SeqCst) == n
}

/// Ask the same thing again (after a model fix).
fn run_again(app: &AppHandle, text: String, typed: bool) {
    let st = state(app);
    let n = {
        let mut i = st.session.inner.lock_ok();
        i.turn += 1;
        i.collapse_gen += 1;
        i.turn
    };
    // Mark the new turn as a retry so a second failure is shown, not retried.
    let _ = retried(app, n);
    let rx = begin_capture(app, n);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { run_turn(app, n, text, Some(rx), typed).await });
}

/// "Start a background job: …", "run this as a job", "start a live agent to …": an explicit
/// request for a job, with a short title from its first words.
fn job_request(text: &str) -> Option<(String, &'static str)> {
    let l = text.to_lowercase();
    let live = l.contains("live agent");
    let asked = live || ["background job", "start a job", "start job", "as a job", "run a job", "new job", "start an agent", "background agent"].iter().any(|k| l.contains(k));
    if !asked || l.starts_with("what") || l.starts_with("how") || l.contains("?") && !l.contains(':') {
        return None;
    }
    let body = text.split_once(':').map(|(_, b)| b).unwrap_or(text);
    let title: Vec<String> = body
        .split_whitespace()
        .filter(|w| !matches!(w.to_lowercase().as_str(), "a" | "an" | "the" | "find" | "please" | "start" | "job" | "background" | "to" | "and"))
        .take(4)
        .map(|w| {
            let w = w.trim_matches(|c: char| !c.is_alphanumeric());
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .filter(|w| !w.is_empty())
        .collect();
    Some((if title.is_empty() { "Job".into() } else { title.join(" ") }, if live { "live" } else { "background" }))
}

/// The last turns of a log as model messages (text only).
fn rebuild(log: &[ChatEntry]) -> Vec<Message> {
    log.iter()
        .rev()
        .take(KEEP_TURNS)
        .rev()
        .flat_map(ChatEntry::messages)
        .collect()
}

impl Inner {
    fn reset_chat(&mut self) {
        self.history.clear();
        self.log.clear();
        self.chat_id = None;
        self.meta = Default::default();
    }

    /// Today's chat: a new day starts a new one; otherwise carry on with today's latest chat
    /// (after a relaunch), unless they asked for a new one. True when it loaded one.
    fn ensure_today(&mut self, data: &std::path::Path, reset_hour: u32) -> bool {
        let today = crate::chats::today(reset_hour);
        if !self.day.is_empty() && self.day != today {
            self.reset_chat();
            self.fresh = false;
        }
        self.day = today;
        if self.chat_id.is_some() || self.fresh || !self.log.is_empty() {
            return false;
        }
        match crate::chats::latest_today(data, reset_hour) {
            Some(c) => {
                self.history = rebuild(&c.entries);
                self.chat_id = Some(c.id);
                self.log = c.entries;
                self.meta = c.meta;
                true
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatEntry {
    pub q: String,
    pub a: String,
    pub at: i64,
    /// Tools the answer used (kept so a repeated request isn't read as answered in words only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
}

impl ChatEntry {
    /// The entry as model messages. Live history and history rebuilt after a restart both come
    /// from here, so they're byte-identical and the provider's prompt cache keeps matching.
    /// (The tools an answer used aren't written into it: models copied that note into their
    /// replies. The rules and the job guard cover repeated requests instead.)
    fn messages(&self) -> [Message; 2] {
        [Message::user_text(self.q.clone()), Message::assistant(vec![Content::text(strip_tool_note(&self.a))])]
    }
}

/// A day's worth of turns.
const LOG_MAX: usize = 500;

#[derive(Default)]
pub struct Session {
    inner: Mutex<Inner>,
}

impl Session {
    pub fn speaking_started(&self, id: &str) {
        self.inner.lock_ok().speaking.insert(id.to_string());
    }
    pub fn speaking_cleared(&self) {
        self.inner.lock_ok().speaking.clear();
    }
    fn speaking_ended(&self, id: &str) -> bool {
        let mut i = self.inner.lock_ok();
        i.speaking.remove(id);
        i.speaking.is_empty()
    }
    pub fn is_speaking(&self) -> bool {
        !self.inner.lock_ok().speaking.is_empty()
    }
    pub fn busy(&self) -> bool {
        let i = self.inner.lock_ok();
        i.answering || !i.speaking.is_empty()
    }
    pub fn hovered(&self) -> bool {
        self.inner.lock_ok().hovered
    }
    pub fn set_hovered(&self, h: bool) {
        self.inner.lock_ok().hovered = h;
    }
    /// Listening, thinking, answering or speaking.
    pub fn active(&self) -> bool {
        let i = self.inner.lock_ok();
        i.listening.is_some() || i.answering || !i.speaking.is_empty()
    }
    /// Today's conversation so far (carrying on with today's saved chat after a relaunch).
    pub fn chat_log(&self, data: &std::path::Path, reset_hour: u32) -> Vec<ChatEntry> {
        let mut i = self.inner.lock_ok();
        i.ensure_today(data, reset_hour);
        i.log.clone()
    }
    /// Start a new conversation: forget the context and the log (the old chat stays saved).
    pub fn chat_clear(&self) {
        let mut i = self.inner.lock_ok();
        i.reset_chat();
        i.fresh = true;
    }
    /// Memory has read a chat up to `n` entries.
    pub fn set_learned(&self, data: &std::path::Path, id: &str, n: usize) {
        let mut i = self.inner.lock_ok();
        if i.chat_id.as_deref() == Some(id) {
            i.meta.learned = i.meta.learned.max(n);
            crate::chats::save(data, id, &i.log, &i.meta);
        } else if let Some(mut c) = crate::chats::get(data, id) {
            c.meta.learned = c.meta.learned.max(n);
            crate::chats::save(data, id, &c.entries, &c.meta);
        }
    }
}

fn state(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}

/// Stop whatever is in flight (speech, model call, tools).
fn interrupt(app: &AppHandle) {
    let st = state(app);
    let cancel = {
        let mut i = st.session.inner.lock_ok();
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
    // Control ids start again from e1 for this request.
    screen::begin_turn(n);
    tauri::async_runtime::spawn(async move {
        let c = if share { screen::capture(&app, n).await } else { Capture { error: Some("screen sharing is off in Settings".into()), ..Default::default() } };
        let _ = tx.send(c);
    });
    rx
}

fn start_listening(app: &AppHandle, kind: Listening) {
    let t0 = std::time::Instant::now();
    interrupt(app);
    let st = state(app);
    let n = {
        let mut i = st.session.inner.lock_ok();
        i.turn += 1;
        i.listening = Some(kind);
        i.collapse_gen += 1;
        i.turn
    };
    if kind == Listening::Talk {
        // Clear marks first so they aren't in the screenshot's way (they're excluded anyway).
        overlay::clear(app);
        let rx = begin_capture(app, n);
        st.session.inner.lock_ok().capture = Some(rx);
    }
    panel::set_mode(app, if kind == Listening::Talk { PanelMode::Listening } else { PanelMode::Dictating });
    emit(app, Event::new("mic.start", json!({ "turn": n, "kind": if kind == Listening::Talk { "talk" } else { "dictate" } })));
    // Pixie's own engine types as you speak unless that's switched off (Settings → Voice). With
    // the system's recogniser it's still in Settings → Experimental (off: the text goes in when
    // you stop).
    let own = crate::stt::wanted(app, if kind == Listening::Talk { crate::stt::For::Talk } else { crate::stt::For::Dictate });
    let (locale, live) = st.settings.lock().map(|s| (s.locale.clone(), if own { s.dictate_live } else { s.experiment(LIVE_TYPING) })).unwrap_or_default();
    let live = live && kind == Listening::Dictate;
    let helper = st.helper.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let args = json!({ "locale": locale, "on_device": true, "live_type": live });
        let started = if own { crate::stt::begin(&app2, live).await } else { helper.call("listen_start", args, Duration::from_secs(15)).await };
        if started.is_ok() {
            // Key press to microphone on (the speed pass measures this).
            log::info!("listening: mic on {} ms after the press was seen (helper {})", t0.elapsed().as_millis(), started.as_ref().map(|v| v["ms"].to_string()).unwrap_or_default());
        }
        if let Ok(v) = &started {
            if live {
                log::info!("dictation: type as you speak {}", if v["live"].as_bool() == Some(true) { format!("on, by {} in {}", v["live_how"].as_str().unwrap_or(""), v["live_app"].as_str().unwrap_or("")) } else { format!("off ({})", v["live_reason"].as_str().unwrap_or("")) });
            }
        }
        if let Err(e) = started {
            log::warn!("listen_start: {e}");
            let still = app2.state::<AppState>().session.inner.lock_ok().turn == n;
            if still {
                show_error(&app2, &e, true);
            }
        }
    });
}

async fn stop_listening(app: &AppHandle) -> String {
    stop_listening_live(app).await.0
}

/// Settings → Experimental flag for type as you speak.
pub const LIVE_TYPING: &str = "type_live";

/// Whether the last dictation had no text cursor to type into (set from listen_stop).
static NO_SURFACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The transcript, plus what type as you speak left in the field (`live.active`, `live.typed`)
/// when it was on.
async fn stop_listening_live(app: &AppHandle) -> (String, Option<Value>) {
    let helper = state(app).helper.clone();
    // Pixie's own engine, when this recording was started with it.
    let stopped = match crate::stt::finish(app).await {
        Some(v) => Ok(v),
        None => helper.call("listen_stop", json!({}), Duration::from_secs(5)).await,
    };
    match stopped {
        Ok(v) => {
            NO_SURFACE.store(v["no_surface"].as_bool().unwrap_or(false), std::sync::atomic::Ordering::SeqCst);
            (v["text"].as_str().unwrap_or("").trim().to_string(), v.get("live").filter(|l| l.is_object()).cloned())
        }
        Err(e) => {
            log::warn!("listen_stop: {e}");
            (String::new(), None)
        }
    }
}

pub fn talk_down(app: &AppHandle) {
    log::info!("listening: talk keys seen");
    start_listening(app, Listening::Talk);
}

/// `tap`: released quickly: just open the panel to type, or close it if it was open.
pub fn talk_up(app: &AppHandle, tap: bool, was_open: bool) {
    let st = state(app);
    let (n, rx) = {
        let mut i = st.session.inner.lock_ok();
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
        run_turn(app, n, text, rx, false).await;
    });
}

/// Typed question from the panel.
pub fn ask_text(app: &AppHandle, text: String) {
    interrupt(app);
    let st = state(app);
    let n = {
        let mut i = st.session.inner.lock_ok();
        i.turn += 1;
        i.collapse_gen += 1;
        i.turn
    };
    let rx = begin_capture(app, n);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { run_turn(app, n, text, Some(rx), true).await });
}

pub fn escape(app: &AppHandle) {
    let listening = state(app).session.inner.lock_ok().listening.take();
    if listening.is_some() {
        if crate::stt::recording() {
            crate::stt::cancel(app);
        } else {
            state(app).helper.send("listen_cancel", json!({}));
        }
    }
    interrupt(app);
    crate::jobs::cancel_live(app);
    overlay::clear(app);
    panel::hide(app);
}

/// Dictation is listening now.
pub fn dictating(app: &AppHandle) -> bool {
    state(app).session.inner.lock_ok().listening == Some(Listening::Dictate)
}

pub fn dictate_down(app: &AppHandle) {
    if state(app).session.inner.lock_ok().listening == Some(Listening::Talk) {
        return;
    }
    start_listening(app, Listening::Dictate);
}

pub fn dictate_up(app: &AppHandle) {
    {
        let st = state(app);
        let mut i = st.session.inner.lock_ok();
        if i.listening != Some(Listening::Dictate) {
            return;
        }
        i.listening = None;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let (raw, live) = stop_listening_live(&app).await;
        log::info!("dictation: heard {} chars; live {}", raw.chars().count(), live.as_ref().map(|l| format!("typed {} chars, still on {}", l["typed"].as_str().map(|t| t.chars().count()).unwrap_or(0), l["active"])).unwrap_or_else(|| "off".into()));
        let typed_live = live.as_ref().and_then(|l| l["typed"].as_str()).unwrap_or("").to_string();
        let still_live = live.as_ref().and_then(|l| l["active"].as_bool()).unwrap_or(false);
        if raw.is_empty() {
            if live.is_some() {
                // Whatever was typed while they spoke stays; just let go of the field.
                let _ = state(&app).helper.call("live_type_finish", json!({ "text": typed_live }), Duration::from_secs(5)).await;
            }
            panel::hide(&app);
            return;
        }
        emit(&app, Event::new("dictation.raw", json!({ "text": raw })));
        if still_live {
            // What they said goes in at once; the tidied text replaces it when it's ready.
            let r = state(&app).helper.call("live_type_now", json!({ "text": raw }), Duration::from_secs(5)).await;
            log::info!("dictation: raw text in {r:?}");
        }
        // Pixie's engine writes without capitals: when the tidy-up doesn't run, start with one.
        let text = match clean_dictation(&app, &raw).await {
            Some(t) => t,
            None if raw.chars().next().is_some_and(|c| c.is_lowercase()) => crate::stt::sentence(&raw),
            None => raw,
        };
        let st = app.state::<AppState>();
        if live.is_some() && (still_live || !typed_live.is_empty()) {
            // Type as you speak: the words are already in the field; swap in the cleaned text.
            let r = st.helper.call("live_type_finish", json!({ "text": text }), Duration::from_secs(8)).await;
            log::info!("dictation: live finish {r:?}");
            match r {
                Ok(v) if v["applied"].as_bool() == Some(true) => {
                    emit(&app, Event::new("dictation.done", json!({ "text": text })));
                    panel::set_mode(&app, PanelMode::Toast);
                    schedule_collapse(&app, Duration::from_millis(1400));
                    return;
                }
                Ok(v) if v["reason"].as_str() == Some("focus moved") => {
                    show_error(&app, &format!("You moved to another field, so I stopped typing. What I heard: {text}"), false);
                    return;
                }
                // The field didn't take it: paste it the usual way so nothing is lost.
                _ => crate::heal::note("dictation", "Typing as you speak didn't reach the field", "Pasted the text instead", ""),
            }
        }
        if NO_SURFACE.swap(false, std::sync::atomic::Ordering::SeqCst) {
            // Nowhere to type (the desktop, a list, Pixie itself): it goes on the clipboard.
            let copied = copy_to_clipboard(&text).await;
            log::info!("dictation: no text cursor, copied {} chars: {}", text.chars().count(), if copied { "ok" } else { "failed" });
            emit(&app, Event::new("dictation.done", json!({ "text": text })));
            emit(&app, Event::new("toast", json!({ "text": format!("No text cursor, so it's on the clipboard: {}", text.chars().take(60).collect::<String>()), "kind": "info", "job": null })));
            panel::set_mode(&app, PanelMode::Toast);
            schedule_collapse(&app, Duration::from_millis(2600));
            return;
        }
        let pasted = st.helper.call("insert_text", json!({ "text": text }), Duration::from_secs(5)).await;
        log::info!("dictation: pasted {} chars: {}", text.chars().count(), if pasted.is_ok() { "ok" } else { "failed" });
        match pasted {
            Ok(_) => {
                emit(&app, Event::new("dictation.done", json!({ "text": text })));
                st.play("type-keys");
                panel::set_mode(&app, PanelMode::Toast);
                schedule_collapse(&app, Duration::from_millis(1400));
            }
            // Pasting failed (Accessibility off, the helper busy, the field gone): their words go
            // on the clipboard so nothing they said is lost.
            Err(e) => {
                let copied = copy_to_clipboard(&text).await;
                log::info!("dictation: paste failed ({e}); copied instead: {}", if copied { "ok" } else { "failed" });
                if copied {
                    emit(&app, Event::new("dictation.done", json!({ "text": text })));
                    let paste = if cfg!(target_os = "macos") { "⌘V" } else { "Ctrl+V" };
                    show_error(&app, &format!("I couldn't type it in, so it's on the clipboard. Press {paste} to paste it."), false);
                } else {
                    show_error(&app, &format!("{e} What I heard: {text}"), false);
                }
            }
        }
    });
}

async fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        use tokio::io::AsyncWriteExt;
        let Ok(mut child) = tokio::process::Command::new("/usr/bin/pbcopy").stdin(std::process::Stdio::piped()).spawn() else { return false };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes()).await;
        }
        child.wait().await.map(|s| s.success()).unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let text = text.to_string();
        tauri::async_runtime::spawn_blocking(move || arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).is_ok()).await.unwrap_or(false)
    }
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
    // Short: they're waiting for their words. Too slow and the words go in as heard.
    let t0 = std::time::Instant::now();
    let r = tokio::time::timeout(Duration::from_millis(1500), fut).await;
    log::info!("dictation: tidy-up {} in {} ms", if matches!(r, Ok(Ok(_))) { "done" } else { "skipped" }, t0.elapsed().as_millis());
    match r {
        Ok(Ok(r)) => Some(r.text().trim().to_string()).filter(|t| !t.is_empty()),
        _ => None,
    }
}

pub fn show_error(app: &AppHandle, message: &str, from_mic: bool) {
    emit(app, Event::new("turn.error", json!({ "message": message, "mic": from_mic })));
    state(app).play("nope");
    panel::set_mode(app, PanelMode::Answer);
    schedule_collapse(app, Duration::from_secs(4));
}

/// Asked about another desktop (Space): one line naming the front app's windows that aren't on
/// this one. Silent otherwise, and when there are none.
pub fn desktops_line(request: &str, capture: Option<&Capture>) -> String {
    let r = request.to_lowercase();
    let asks = ["other desktop", "another desktop", "desktops", "other space", "another space", "spaces", "minimized", "minimised", "hidden window", "other window"]
        .iter()
        .any(|k| r.contains(k));
    let Some(c) = capture.filter(|c| asks && !c.elsewhere.is_empty()) else { return String::new() };
    format!(
        "{} also has windows on another desktop or minimized (not visible, can't be seen or clicked from here): {}. ctrl+left / ctrl+right (mac_step key) switches desktops if they ask.",
        if c.app.is_empty() { "The front app" } else { c.app.as_str() },
        c.elsewhere.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ")
    )
}

fn environment(app: &AppHandle, cwd: &std::path::Path, capture: Option<&Capture>) -> prompt::Environment {
    let st = state(app);
    let s = st.settings.lock_ok().clone();
    prompt::Environment {
        version: app.package_info().version.to_string(),
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
        memory: crate::memory::prompt_lines(app),
        helper: crate::jobs::helper_hint(&st),
        coders: pixie_tools::coders::detect().iter().map(|c| c.label.to_string()).collect(),
        persona: s.persona.clone(),
        spicy: s.spicy,
        canvas: s.canvas_use.clone(),
        jobs: st.jobs.list().iter().take(5).map(|j| j.line()).collect(),
        screens: screens_now(app, capture),
    }
}

/// One line about their screens when there are several (none with one screen).
fn screens_now(app: &AppHandle, capture: Option<&Capture>) -> String {
    let ds = screen::displays(app);
    if ds.len() < 2 {
        return String::new();
    }
    let pointer = capture
        .and_then(|c| c.cursor.as_ref())
        .and_then(|c| Some((c["x"].as_f64()?, c["y"].as_f64()?)))
        .unwrap_or_else(crate::macos::cursor);
    screen::screens_line(&ds, pointer, capture.and_then(|c| c.window_frame.as_ref()))
}

/// One spoken turn.
async fn run_turn(app: AppHandle, n: u64, text: String, capture: Option<oneshot::Receiver<Capture>>, typed: bool) {
    // "Quit", "stop working", "never mind": done at once, without asking the model.
    if let Some(q) = crate::myself::quick(&text) {
        crate::myself::run_quick(&app, q);
        return;
    }
    // Stage times go to the log, so a slow answer shows where the time went.
    let began = std::time::Instant::now();
    let st = app.state::<AppState>();
    let cancel = CancellationToken::new();
    {
        let mut i = st.session.inner.lock_ok();
        if i.turn != n {
            return; // superseded
        }
        i.cancel = Some(cancel.clone());
        i.answering = true;
        let reset = st.settings.lock().map(|s| s.chat_reset_hour).unwrap_or(0);
        if i.ensure_today(&st.data_dir, reset) {
            emit(&app, Event::new("chat.updated", json!({ "log": i.log, "id": i.chat_id })));
        }
        i.last_turn = Some(Instant::now());
    }
    // Carrying on after a while: say how long it's been.
    let gap_note = {
        let i = st.session.inner.lock_ok();
        let since = i.log.last().map(|e| chrono::Utc::now().timestamp_millis() - e.at).unwrap_or(0);
        if since > GAP_NOTE.as_millis() as i64 {
            let mins = since / 60_000;
            let ago = if mins < 120 { format!("{mins} minutes") } else if mins < 48 * 60 { format!("{} hours", mins / 60) } else { format!("{} days", mins / 1440) };
            format!("[{ago} since the last message; it's now {}. What's on screen now may be different.] ", chrono::Local::now().format("%H:%M"))
        } else {
            String::new()
        }
    };
    panel::set_mode(&app, PanelMode::Thinking);
    emit(&app, Event::new("turn.start", json!({ "turn": n, "text": text })));
    // A typed question can be answered in text (Settings → General → Typed questions).
    let (text_reply, show_text) = st.settings.lock().map(|s| (typed && s.typed_answers == "text", s.show_typed_text)).unwrap_or((false, true));
    if text_reply && show_text {
        emit(&app, Event::new("turn.textmode", json!({ "turn": n })));
    }

    let Some(provider) = st.provider() else {
        finish_turn(&app, n, false);
        show_error(&app, "I need a model key first. Open Settings → AI and paste a DeepSeek or Claude key.", false);
        return;
    };

    let cap = match capture {
        Some(rx) => tokio::time::timeout(Duration::from_secs(4), rx).await.ok().and_then(Result::ok),
        None => {
            screen::begin_turn(n);
            None
        }
    };
    *st.last_capture.lock_ok() = cap.clone();
    let session_dir = st.data_dir.join("session");
    let _ = std::fs::create_dir_all(&session_dir);
    let note_path = session_dir.join("note.md");
    // A fresh page for every message.
    let _ = std::fs::write(&note_path, "");
    emit(&app, Event::new("session.note", json!({ "doc": null })));

    let mode = Mode::Voice;
    // Questions that clearly aren't about the screen ("what's the capital of…") skip it: no
    // screenshot, no screen text. The model can still call look.
    let use_screen = screen::about_screen(&text);
    let task = match &cap {
        Some(c) if use_screen => c.describe_for(&text),
        Some(_) => "Not looked at for this request (it doesn't seem to be about the screen). Call look if it is.".to_string(),
        None => String::new(),
    };
    let mut input = PromptInput { mode, env: environment(&app, &session_dir, cap.as_ref()), task };
    input.env.screenshot &= use_screen;
    let desktops = desktops_line(&text, cap.as_ref());
    if !desktops.is_empty() {
        input.env.screens = [input.env.screens.as_str(), desktops.as_str()].iter().filter(|l| !l.is_empty()).copied().collect::<Vec<_>>().join(" ");
    }
    // Stable system prompt (cached); the time, front app and screen lead the newest message.
    let system = prompt::assemble(&input);
    let now_context = prompt::context(&input);
    let mut content = vec![];
    // Numbers in marks are read off the screenshot when the model sees one.
    crate::narrate::set_saw_image(provider.supports_images());
    if provider.supports_images() && use_screen {
        if let Some(b64) = cap.as_ref().and_then(Capture::image_base64) {
            content.push(Content::jpeg_base64(b64));
        }
    }
    let reading = if text_reply { "\n\nHOW THEY GET YOUR ANSWER\nThey typed this and will read your answer, not hear it: plain sentences, a little more detail is fine, no Markdown." } else { "" };
    content.push(Content::text(format!("{now_context}{reading}\n\nTHEIR REQUEST\n{gap_note}{text}")));
    let mut messages = {
        let i = st.session.inner.lock_ok();
        let mut m = vec![];
        if !i.meta.summary.is_empty() {
            m.push(Message::user_text(format!("(Earlier in today's conversation, in short: {})", i.meta.summary)));
            m.push(Message::assistant(vec![Content::text("Okay.")]));
        }
        m.extend(i.history.iter().cloned());
        m
    };
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
        coding: st.settings.lock().map(|s| s.coding.clone()).unwrap_or_default(),
    };
    let mut tools = pixie_tools::defs::for_mode(mode, crate::browser::connected(&app));
    tools.extend(pixie_tools::Host::extra_tools(host.as_ref(), mode));
    crate::connectors::trim(&mut tools, &text);
    // Everything before the new message is earlier conversation (cached for the next turn).
    let opts = AgentOptions { max_steps: 12, effort: Effort::Fast, max_tokens: 1500, history: Some(messages.len().saturating_sub(1)) };

    let mut voice = SpeechStream::new(&app, n, !text_reply);
    let started = std::time::Instant::now();
    log::info!("turn {n}: asking the model {} ms after the turn began (screen read, prompt built)", began.elapsed().as_millis());
    // Which tools this answer used, kept with it in history: without them a repeated request
    // ("start a job…") reads like it was answered in words only, and the model may copy that.
    let used = Arc::new(Mutex::new(Vec::<String>::new()));
    let used_ev = used.clone();
    let app_ev = app.clone();
    let mut first = true;
    let result = crate::opencode::run_agent(&app, provider.as_ref(), &system, messages, tools, &ctx, &opts, &mut |e| match e {
        AgentEvent::Text(d) => {
            if first {
                first = false;
                log::info!("turn {n}: first words after {} ms", started.elapsed().as_millis());
                panel::set_mode(&app_ev, PanelMode::Answer);
            }
            let shown = voice.push(&d);
            if !shown.is_empty() {
                emit(&app_ev, Event::new("turn.delta", json!({ "turn": n, "text": shown })));
            }
        }
        AgentEvent::ToolStart { name, input } => {
            if let Ok(mut u) = used_ev.lock() {
                if !u.contains(&name) {
                    u.push(name.clone());
                }
            }
            log::info!("turn {n}: {name} at {} ms {}", started.elapsed().as_millis(), input.to_string().chars().take(240).collect::<String>());
            emit(&app_ev, Event::new("turn.tool", json!({ "turn": n, "name": name, "input": input })));
        }
        AgentEvent::ToolEnd { name, ok, summary } => {
            log::info!("turn {n}: {name} {} at {} ms {summary}", if ok { "ok" } else { "failed" }, started.elapsed().as_millis());
            emit(&app_ev, Event::new("turn.tool_done", json!({ "turn": n, "name": name, "ok": ok, "summary": summary })));
        }
        AgentEvent::Step { .. } => {
            let held = voice.flush();
            if !held.is_empty() {
                emit(&app_ev, Event::new("turn.delta", json!({ "turn": n, "text": held })));
            }
        }
    })
    .await;
    log::info!("turn {n}: the model finished {} ms after it was asked", started.elapsed().as_millis());
    let held = voice.flush();
    if !held.is_empty() {
        emit(&app, Event::new("turn.delta", json!({ "turn": n, "text": held })));
    }
    // Marks drawn while speaking aren't part of what was said.
    let result = result.map(|mut o| {
        o.final_text = crate::narrate::strip(&o.final_text);
        o
    });

    match result {
        Ok(out) => {
            {
                let mut i = st.session.inner.lock_ok();
                let clean = strip_tool_note(&out.final_text);
                let a = if clean.trim().is_empty() { "(showed it on screen)".to_string() } else { clean };
                let entry = ChatEntry { q: text.clone(), a, at: chrono::Utc::now().timestamp_millis(), tools: used.lock().map(|u| u.clone()).unwrap_or_default() };
                i.history.extend(entry.messages());
                let excess = i.history.len().saturating_sub(HISTORY_MAX);
                i.history.drain(..excess);
                i.log.push(entry);
                let excess = i.log.len().saturating_sub(LOG_MAX);
                i.log.drain(..excess);
                let id = i.chat_id.get_or_insert_with(crate::chats::new_id).clone();
                i.fresh = false;
                crate::chats::save(&st.data_dir, &id, &i.log, &i.meta);
                emit(&app, Event::new("chat.updated", json!({ "log": i.log, "id": id })));
                // Turns that dropped out of the window get folded into the summary.
                let upto = i.log.len().saturating_sub(KEEP_TURNS);
                if upto >= i.meta.summarized + 6 {
                    let older = i.log[i.meta.summarized..upto].to_vec();
                    let prev = i.meta.summary.clone();
                    tauri::async_runtime::spawn(summarize(app.clone(), id, prev, older, upto));
                }
            }
            crate::myself::ensure_persona(&app, &text);
            // They asked for a job in so many words and the model only said it started one.
            let used_now = used.lock().map(|u| u.clone()).unwrap_or_default();
            if !used_now.iter().any(|t| t == "start_job") {
                if let Some((title, kind)) = job_request(&text) {
                    log::info!("job: asked for a {kind} job but none was started; starting it");
                    match crate::jobs::start(&app, &title, &text, kind) {
                        Ok(_) => crate::jobs::toast(&app, &format!("Started: {title}"), "info", None),
                        Err(e) => crate::jobs::toast(&app, &e, "error", None),
                    }
                }
            }
            emit(&app, Event::new("turn.done", json!({ "turn": n, "text": out.final_text })));
            panel::set_mode(&app, PanelMode::Answer);
            finish_turn(&app, n, voice.spoke_anything());
            if text_reply && show_text {
                // Long enough to read it (about 3 words a second, at least 12 s); hovering keeps it.
                let words = out.final_text.split_whitespace().count() as u64;
                schedule_collapse(&app, Duration::from_secs((words / 3 + 6).clamp(12, 90)));
            }
        }
        Err(pixie_llm::LlmError::Cancelled) => finish_turn(&app, n, false),
        Err(e) => {
            log::warn!("turn {n}: {e}");
            let err = e.to_string();
            if let Some((what, fix)) = crate::heal::classify(&err) {
                crate::heal::note("model", &format!("{what} ({})", active_provider(&app)), fix, &err.chars().take(400).collect::<String>());
            }
            // A wrong or retired model id: go back to the provider's default and ask again once.
            if crate::myself::model_missing(&e.to_string()) && !retried(&app, n) {
                let provider = active_provider(&app);
                if crate::myself::check_choices(&app, &provider).await {
                    finish_turn(&app, n, false);
                    run_again(&app, text, typed);
                    return;
                }
            }
            finish_turn(&app, n, false);
            show_error(&app, &e.to_string(), false);
        }
    }
}

/// Fold older turns into the chat's running summary (cheap model, in the background).
async fn summarize(app: AppHandle, id: String, prev: String, older: Vec<ChatEntry>, upto: usize) {
    let Some(provider) = state(&app).provider() else { return };
    let mut msg = String::new();
    if !prev.is_empty() {
        msg.push_str(&format!("Summary so far: {prev}\n\nMore of the conversation:\n"));
    }
    for e in &older {
        msg.push_str(&format!("They: {}\nPixie: {}\n", e.q.chars().take(500).collect::<String>(), e.a.chars().take(500).collect::<String>()));
    }
    let mut req = Request::new(
        "Summarise this part of a conversation between the user and Pixie, their desktop helper, in under 120 words: what they asked about, what was decided or done, names, numbers and anything still open. Merge it with the summary so far if there is one. Plain sentences, no lists.",
        vec![Message::user_text(msg)],
    );
    req.max_tokens = 300;
    req.effort = Effort::Fast;
    let Ok(Ok(r)) = tokio::time::timeout(Duration::from_secs(40), provider.complete(&req, &mut |_| {})).await else { return };
    let summary = r.text().trim().to_string();
    if summary.is_empty() {
        return;
    }
    let st = state(&app);
    let mut i = st.session.inner.lock_ok();
    if i.chat_id.as_deref() == Some(id.as_str()) && upto > i.meta.summarized {
        i.meta.summary = summary;
        i.meta.summarized = upto;
        crate::chats::save(&st.data_dir, &id, &i.log, &i.meta);
    }
}

/// Long-term memory (memory = auto): once a chat has been quiet a few minutes, read what's new
/// in it for lasting facts. Looks at the last few chats, so one left by New chat isn't missed.
pub fn spawn_learner(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(90)).await;
        loop {
            if crate::memory::mode(&app) == "auto" {
                let data = state(&app).data_dir.clone();
                let quiet = chrono::Utc::now().timestamp_millis() - 3 * 60_000;
                for c in crate::chats::list(&data).into_iter().take(3) {
                    let Some(chat) = crate::chats::get(&data, &c.id) else { continue };
                    let n = chat.entries.len();
                    if chat.updated > quiet || chat.meta.learned >= n {
                        continue;
                    }
                    crate::memory::learn(&app, &chat.entries[chat.meta.learned.min(n)..]).await;
                    state(&app).session.set_learned(&data, &chat.id, n);
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

fn finish_turn(app: &AppHandle, n: u64, spoke: bool) {
    let st = state(app);
    {
        let mut i = st.session.inner.lock_ok();
        if i.turn == n {
            i.answering = false;
            i.cancel = None;
        }
    }
    // Close soon once nothing is being said: also when the speech already finished before the
    // model call ended (then no speech-end event is left to close it).
    if !st.session.is_speaking() {
        schedule_collapse(app, if spoke { Duration::from_millis(800) } else { Duration::from_millis(1500) });
    }
}

/// Collapse the panel after `after`, unless something new happened or it's hovered.
pub fn schedule_collapse(app: &AppHandle, after: Duration) {
    let st = state(app);
    let gen = {
        let mut i = st.session.inner.lock_ok();
        i.collapse_gen += 1;
        i.collapse_gen
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(after).await;
        let st = app.state::<AppState>();
        let (same, hovered, busy, listening) = {
            let i = st.session.inner.lock_ok();
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
    // Its marks still show.
    crate::narrate::ended(app, id);
    let st = app.state::<AppState>();
    let all_done = st.session.speaking_ended(id);
    emit(app, Event::new("speech.end", json!({ "id": id, "cancelled": true })));
    if all_done && !st.session.busy() {
        schedule_collapse(app, Duration::from_secs(2));
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
                "speak_start" => {
                    crate::narrate::started(&app, data["id"].as_str().unwrap_or(""));
                    emit(&app, Event::new("speech.start", data));
                }
                "speak_word" => {
                    let at = data["at"].as_u64().unwrap_or(0) as usize;
                    crate::narrate::word(&app, data["id"].as_str().unwrap_or(""), at);
                    emit(&app, Event::new("speech.word", data));
                }
                "speak_end" => {
                    let id = data["id"].as_str().unwrap_or("").to_string();
                    if data["cancelled"].as_bool() != Some(true) {
                        crate::narrate::ended(&app, &id);
                    }
                    let st = app.state::<AppState>();
                    let all_done = st.session.speaking_ended(&id);
                    emit(&app, Event::new("speech.end", data));
                    if all_done && !st.session.busy() {
                        // Voice-only answers: tuck the notch back right after the last word.
                        schedule_collapse(&app, Duration::from_millis(800));
                        overlay::schedule_clear(&app, Duration::from_secs(6));
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

#[cfg(test)]
mod desktops_tests {
    use super::{desktops_line, Capture};

    #[test]
    fn desktops_line_only_when_asked() {
        let c = Capture { app: "Safari".into(), elsewhere: vec!["Docs".into()], ..Default::default() };
        assert!(desktops_line("what's on my other desktop?", Some(&c)).contains("Safari also has windows"));
        assert_eq!(desktops_line("what's this button?", Some(&c)), "");
        let none = Capture::default();
        assert_eq!(desktops_line("what's on my other desktop?", Some(&none)), "");
    }
}

#[cfg(test)]
mod job_request_tests {
    use super::job_request;

    #[test]
    fn tool_notes_are_scrubbed() {
        use super::strip_tool_note;
        assert_eq!(strip_tool_note("Done.\n(Tools used for this answer: pixie, code. A new request needs its own calls.)"), "Done.");
        assert_eq!(strip_tool_note("(Tools used for this answer: start_job."), "");
        assert_eq!(strip_tool_note("A new request needs its own calls.)"), "");
        assert_eq!(strip_tool_note("Canberra."), "Canberra.");
    }

    #[test]
    fn explicit_job_requests_are_caught() {
        let (t, k) = job_request("Start a background job: find five popular open-source note-taking apps").unwrap();
        assert_eq!((t.as_str(), k), ("Five Popular Open-source Note-taking", "background"));
        assert_eq!(job_request("start a live agent to rename these files").unwrap().1, "live");
        assert!(job_request("what's a background job?").is_none());
        assert!(job_request("how do jobs work").is_none());
        assert!(job_request("what's the weather").is_none());
    }
}
