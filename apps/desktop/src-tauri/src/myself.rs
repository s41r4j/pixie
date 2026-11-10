//! Pixie knows itself. The `pixie` tool answers "what can you do / how are you set up" from the
//! live state (version, shortcuts, voice, model, permissions, apps, connectors, experiments,
//! updates, usage), and changes Pixie's own settings in one call, in-process: no clicking through
//! the Settings window. Changes go through the same path as the Settings window (`settings_set`),
//! so hotkeys, sounds, the model and the tray update at once, and an open Settings window reloads.
//!
//! API keys and the Google client secret are never read or set here: they're typed into
//! Settings by the user.

use crate::{commands, AppState};
use openclicky_llm::ToolDef;
use serde_json::{json, Map, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const TABS: &[(&str, &str)] = &[
    ("general", "general"),
    ("voice", "voice"),
    ("sounds", "sounds"),
    ("models", "ai"),
    ("ai", "ai"),
    ("usage", "usage"),
    ("coding", "coding"),
    ("permissions", "setup"),
    ("setup", "setup"),
    ("apps", "apps"),
    ("connectors", "connectors"),
    ("experimental", "experimental"),
    ("about", "about"),
    ("memory", "memory"),
    ("updates", "about"),
];

pub fn tool() -> ToolDef {
    ToolDef {
        name: "pixie".into(),
        description: "You, Pixie: your own settings and facts, changed instantly (never click through your own Settings window).\n\
about: everything about you now (version, abilities, shortcuts, voice, model, permissions, apps, connectors, updates, usage, every setting and value). Use it before answering how you're set up.\n\
set changes={key: value, …}: speak_answers, look_at_screen, can_click, auto_update, muted, spicy (Xtrem swears mildly), connectors (the Experimental switch) as bools · buddy (always|idle|off) · name (what to call them) · voice (Heart, Bella, Nicole, Sarah, Michael, Fenrir, Emma, George, system) · voice_speed (0.8–1.3) · voice_loudness (quiet|normal|loud) · speech_rate (system voice 0.4–0.65) · volume (sounds 0–1.5) · language (en-US… or auto) · talk_shortcut, dictate_shortcut (ctrl+option, fn+ctrl…) · dictate_mode (hold|toggle: press to start, press to stop) · dictate_live (bool, the Experimental switch: type as you speak, else typed when done) · notch_screen (follow|builtin|external|a screen's name: where the notch opens) · typed_answers (spoken|text: how typed questions are answered) · show_typed_text (bool) · provider (claude|openai|deepseek|opencode = free models through opencode on this computer) · model, agents_model, backup_model (provider:model or none) · coding_agent (auto|codex|claude|opencode), coding_folder · memory (auto|manual|off) · new_chat_at (an hour) · persona (pix friendly|inos nerdy|xtrem witty|eon concise) · drawing (asked|helps|often) · calendar, reminders, contacts, mail · connector:<id>. Say what changed in a few words; never claim a change this call didn't make.\n\
open page=general|voice|sounds|memory|models|usage|coding|permissions|apps|connectors|experimental|about|canvas (keys, permissions and sign-ins are theirs to do there).\n\
jobs (every job: state, progress, plan, which coding agent did the work, report, errors; id=<job id or title> adds its note): use it before answering how a job is going or what it did.\n\
check_updates, restart_to_update, new_chat (fresh conversation), stop_all (every job and live agent), quit (when they say quit, exit or close you; say a short bye first).\n\
checkup (\"check yourself\", \"is everything working?\"): helper, permissions, model, voice, disk, recent problems; it fixes what it can. issues: problems you worked around lately. report what=<their words> browser=<chrome|safari|arc|firefox…>: opens a GitHub issue in their browser, filled in with what happened, your version and log lines (personal paths, keys and emails removed); they read it and press Submit themselves. Ask which browser they're signed in to GitHub in if they haven't said.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["about", "set", "open", "jobs", "check_updates", "restart_to_update", "quit", "stop_all", "new_chat", "checkup", "issues", "report"] },
                "changes": { "type": "object", "description": "set: key → new value" },
                "what": { "type": "string", "description": "report: what went wrong, in their words" },
                "browser": { "type": "string", "description": "report: the browser they use GitHub in" },
                "page": { "type": "string" },
                "id": { "type": "string", "description": "jobs: one job's id or title" }
            },
            "required": ["action"]
        }),
    }
}

pub async fn call(app: &AppHandle, input: &Value) -> Result<String, String> {
    match input.get("action").and_then(Value::as_str).unwrap_or("about") {
        "about" => Ok(about(app).await),
        "jobs" => Ok(jobs_report(app, input.get("id").and_then(Value::as_str).unwrap_or(""))),
        "set" => {
            let changes = input.get("changes").and_then(Value::as_object).cloned().unwrap_or_default();
            if changes.is_empty() {
                return Err("Give changes, e.g. {\"speak_answers\": false}.".into());
            }
            set(app, &changes).await
        }
        "open" => open(app, input.get("page").and_then(Value::as_str).unwrap_or("general")),
        "check_updates" => {
            let s = crate::updates::check(app).await?;
            Ok(describe_update(&s))
        }
        "restart_to_update" => crate::updates::restart_now(app).map(|_| "Restarting into the new version.".into()),
        "quit" => {
            let n = running_jobs(app);
            quit_soon(app, None);
            Ok(if n > 0 {
                format!("Quitting once you've said goodbye; {n} running job(s) stop with it. Say one short goodbye.")
            } else {
                "Quitting once you've said goodbye. Say one short goodbye.".into()
            })
        }
        "new_chat" => {
            new_chat(app);
            Ok("Started a new chat; the earlier one is saved in Chats.".into())
        }
        "stop_all" => {
            let n = stop_jobs(app);
            Ok(if n == 0 { "Nothing was running.".into() } else { format!("Stopped {n} job(s).") })
        }
        "checkup" => Ok(crate::heal::summary(&checkup(app).await)),
        "issues" => Ok(issues_report(app)),
        "report" => {
            let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let opened = open_report(app, &s("what"), &s("browser"))?;
            Ok(format!("The issue page is open in {opened}, filled in. Tell them to read it, change anything they like, and press Submit if they're happy; you don't post it."))
        }
        other => Err(format!("Unknown action \"{other}\".")),
    }
}

fn open(app: &AppHandle, page: &str) -> Result<String, String> {
    let page = page.trim().to_lowercase();
    if page == "canvas" {
        crate::canvas::canvas_open(app.clone(), None, None);
        return Ok("The canvas is open.".into());
    }
    let tab = TABS.iter().find(|(k, _)| *k == page).map(|(_, t)| *t).ok_or_else(|| format!("No Settings page \"{page}\"."))?;
    crate::windows::open_settings(app, tab);
    Ok(format!("Opened Settings → {page}."))
}

fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "on" | "true" | "yes" | "enabled" | "enable" => Some(true),
            "off" | "false" | "no" | "disabled" | "disable" => Some(false),
            _ => None,
        },
        Value::Number(n) => n.as_f64().map(|f| f != 0.0),
        _ => None,
    }
}

fn as_num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches('%').trim_end_matches('x').parse().ok(),
        _ => None,
    }
}

fn as_str(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.trim().to_string())
}

/// "midnight", "4am", "4 pm", "06:00", 5 → an hour 0–23.
fn hour_of(v: &Value) -> Option<u32> {
    if let Some(n) = v.as_u64() {
        return (n < 24).then_some(n as u32);
    }
    let s = v.as_str()?.trim().to_lowercase().replace([' ', '.'], "");
    match s.as_str() {
        "midnight" => return Some(0),
        "noon" | "midday" => return Some(12),
        _ => {}
    }
    let (num, pm) = if let Some(n) = s.strip_suffix("pm") { (n, Some(true)) } else if let Some(n) = s.strip_suffix("am") { (n, Some(false)) } else { (s.as_str(), None) };
    let h: u32 = num.split(':').next()?.parse().ok()?;
    let h = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        _ => h,
    };
    (h < 24).then_some(h)
}

/// "New chat": the panel's button, the pixie tool and the spoken command.
pub fn new_chat(app: &AppHandle) {
    app.state::<AppState>().session.chat_clear();
    crate::emit(app, openclicky_core::Event::new("chat.updated", json!({ "log": [] })));
}

pub fn persona_name(id: &str) -> &'static str {
    match id {
        "inos" => "Inos (nerdy, curious)",
        "xtrem" => "Xtrem (chaotic, witty)",
        "eon" => "Eon (calm, concise)",
        _ => "Pix (friendly, warm)",
    }
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// Apply changes; every key is checked first, the good ones are applied, the bad ones explained.
/// Their jobs for Pixie: every job with state, progress, plan, who coded, the report and any
/// error; with an id, that job's note too.
fn jobs_report(app: &AppHandle, id: &str) -> String {
    let st = app.state::<AppState>();
    let all = st.jobs.list();
    if all.is_empty() {
        return "No jobs yet.".into();
    }
    let detail = |j: &crate::jobs::JobView, full: bool| {
        let mut s = format!("{}\n  brief: {}", j.line(), j.instructions.chars().take(if full { 1200 } else { 160 }).collect::<String>());
        if !j.plan.is_empty() {
            let steps: Vec<String> = j.plan.iter().map(|p| format!("{} [{:?}]", p.label, p.status).to_lowercase()).collect();
            s.push_str(&format!("\n  plan: {}", steps.join("; ")));
        }
        if let Some(q) = &j.question {
            s.push_str(&format!("\n  asking them: {}", q.question));
        }
        if let Some(r) = &j.report {
            s.push_str(&format!("\n  report: {}", r.body.chars().take(if full { 1500 } else { 300 }).collect::<String>()));
            if !r.artifacts.is_empty() {
                s.push_str(&format!("\n  made: {}", r.artifacts.join(", ")));
            }
        }
        if let Some(e) = &j.error {
            s.push_str(&format!("\n  error: {e}"));
        }
        if full {
            let note = std::fs::read_to_string(j.cwd.join("note.md")).unwrap_or_default();
            if !note.trim().is_empty() {
                s.push_str(&format!("\n  note:\n{}", note.chars().take(3000).collect::<String>()));
            }
        }
        s
    };
    if !id.is_empty() {
        return match all.iter().find(|j| j.id == id || j.title.eq_ignore_ascii_case(id)) {
            Some(j) => detail(j, true),
            None => format!("No job {id}. Jobs: {}", all.iter().map(|j| format!("{} ({})", j.title, j.id)).collect::<Vec<_>>().join(", ")),
        };
    }
    all.iter().take(10).map(|j| detail(j, false)).collect::<Vec<_>>().join("\n")
}

/// Pick the provider's real model for what they said: an exact id, the same id or official name
/// ignoring case and punctuation ("deepseek v4.1 flash" → deepseek-flash, whose name is
/// DeepSeek-V4.1-Flash), else the first model (newest) whose id and name hold every word.
pub fn pick_model(list: &[(String, String)], wanted: &str) -> Option<String> {
    let norm = |s: &str| s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>();
    let w = wanted.trim();
    if let Some((id, _)) = list.iter().find(|(id, _)| id == w) {
        return Some(id.clone());
    }
    let nw = norm(w);
    if let Some((id, _)) = list.iter().find(|(id, name)| norm(id) == nw || (!name.is_empty() && norm(name) == nw)) {
        return Some(id.clone());
    }
    let words: Vec<String> = w.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|x| !x.is_empty()).map(str::to_string).collect();
    if words.is_empty() {
        return None;
    }
    list.iter()
        .find(|(id, name)| {
            let hay = format!("{id} {name}").to_lowercase();
            words.iter().all(|x| hay.contains(x.as_str()))
        })
        .map(|(id, _)| id.clone())
}

/// What they named → that provider's real model id, from its live list. An error lists a few
/// it does have, so a name from another provider (an opencode id on DeepSeek) is never saved.
pub async fn resolve_model(app: &AppHandle, provider: &str, wanted: &str) -> Result<String, String> {
    let st = app.state::<AppState>();
    let list: Vec<(String, String)> = if provider == "opencode" {
        crate::opencode::refresh_models(app).await?.iter().map(|m| (m["id"].as_str().unwrap_or("").to_string(), m["name"].as_str().unwrap_or("").to_string())).collect()
    } else {
        let settings = st.settings.lock().map(|s| s.clone()).unwrap_or_default();
        crate::usage::fetch_models(&st.data_dir, &settings, provider)
            .await?
            .iter()
            .map(|m| (m["id"].as_str().unwrap_or("").to_string(), m["name"].as_str().unwrap_or("").to_string()))
            .collect()
    };
    pick_model(&list, wanted).ok_or_else(|| {
        let some: Vec<&str> = list.iter().take(6).map(|(id, _)| id.as_str()).collect();
        format!("{provider} has no model like \"{wanted}\"; it has {}", some.join(", "))
    })
}

/// Drop talk / agents model choices the provider doesn't have (a name left from another
/// provider, or a model it retired), so it falls back to its default. True when one was dropped.
pub async fn check_choices(app: &AppHandle, provider: &str) -> bool {
    let st = app.state::<AppState>();
    let cur = st.settings.lock().map(|s| s.clone()).unwrap_or_default();
    let Some(choice) = cur.models.get(provider).cloned().filter(|c| !c.fast.is_empty() || !c.deep.is_empty()) else { return false };
    let ids: Vec<String> = if provider == "opencode" {
        match crate::opencode::refresh_models(app).await {
            Ok(m) => m.iter().filter_map(|x| x["id"].as_str().map(str::to_string)).collect(),
            Err(_) => return false,
        }
    } else {
        match crate::usage::fetch_models(&st.data_dir, &cur, provider).await {
            Ok(m) => m.iter().filter_map(|x| x["id"].as_str().map(str::to_string)).collect(),
            Err(_) => return false,
        }
    };
    if ids.is_empty() {
        return false;
    }
    let mut fixed = choice.clone();
    let mut gone = Vec::new();
    for (slot, label) in [(&mut fixed.fast, "talk"), (&mut fixed.deep, "agents")] {
        if !slot.is_empty() && !ids.contains(slot) {
            // A retired model: the newest of the same family takes over; else the default.
            match successor(slot, &ids) {
                Some(next) => {
                    gone.push(format!("The {label} model {slot} is retired on {provider}; using {next}, the newest of its family"));
                    *slot = next;
                }
                None => {
                    gone.push(format!("The {label} model {slot} isn't available on {provider}; using its default"));
                    slot.clear();
                }
            }
        }
    }
    if gone.is_empty() {
        return false;
    }
    let mut models = cur.models.clone();
    models.insert(provider.to_string(), fixed);
    if commands::settings_set(app.clone(), json!({ "models": models })).is_ok() {
        crate::emit(app, openclicky_core::Event::new("settings.external", json!({})));
        let msg = format!("{}.", gone.join(". "));
        crate::heal::note("model", &msg, "Switched the model", "");
        log::warn!("models: {msg}");
        crate::jobs::toast(app, &msg, "info", None);
    }
    true
}

/// A model id's family and version: `claude-sonnet-4-5-20250929` is (claude sonnet, [4, 5]);
/// `gpt-5.1-mini` is (gpt mini, [5, 1]). Dates and other long numbers aren't the version.
fn family(id: &str) -> (String, Vec<u32>) {
    let id = id.rsplit('/').next().unwrap_or(id).to_lowercase();
    let mut words = vec![];
    let mut version = vec![];
    for part in id.split(['-', '.', '_', ':', ' ']).filter(|p| !p.is_empty()) {
        if part.chars().all(|c| c.is_ascii_digit()) {
            if part.len() <= 3 {
                version.push(part.parse().unwrap_or(0));
            }
        } else if let Some(n) = part.strip_prefix('v').filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            version.push(n.parse().unwrap_or(0));
        } else {
            // "4o" and the like: keep the word, the number is part of the name.
            words.push(part.to_string());
        }
    }
    (words.join(" "), version)
}

/// The newest model of `old`'s family in `ids` (for a retired id), preferring the plain alias
/// over a dated snapshot of the same version.
pub fn successor(old: &str, ids: &[String]) -> Option<String> {
    let (fam, _) = family(old);
    if fam.is_empty() {
        return None;
    }
    ids.iter()
        .filter(|id| id.as_str() != old)
        .filter_map(|id| {
            let (f, v) = family(id);
            (f == fam).then(|| (v, std::cmp::Reverse(id.len()), id.clone()))
        })
        .max()
        .map(|(_, _, id)| id)
}

/// The provider said the model doesn't exist (a wrong or retired model id).
pub fn model_missing(e: &str) -> bool {
    let e = e.to_lowercase();
    e.contains("model") && ["not exist", "not found", "does not exist", "invalid model", "unknown model", "unavailable", "no such model"].iter().any(|k| e.contains(k))
}

async fn set(app: &AppHandle, changes: &Map<String, Value>) -> Result<String, String> {
    let st = app.state::<AppState>();
    let data = st.data_dir.clone();
    let cur = st.settings.lock().map(|s| s.clone()).unwrap_or_default();
    let mut patch = Map::new();
    let mut done: Vec<String> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    let mut later: Vec<(String, bool)> = Vec::new(); // apps
    let mut connectors: Vec<(String, bool)> = Vec::new();

    for (raw, v) in changes {
        let key = raw.trim().to_lowercase().replace([' ', '-'], "_");
        let mut fail = |why: String| bad.push(format!("{raw}: {why}"));
        match key.as_str() {
            "dictate_mode" | "dictation_mode" | "dictate_toggle" => {
                let w = v.as_str().map(str::to_lowercase).unwrap_or_default();
                let mode = match (w.as_str(), as_bool(v)) {
                    ("toggle" | "press" | "tap" | "start stop" | "hands free", _) | (_, Some(true)) => Some("toggle"),
                    ("hold" | "push" | "push to talk", _) | (_, Some(false)) => Some("hold"),
                    _ => None,
                };
                match mode {
                    Some(m) => {
                        patch.insert("dictate_mode".into(), json!(m));
                        done.push(if m == "toggle" { "dictation now starts and stops with a press".into() } else { "dictation is hold to talk again".to_string() });
                    }
                    None => fail("hold or toggle".into()),
                }
            }
            "typed_answers" | "typed_replies" | "reply_mode" => {
                let w = v.as_str().map(str::to_lowercase).unwrap_or_default();
                let mode = match (w.as_str(), as_bool(v)) {
                    ("text" | "typed" | "written" | "write" | "same", _) | (_, Some(true)) => Some("text"),
                    ("spoken" | "voice" | "speak" | "out loud" | "aloud", _) | (_, Some(false)) => Some("spoken"),
                    _ => None,
                };
                match mode {
                    Some(m) => {
                        patch.insert("typed_answers".into(), json!(m));
                        done.push(if m == "text" { "typed questions get typed answers".into() } else { "typed questions are answered out loud".to_string() });
                    }
                    None => fail("text or spoken".into()),
                }
            }
            "notch_screen" | "notch_on" | "notch_display" => {
                let w = v.as_str().unwrap_or("").trim().to_lowercase();
                let pick = match w.as_str() {
                    "follow" | "auto" | "current" | "the one i'm using" | "mine" | "" => Some("follow".to_string()),
                    "builtin" | "built-in" | "built in" | "laptop" | "internal" | "main" => Some("builtin".to_string()),
                    other => crate::screen::displays(app).into_iter().map(|d| d.name).find(|n| n.to_lowercase().contains(other)).or_else(|| (other == "external" || other == "monitor").then(|| crate::screen::displays(app).into_iter().find(|d| !d.name.to_lowercase().contains("built")).map(|d| d.name)).flatten()),
                };
                match pick {
                    Some(p) => {
                        done.push(match p.as_str() {
                            "follow" => "the notch now opens on the screen you're using".to_string(),
                            "builtin" => "the notch stays on the built-in screen".to_string(),
                            n => format!("the notch stays on {n}"),
                        });
                        patch.insert("notch_screen".into(), json!(p));
                    }
                    None => fail("follow, builtin, or a connected screen's name".into()),
                }
            }
            "dictate_live" | "type_as_you_speak" | "live_dictation" | "live_typing" => match as_bool(v) {
                Some(b) => {
                    let mut ex = app.state::<AppState>().settings.lock().map(|s| s.experimental.clone()).unwrap_or_default();
                    ex.insert(crate::session::LIVE_TYPING.into(), b);
                    patch.insert("experimental".into(), json!(ex));
                    done.push(if b { "dictated words now appear as you speak".into() } else { "dictation types when you're done".to_string() });
                }
                None => fail("on or off".into()),
            },
            "show_typed_text" | "show_text" | "show_answers" => match as_bool(v) {
                Some(b) => {
                    patch.insert("show_typed_text".into(), json!(b));
                    done.push(format!("showing typed answers in the notch {}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            "speak_answers" | "speak" | "talk_back" => match as_bool(v) {
                Some(b) => {
                    patch.insert("speak_answers".into(), json!(b));
                    done.push(format!("speak answers {}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            "look_at_screen" | "share_screen" | "screen" => match as_bool(v) {
                Some(b) => {
                    patch.insert("share_screen".into(), json!(b));
                    done.push(format!("looking at the screen {}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            "can_click" | "click" | "click_and_type" => match as_bool(v) {
                Some(b) => {
                    patch.insert("can_click".into(), json!(b));
                    done.push(if b { "clicking and typing for them on".into() } else { "pointer only (no clicking)".into() });
                }
                None => fail("on or off".into()),
            },
            "auto_update" | "updates" => match as_bool(v) {
                Some(b) => {
                    patch.insert("auto_update".into(), json!(b));
                    done.push(format!("automatic updates {}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            "muted" | "mute" | "sounds_muted" => match as_bool(v) {
                Some(b) => {
                    patch.insert("muted".into(), json!(b));
                    done.push(if b { "sounds muted".into() } else { "sounds unmuted".into() });
                }
                None => fail("on or off".into()),
            },
            "sounds" => match as_bool(v) {
                Some(b) => {
                    patch.insert("muted".into(), json!(!b));
                    done.push(format!("sounds {}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            "buddy" | "buddy_mode" | "pointer" => {
                let m = match v {
                    Value::Bool(true) => Some("always".to_string()),
                    Value::Bool(false) => Some("off".to_string()),
                    _ => as_str(v).map(|s| s.to_lowercase()).map(|s| match s.as_str() {
                        "on" | "always on" => "always".to_string(),
                        "idle mode" | "only when busy" | "when busy" => "idle".to_string(),
                        _ => s,
                    }),
                };
                match m.as_deref() {
                    Some(m @ ("always" | "idle" | "off")) => {
                        patch.insert("buddy".into(), json!(true));
                        patch.insert("buddy_mode".into(), json!(m));
                        done.push(format!("buddy {m}"));
                    }
                    _ => fail("always, idle or off".into()),
                }
            }
            "name" | "user_name" | "their_name" => match as_str(v).filter(|s| s.chars().count() <= 60) {
                Some(n) => {
                    done.push(if n.is_empty() { "name cleared".into() } else { format!("calling them {n}") });
                    patch.insert("user_name".into(), json!(n));
                }
                None => fail("a short name".into()),
            },
            "voice" | "natural_voice" | "kokoro_voice" | "voice_engine" => {
                let s = as_str(v).unwrap_or_default().to_lowercase();
                if matches!(s.as_str(), "system" | "apple" | "mac" | "windows") {
                    patch.insert("voice_engine".into(), json!("apple"));
                    done.push("system voice".into());
                } else if let Some((id, name, _)) = crate::kokoro::VOICES.iter().find(|(id, n, _)| n.to_lowercase() == s || id.to_string() == s) {
                    if !crate::kokoro::installed(&data) {
                        fail("the natural voice isn't downloaded yet; open the voice page so they can get it".into());
                    } else {
                        patch.insert("voice_engine".into(), json!("kokoro"));
                        patch.insert("kokoro_voice".into(), json!(id));
                        done.push(format!("voice {name}"));
                    }
                } else if matches!(s.as_str(), "natural" | "kokoro") {
                    if crate::kokoro::installed(&data) {
                        patch.insert("voice_engine".into(), json!("kokoro"));
                        done.push("natural voice".into());
                    } else {
                        fail("the natural voice isn't downloaded yet; open the voice page so they can get it".into());
                    }
                } else {
                    fail(format!("one of {}, or system", crate::kokoro::VOICES.iter().map(|v| v.1).collect::<Vec<_>>().join(", ")));
                }
            }
            "voice_speed" | "speed" => match as_num(v) {
                Some(n) => {
                    let n = if n > 3.0 { n / 100.0 } else { n }.clamp(0.8, 1.3);
                    patch.insert("voice_speed".into(), json!((n * 100.0).round() / 100.0));
                    done.push(format!("natural voice speed {n:.2}×"));
                }
                None => fail("a number 0.8–1.3".into()),
            },
            "voice_loudness" | "loudness" | "voice_volume" | "speech_volume" => {
                let l = v.as_str().unwrap_or("").to_lowercase();
                let level = match l.as_str() {
                    "quiet" | "low" | "soft" | "quieter" | "softer" => Some("quiet"),
                    "normal" | "medium" | "default" => Some("normal"),
                    "loud" | "high" | "louder" | "max" => Some("loud"),
                    _ => None,
                };
                match level {
                    Some(l) => {
                        patch.insert("voice_loudness".into(), json!(l));
                        done.push(format!("voice loudness {l}"));
                    }
                    None => fail("quiet, normal or loud".into()),
                }
            }
            "speech_rate" | "system_voice_speed" => match as_num(v) {
                Some(n) => {
                    let n = n.clamp(0.4, 0.65);
                    patch.insert("speech_rate".into(), json!(n));
                    done.push(format!("system voice rate {n:.2}"));
                }
                None => fail("a number 0.4–0.65".into()),
            },
            "volume" | "master_volume" | "sound_volume" => match as_num(v) {
                Some(n) => {
                    let n = if n > 1.5 { n / 100.0 } else { n }.clamp(0.0, 1.5) as f32;
                    patch.insert("master_volume".into(), json!(n));
                    done.push(format!("sound volume {}%", (n * 100.0).round()));
                }
                None => fail("0–1.5 (1 = normal), or a percentage".into()),
            },
            "language" | "locale" | "listening_language" => {
                let s = as_str(v).unwrap_or_default();
                let s = if matches!(s.to_lowercase().as_str(), "auto" | "system" | "default") { String::new() } else { s.replace('_', "-") };
                let ok = s.is_empty() || (s.len() <= 10 && s.split('-').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric())));
                if ok {
                    done.push(if s.is_empty() { "listening language: automatic".into() } else { format!("listening language {s}") });
                    patch.insert("locale".into(), json!(s));
                } else {
                    fail("a locale like en-US, en-GB, de-DE, or auto".into());
                }
            }
            "talk_shortcut" | "shortcut_talk" | "dictate_shortcut" | "shortcut_dictate" => {
                let chord = as_str(v).unwrap_or_default().to_lowercase().replace(' ', "").replace("⌃", "ctrl").replace("⌥", "option").replace("⌘", "cmd").replace("⇧", "shift").replace("alt", "option").replace("control", "ctrl").replace("command", "cmd");
                let talk = key.contains("talk");
                let other = if talk { &cur.shortcut_dictate } else { &cur.shortcut_talk };
                if crate::hotkey::parse(&chord).is_none() {
                    fail("a chord of modifier keys like ctrl+option, fn+ctrl, cmd+shift".into());
                } else if &chord == other {
                    fail("that's already the other shortcut".into());
                } else {
                    patch.insert((if talk { "shortcut_talk" } else { "shortcut_dictate" }).into(), json!(chord));
                    done.push(format!("{} shortcut {chord}", if talk { "talk" } else { "dictate" }));
                }
            }
            "provider" => {
                let p = as_str(v).unwrap_or_default().to_lowercase();
                let p = match p.as_str() {
                    "anthropic" => "claude".to_string(),
                    "chatgpt" | "gpt" => "openai".to_string(),
                    "open code" | "open-code" | "local" | "free" => "opencode".to_string(),
                    _ => p,
                };
                if p == "opencode" {
                    if crate::opencode::binary().is_some() {
                        patch.insert("provider".into(), json!("opencode"));
                        done.push("using opencode on this computer (free models)".into());
                    } else {
                        fail("opencode isn't installed on this computer".into());
                    }
                } else if !matches!(p.as_str(), "claude" | "openai" | "deepseek") {
                    fail("claude, openai, deepseek or opencode".into());
                } else if crate::config::key_for(&data, &cur, &p).is_none() {
                    fail(format!("there's no {p} key yet; they add it in Settings → Models"));
                } else {
                    patch.insert("provider".into(), json!(p));
                    done.push(format!("using {p}"));
                }
            }
            "model" | "voice_model" | "agents_model" | "deep_model" | "jobs_model" => match as_str(v).filter(|s| !s.trim().is_empty()) {
                Some(m) => {
                    let provider = patch
                        .get("provider")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| (cur.provider == "opencode").then(|| "opencode".to_string()))
                        .or_else(|| crate::config::provider_config(&data, &cur).map(|c| c.kind.id().to_string()));
                    match provider {
                        Some(p) => match resolve_model(app, &p, &m).await {
                            Ok(id) => {
                                // Build on models already changed in this call (talk and agents at once).
                                let mut models: std::collections::BTreeMap<String, crate::config::ModelChoice> =
                                    patch.get("models").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_else(|| cur.models.clone());
                                let e = models.entry(p.clone()).or_default();
                                let deep = matches!(key.as_str(), "agents_model" | "deep_model" | "jobs_model");
                                if deep {
                                    e.deep = id.clone();
                                } else {
                                    e.fast = id.clone();
                                }
                                patch.insert("models".into(), serde_json::to_value(models).unwrap_or_default());
                                done.push(format!("{} model {id} ({p})", if deep { "agents" } else { "voice" }));
                            }
                            Err(e) => bad.push(format!("{key}: {e}")),
                        },
                        None => fail("no model provider is set up yet".into()),
                    }
                }
                None => fail("a model id".into()),
            },
            "backup_model" | "fallback" => {
                let s = as_str(v).unwrap_or_default();
                if matches!(s.to_lowercase().as_str(), "" | "none" | "off") {
                    patch.insert("fallback".into(), json!(""));
                    done.push("no backup model".into());
                } else if let Some((p, m)) = s.split_once(':').filter(|(p, m)| matches!(*p, "claude" | "openai" | "deepseek") && !m.is_empty()) {
                    patch.insert("fallback".into(), json!(format!("{p}:{m}")));
                    done.push(format!("backup model {m} ({p})"));
                } else {
                    fail("provider:model, e.g. claude:claude-haiku-5-5, or none".into());
                }
            }
            "coding_agent" => {
                let a = as_str(v).unwrap_or_default().to_lowercase();
                let a = match a.as_str() {
                    "claude code" | "claude_code" => "claude".to_string(),
                    "codex cli" => "codex".to_string(),
                    _ => a,
                };
                if matches!(a.as_str(), "auto" | "codex" | "claude" | "opencode") {
                    let mut c = cur.coding.clone();
                    c.agent = a.clone();
                    patch.insert("coding".into(), serde_json::to_value(c).unwrap_or_default());
                    done.push(format!("coding agent {a}"));
                } else {
                    fail("auto, codex, claude or opencode".into());
                }
            }
            "coding_folder" => match as_str(v) {
                Some(d) => {
                    let mut c = patch.get("coding").and_then(|c| serde_json::from_value::<openclicky_tools::coders::Prefs>(c.clone()).ok()).unwrap_or_else(|| cur.coding.clone());
                    c.dir = d.clone();
                    patch.insert("coding".into(), serde_json::to_value(c).unwrap_or_default());
                    done.push(format!("coding folder {d}"));
                }
                None => fail("a folder path".into()),
            },
            "memory" | "long_term_memory" => {
                let m = match v {
                    Value::Bool(true) => Some("auto".to_string()),
                    Value::Bool(false) => Some("off".to_string()),
                    _ => as_str(v).map(|s| s.to_lowercase()).map(|s| match s.as_str() {
                        "on" | "automatic" | "learn" => "auto".to_string(),
                        "only when asked" | "ask" | "asked" => "manual".to_string(),
                        "pause" | "paused" | "disabled" => "off".to_string(),
                        _ => s,
                    }),
                };
                match m.as_deref() {
                    Some(m @ ("auto" | "manual" | "off")) => {
                        patch.insert("memory".into(), json!(m));
                        done.push(match m {
                            "auto" => "memory on (also learns from chats)".to_string(),
                            "manual" => "memory only when they say remember".to_string(),
                            _ => "memory off".to_string(),
                        });
                    }
                    _ => fail("auto, manual or off".into()),
                }
            }
            "new_chat_at" | "chat_reset" | "chat_reset_hour" | "new_day_at" => match hour_of(v) {
                Some(h) => {
                    patch.insert("chat_reset_hour".into(), json!(h));
                    done.push(format!("a new chat starts each day at {h:02}:00"));
                }
                None => fail("an hour like 0 (midnight), 4am or 06:00".into()),
            },
            "drawing" | "canvas" | "canvas_use" | "draw" => {
                let level = match v {
                    Value::Bool(true) => Some("helps"),
                    Value::Bool(false) => Some("asked"),
                    _ => match as_str(v).unwrap_or_default().to_lowercase().as_str() {
                        "asked" | "only when asked" | "when asked" | "less" | "never" | "off" | "rarely" => Some("asked"),
                        "helps" | "when it helps" | "normal" | "default" | "on" | "sometimes" => Some("helps"),
                        "often" | "more" | "always" | "a lot" | "lots" => Some("often"),
                        _ => None,
                    },
                };
                match level {
                    Some(l) => {
                        patch.insert("canvas_use".into(), json!(l));
                        done.push(match l {
                            "asked" => "drawing on the canvas only when they ask".to_string(),
                            "often" => "drawing on the canvas often".to_string(),
                            _ => "drawing on the canvas when it helps".to_string(),
                        });
                    }
                    None => fail("asked, helps or often".into()),
                }
            }
            "persona" | "personality" | "style" | "character" => {
                let p = as_str(v).unwrap_or_default().to_lowercase();
                let p = match p.as_str() {
                    "pix" | "pixie" | "default" | "normal" | "friendly" | "kind" | "warm" => Some("pix"),
                    "inos" | "nerdy" | "nerd" | "smart" | "geeky" | "curious" => Some("inos"),
                    "xtrem" | "extreme" | "chaotic" | "funny" | "fun" | "unhinged" | "witty" => Some("xtrem"),
                    "eon" | "concise" | "short" | "brief" | "calm" | "terse" => Some("eon"),
                    _ => None,
                };
                match p {
                    Some(p) => {
                        patch.insert("persona".into(), json!(p));
                        done.push(format!("personality {}", persona_name(p)));
                    }
                    None => fail("Pix, Inos, Xtrem or Eon".into()),
                }
            }
            "spicy" | "swearing" | "spicy_mode" => match as_bool(v) {
                Some(b) => {
                    patch.insert("spicy".into(), json!(b));
                    let note = if cur.persona == "xtrem" || patch.get("persona").and_then(Value::as_str) == Some("xtrem") { "" } else { " (it only applies to Xtrem)" };
                    done.push(format!("spicy {}{note}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            "calendar" | "reminders" | "contacts" | "mail" => match as_bool(v) {
                Some(b) if cfg!(target_os = "macos") => later.push((key.clone(), b)),
                Some(_) => fail("Mac only; on Windows and Linux use the Google account".into()),
                None => fail("on or off".into()),
            },
            "connectors" | "connectors_enabled" | "experimental_connectors" => match as_bool(v) {
                Some(b) => {
                    let mut ex = app.state::<AppState>().settings.lock().map(|s| s.experimental.clone()).unwrap_or_default();
                    ex.insert(crate::connectors::FLAG.into(), b);
                    patch.insert("experimental".into(), json!(ex));
                    done.push(format!("connectors {}", on_off(b)));
                }
                None => fail("on or off".into()),
            },
            k if k.starts_with("connector:") || k.starts_with("connector_") => match as_bool(v) {
                Some(b) => connectors.push((raw.trim()[10..].to_string(), b)),
                None => fail("on or off".into()),
            },
            k if k.contains("key") || k.contains("secret") || k.contains("token") || k.contains("password") => {
                fail("keys and secrets are typed in by them in Settings; open the page for them".into())
            }
            _ => fail("not a setting I can change; action=about lists them".into()),
        }
    }

    if !patch.is_empty() {
        commands::settings_set(app.clone(), Value::Object(patch))?;
    }
    for (kind, on) in later {
        match crate::integrations::set(app, &kind, on).await {
            Ok(_) => done.push(format!("{kind} {}", on_off(on))),
            Err(e) => bad.push(format!("{kind}: {e}")),
        }
    }
    if !connectors.is_empty() {
        let g = crate::connectors::gallery(app);
        let all: Vec<Value> = g["presets"].as_array().into_iter().flatten().chain(g["custom"].as_array().into_iter().flatten()).cloned().collect();
        for (id, on) in connectors {
            let found = all.iter().find(|c| c["id"].as_str().map(|i| i.eq_ignore_ascii_case(&id)).unwrap_or(false) || c["name"].as_str().map(|n| n.eq_ignore_ascii_case(&id)).unwrap_or(false));
            match found {
                Some(c) if c["added"].as_bool().unwrap_or(false) => {
                    let cid = c["id"].as_str().unwrap_or(&id).to_string();
                    match crate::connectors::save(app, &cid, &json!({}), on) {
                        Ok(()) => done.push(format!("{} connector {}", c["name"].as_str().unwrap_or(&cid), on_off(on))),
                        Err(e) => bad.push(format!("connector {id}: {e}")),
                    }
                }
                Some(c) => bad.push(format!("{} isn't added yet; they add it in Settings → Connectors", c["name"].as_str().unwrap_or(&id))),
                None => bad.push(format!("no connector \"{id}\"")),
            }
        }
    }
    crate::emit(app, openclicky_core::Event::new("settings.external", json!({})));

    let mut out = String::new();
    if !done.is_empty() {
        out.push_str(&format!("Done: {}.", done.join("; ")));
    }
    if !bad.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!("Not changed: {}.", bad.join("; ")));
    }
    if done.is_empty() {
        Err(out)
    } else {
        Ok(out)
    }
}

fn describe_update(s: &Value) -> String {
    let cur = s["current"].as_str().unwrap_or("");
    match s["phase"].as_str().unwrap_or("") {
        "ready" => format!("Pixie {} is downloaded and waiting; it switches over at the next restart (or restart_to_update).", s["version"].as_str().unwrap_or("")),
        "downloading" => format!("Downloading Pixie {}.", s["version"].as_str().unwrap_or("")),
        "checking" => "Checking GitHub now.".into(),
        "up_to_date" => format!("Pixie {cur} is the newest version."),
        "error" => format!("Couldn't check: {}", s["error"].as_str().unwrap_or("unknown error")),
        _ if s["supported"] == json!(false) => format!("This is a development build ({cur}); it doesn't update itself."),
        _ => format!("Pixie {cur}; not checked yet this run."),
    }
}

/// Everything about Pixie, now: what it is, what it can do, and how it's set up.
async fn about(app: &AppHandle) -> String {
    let st = app.state::<AppState>();
    let s = st.settings.lock().map(|s| s.clone()).unwrap_or_default();
    let data = st.data_dir.clone();
    let mac = cfg!(target_os = "macos");
    let version = app.package_info().version.to_string();
    let platform = if mac { "macOS" } else if cfg!(windows) { "Windows (preview)" } else { "Linux (preview)" };
    let model = st.provider().map(|p| p.label()).unwrap_or_else(|| "none (no API key yet)".into());
    let voice = if s.voice_engine == "apple" || !crate::kokoro::installed(&data) {
        format!("system voice (rate {:.2}){}", s.speech_rate, if crate::kokoro::installed(&data) { "" } else { "; the natural voice isn't downloaded" })
    } else {
        let name = crate::kokoro::VOICES.iter().find(|v| v.0 == s.kokoro_voice).map(|v| v.1).unwrap_or("Heart");
        format!("natural voice {name}, speed {:.2}×, loudness {}", s.voice_speed, s.voice_loudness)
    };
    let perms = crate::permissions::permissions_status(app.clone())
        .await
        .into_iter()
        .filter_map(|p| serde_json::to_value(p).ok())
        .map(|p| format!("{} {}", p["label"].as_str().or(p["id"].as_str()).unwrap_or(""), p["state"].as_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join(", ");
    let apps = ["calendar", "reminders", "contacts", "mail"]
        .iter()
        .map(|k| format!("{k} {}", on_off(s.apps.get(*k).copied().unwrap_or(false))))
        .collect::<Vec<_>>()
        .join(", ");
    let g = crate::connectors::gallery(app);
    let conns = g["presets"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(g["custom"].as_array().into_iter().flatten())
        .filter(|c| c["added"].as_bool().unwrap_or(false))
        .map(|c| format!("{} ({}) {}", c["name"].as_str().unwrap_or(""), c["id"].as_str().unwrap_or(""), on_off(c["on"].as_bool().unwrap_or(false))))
        .collect::<Vec<_>>();
    let google = crate::google::status(app);
    let usage = {
        let lines = crate::usage::read(&data);
        let now = chrono::Utc::now().timestamp_millis();
        let sum = crate::usage::summary_with(&lines, &s, &crate::usage::Filter { days: 30, ..Default::default() }, now);
        format!("last 30 days ${:.2} over {} requests; all time ${:.2}", sum["cost"].as_f64().unwrap_or(0.0), sum["requests"].as_u64().unwrap_or(0), sum["all_time"]["cost"].as_f64().unwrap_or(0.0))
    };
    let jobs = st.jobs.list();
    let running = jobs.iter().filter(|j| matches!(j.state, openclicky_core::model::JobState::Running | openclicky_core::model::JobState::WaitingOnUser)).count();
    let talk = &s.shortcut_talk;
    let dictate = &s.shortcut_dictate;
    let fallback = if s.fallback.is_empty() { "none".to_string() } else { s.fallback.clone() };
    let coding = format!("{} (folder {})", if s.coding.agent.is_empty() { "auto" } else { &s.coding.agent }, if s.coding.dir.is_empty() { "not set" } else { &s.coding.dir });
    let locale = if s.locale.is_empty() { "automatic".to_string() } else { s.locale.clone() };
    format!(
        "PIXIE {version} on {platform}. Open source (github.com/s41r4j/pixie), runs on their computer with their own API keys; no account.\n\
\n\
WHAT YOU DO\n\
- Live in the notch at the top of the screen. Hold {talk} to talk (answers are spoken and shown in the notch); {dictate} dictates text into any app. Esc stops you. Clicking the menu-bar icon or Type a Question… lets them type. Hovering the notch shows the answer, the Chat tab (history) and the Agents tab (jobs).\n\
- See their screen when they ask (screenshot, text on screen, the buttons in the front window), point at things with the buddy pointer, circle/box/underline them, and click, type, press keys and use menus for them.\n\
- Explain with drawings on the Excalidraw canvas, in its own window they can edit and keep (open, draw shapes or Mermaid, read back, clear, delete; export PNG, SVG, .excalidraw). Show Canvas is in the menu bar menu.\n\
- Run background jobs (research, documents, files, code) with a plan, a note and a report; live agents that do several steps on their screen while explaining; coding tasks through Codex, Claude Code or opencode.\n\
- Web search and pages, files on their computer (never overwriting, private paths blocked), clipboard, PDFs; Calendar, Reminders, Contacts and Mail drafts (Mac), Gmail drafts and Google Calendar, MCP connectors, browser control through the extension. You never send mail; you draft.\n\
- Remember them across days: short notes about who they are, their preferences, people, projects and setup (memory tool; learned from chats when memory is on auto). Quit, stop all jobs or start a new chat when asked. Update yourself from GitHub when idle.\n\
\n\
HOW YOU'RE SET UP NOW\n\
- Model: {model}; backup {fallback}. Providers: Claude, OpenAI, DeepSeek (keys in Settings → Models).\n\
- Voice: {voice}; speak answers {speak}; typed questions answered {typed}; listening language {locale}.\n\
- Buddy pointer: {buddy}; click and type for them: {click}; look at the screen when asked: {screen}.\n\
- Sounds: volume {vol}%, {muted}.\n\
- Calling them: {name}.\n\
- Coding agent: {coding}.\n\
- Apps: {apps}. Google: {google}.\n\
- Connectors: {conns}.\n\
- Memory: {memory_mode}; {notes} note(s) about them (Settings → Memory). Today's chat carries on all day and a new one starts at {reset:02}:00.\n\
- Personality: {persona}{spicy}. Drawing on the canvas: {drawing}.\n\
- Updates: automatic {auto}; {update}.\n\
- Permissions: {perms}.\n\
- Jobs: {njobs} in the list, {running} running. Usage: {usage}.\n\
- Settings pages: General, Voice, Sounds, Models, Usage, Coding, Permissions, Apps, Connectors, Experimental, About. Files: {data}.\n\
\n\
Change any of it with action=set; API keys, permissions and sign-ins are done by them on the page (action=open).",
        speak = on_off(s.speak_answers),
        typed = if s.typed_answers == "text" { format!("in text ({} in the notch)", if s.show_typed_text { "shown" } else { "not shown" }) } else { "out loud".to_string() },
        buddy = if !s.buddy { "off" } else { s.buddy_mode.as_str() },
        click = on_off(s.can_click),
        screen = on_off(s.share_screen),
        vol = (s.master_volume * 100.0).round(),
        muted = if s.muted { "muted" } else { "not muted" },
        name = if s.user_name.is_empty() { "no name set" } else { &s.user_name },
        google = if google["signed_in"].as_bool().unwrap_or(false) { "signed in" } else { "not signed in" },
        conns = if !s.experiment(crate::connectors::FLAG) {
            "switched off (Settings → Experimental → Connectors; set connectors=true to show and run them)".to_string()
        } else if conns.is_empty() {
            "on, none added".to_string()
        } else {
            conns.join(", ")
        },
        persona = persona_name(&s.persona),
        spicy = if s.persona == "xtrem" { if s.spicy { ", spicy" } else { ", clean" } } else { "" },
        drawing = match s.canvas_use.as_str() { "asked" => "only when asked", "often" => "often", _ => "when it helps" },
        memory_mode = match crate::memory::mode(app).as_str() { "auto" => "on, learning from chats", "manual" => "only when they say remember", _ => "off" },
        notes = crate::memory::load(&data).len(),
        reset = s.chat_reset_hour,
        auto = on_off(s.auto_update),
        update = describe_update(&crate::updates::status(app)),
        njobs = jobs.len(),
        data = data.display(),
    )
}

// ---- quit / stop ----

fn running_jobs(app: &AppHandle) -> usize {
    use openclicky_core::model::JobState;
    app.state::<AppState>().jobs.list().iter().filter(|j| matches!(j.state, JobState::Running | JobState::WaitingOnUser)).count()
}

/// Stop every running job and live agent; how many there were.
fn stop_jobs(app: &AppHandle) -> usize {
    use openclicky_core::model::JobState;
    let ids: Vec<String> = app
        .state::<AppState>()
        .jobs
        .list()
        .into_iter()
        .filter(|j| matches!(j.state, JobState::Running | JobState::WaitingOnUser))
        .map(|j| j.id)
        .collect();
    for id in &ids {
        crate::jobs::cancel(app, id);
    }
    crate::jobs::cancel_live(app);
    ids.len()
}

/// Quit after the goodbye has been said (or at once if nothing is speaking).
fn quit_soon(app: &AppHandle, line: Option<&str>) {
    if let Some(l) = line {
        let speak = app.state::<AppState>().settings.lock().map(|s| s.speak_answers).unwrap_or(true);
        crate::jobs::toast(app, l, "info", None);
        if speak {
            crate::speech::speak(app, "bye", l);
        }
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while app.state::<AppState>().session.busy() && std::time::Instant::now() < until {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        log::info!("quitting (asked to)");
        app.exit(0);
    });
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Quick {
    /// Switch personality.
    Persona(&'static str),
    /// Quit the app.
    Quit,
    /// Stop every job and live agent too.
    StopAll,
    /// Just stop talking / drop it.
    Hush,
    /// Start a new conversation.
    NewChat,
}

/// Whole-utterance commands that need no model: "quit", "exit Pixie", "stop working",
/// "never mind". Anything longer or different goes to the model as usual.
pub fn quick(text: &str) -> Option<Quick> {
    let clean: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '\'' { c } else { ' ' }).collect();
    if let Some(p) = persona_command(&clean) {
        return Some(Quick::Persona(p));
    }
    let filler = ["hey", "ok", "okay", "please", "pixie", "pixi", "now", "just", "can", "you", "could", "would", "for", "me", "thanks", "thank", "the", "app", "yourself", "right", "away"];
    let words: Vec<&str> = clean.split_whitespace().filter(|w| !filler.contains(w)).collect();
    let said_pixie = clean.split_whitespace().any(|w| w == "pixie" || w == "pixi" || w == "yourself" || w == "app");
    let core = words.join(" ");
    if clean.split_whitespace().count() > 8 {
        return None;
    }
    match core.as_str() {
        "quit" | "exit" | "quit out" | "shut down" | "shutdown" => Some(Quick::Quit),
        "close" | "turn off" | "switch off" | "go away" | "bye" | "goodbye" if said_pixie => Some(Quick::Quit),
        "stop working" | "stop everything" | "stop all" | "stop all jobs" | "stop all agents" | "stop all tasks" | "cancel everything" | "cancel all" | "cancel all jobs" | "stop jobs" | "stop agents" => Some(Quick::StopAll),
        "new chat" | "start new chat" | "start a new chat" | "a new chat" | "start over" | "fresh start" | "clear chat" => Some(Quick::NewChat),
        "stop" | "stop talking" | "stop it" | "never mind" | "nevermind" | "be quiet" | "quiet" | "shut up" | "cancel" | "shh" => Some(Quick::Hush),
        _ => None,
    }
}

/// A persona named in speech: the names (and how speech recognition tends to hear them), or a
/// description followed by "one", "mode" or "personality" ("the calm one").
fn persona_word(words: &[&str], i: usize) -> Option<&'static str> {
    let w = words[i];
    let next = words.get(i + 1).copied().unwrap_or("");
    let prev = if i > 0 { words[i - 1] } else { "" };
    let by_name = match w {
        "pix" | "pics" | "picks" | "pixs" => Some("pix"),
        "inos" | "eenos" | "inose" | "ino's" | "enos" | "eno's" | "eanos" | "inos's" | "ee-nos" | "inous" | "eno" => Some("inos"),
        "xtrem" | "extreme" | "xtreme" | "extrem" | "x-trem" => Some("xtrem"),
        "eon" | "eeon" | "aeon" | "ion" | "eon's" | "eons" => Some("eon"),
        // "Pixie" is how they address it; it names the default only as "back to Pixie" / "be Pixie".
        "pixie" | "pixi" if matches!(prev, "to" | "be" | "become" | "normal" | "regular" | "usual" | "old" | "original") && i + 1 == words.len() => Some("pix"),
        _ => None,
    };
    if by_name.is_some() {
        return by_name;
    }
    if !matches!(next, "one" | "mode" | "personality" | "persona" | "version" | "self" | "pixie" | "you") && !matches!(prev, "to" | "be") {
        return None;
    }
    match w {
        "normal" | "default" | "regular" | "usual" | "friendly" | "original" | "old" | "warm" => Some("pix"),
        "nerdy" | "nerd" | "geeky" | "geek" | "smart" | "intelligent" | "curious" => Some("inos"),
        "chaotic" | "funny" | "witty" | "crazy" | "wild" | "mischievous" | "spicy" => Some("xtrem"),
        "calm" | "concise" | "brief" | "short" | "precise" | "minimal" | "quiet" => Some("eon"),
        _ => None,
    }
}

const SWITCH: &[&str] = &["switch", "change", "be", "become", "use", "go", "turn", "talk", "sound", "act", "set", "make", "want", "bring", "put"];

/// The whole request is a persona switch: "switch to Eon", "switch back to Pix", "be Xtrem",
/// "Inos mode", "talk like the nerdy one", "go back to normal Pixie".
fn persona_command(clean: &str) -> Option<&'static str> {
    let words: Vec<&str> = clean.split_whitespace().collect();
    const FILL: &[&str] = &[
        "hey", "hi", "ok", "okay", "please", "now", "back", "to", "into", "the", "can", "you", "could", "would", "your", "personality", "persona", "mode", "style",
        "as", "and", "just", "again", "pixie", "pixi", "like", "one", "version", "self", "yourself", "i", "i'd", "let's", "lets", "me", "my", "it", "a", "bit", "more", "of",
        "with", "on", "from", "for", "time", "instead", "voice", "be", "switch",
    ];
    if words.len() > 12 || words.first().is_some_and(|w| matches!(*w, "what" | "who" | "why" | "how" | "does" | "is" | "tell" | "explain" | "which")) {
        return None;
    }
    let target = (0..words.len()).rev().find_map(|i| persona_word(&words, i))?;
    let has_verb = words.iter().any(|w| SWITCH.contains(w));
    let mode_word = words.last().is_some_and(|w| matches!(*w, "mode" | "personality" | "persona"));
    let rest_ok = words.iter().enumerate().all(|(i, w)| SWITCH.contains(w) || FILL.contains(w) || persona_word(&words, i).is_some() || DESCRIBE.contains(w));
    (rest_ok && (has_verb || mode_word)).then_some(target)
}

const DESCRIBE: &[&str] = &[
    "normal", "default", "regular", "usual", "friendly", "original", "old", "warm", "nerdy", "nerd", "geeky", "geek", "smart", "intelligent", "curious", "chaotic", "funny", "witty", "crazy",
    "wild", "mischievous", "spicy", "calm", "concise", "brief", "short", "precise", "minimal", "quiet",
];

/// A persona switch asked for inside a longer request ("ok switch back to Pix and tell me a
/// joke"): a switch word a few words before the persona. Used after the model's turn, so the
/// setting really changes even when the model only said it did.
pub fn persona_intent(text: &str) -> Option<&'static str> {
    let clean: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '\'' || c == '-' { c } else { ' ' }).collect();
    let words: Vec<&str> = clean.split_whitespace().collect();
    if words.first().is_some_and(|w| matches!(*w, "what" | "who" | "why" | "how" | "does" | "is" | "which")) {
        return None;
    }
    (0..words.len()).find_map(|i| {
        let p = persona_word(&words, i)?;
        let from = i.saturating_sub(5);
        let before = &words[from..i];
        let asked = before.iter().any(|w| matches!(*w, "switch" | "change" | "become" | "personality" | "persona"))
            || before.windows(2).any(|w| matches!(w[0], "talk" | "sound" | "act") && w[1] == "like")
            || i > 0 && words[i - 1] == "be";
        asked.then_some(p)
    })
}

/// Switch personality now (Settings, the notch face and every open window follow).
pub fn set_persona(app: &AppHandle, p: &str) -> Result<(), String> {
    commands::settings_set(app.clone(), json!({ "persona": p }))?;
    crate::emit(app, openclicky_core::Event::new("settings.external", json!({})));
    Ok(())
}

/// After a turn: they asked for a personality in a longer request and the model didn't switch
/// (or only said it did). Make it true.
pub fn ensure_persona(app: &AppHandle, asked: &str) {
    let Some(p) = persona_intent(asked) else { return };
    let cur = app.state::<AppState>().settings.lock().map(|s| s.persona.clone()).unwrap_or_default();
    let cur = if cur.is_empty() { "pix".to_string() } else { cur };
    if cur == p {
        return;
    }
    log::info!("persona: asked for {p} but it's still {cur}; switching");
    if set_persona(app, p).is_ok() {
        crate::jobs::toast(app, &format!("Personality: {}", persona_name(p)), "info", None);
    }
}

pub fn run_quick(app: &AppHandle, q: Quick) {
    log::info!("quick command: {q:?}");
    match q {
        Quick::Quit => quit_soon(app, Some("Bye for now.")),
        Quick::StopAll => {
            let n = stop_jobs(app);
            crate::session::escape(app);
            let text = if n == 0 { "Nothing was running.".to_string() } else { format!("Stopped {n} job{}.", if n == 1 { "" } else { "s" }) };
            crate::jobs::toast(app, &text, "info", None);
        }
        Quick::Hush => crate::session::escape(app),
        Quick::Persona(p) => {
            if let Err(e) = set_persona(app, p) {
                crate::jobs::toast(app, &e, "error", None);
                return;
            }
            let line = match p {
                "inos" => "Inos here. Ask me anything.",
                "xtrem" => "Xtrem's in the house.",
                "eon" => "Eon.",
                _ => "Pix again, hi!",
            };
            crate::jobs::toast(app, line, "info", None);
            if app.state::<AppState>().settings.lock().map(|s| s.speak_answers).unwrap_or(true) {
                crate::speech::speak(app, "persona", line);
            }
            crate::session::schedule_collapse(app, std::time::Duration::from_secs(4));
        }
        Quick::NewChat => {
            new_chat(app);
            crate::session::escape(app);
            crate::jobs::toast(app, "New chat. The earlier one is saved in Chats.", "info", None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_commands() {
        assert_eq!(quick("Quit."), Some(Quick::Quit));
        assert_eq!(quick("Hey Pixie, exit please"), Some(Quick::Quit));
        assert_eq!(quick("close Pixie"), Some(Quick::Quit));
        assert_eq!(quick("turn yourself off"), Some(Quick::Quit));
        assert_eq!(quick("close"), None); // could be a window
        assert_eq!(quick("Stop working"), Some(Quick::StopAll));
        assert_eq!(quick("never mind"), Some(Quick::Hush));
        assert_eq!(quick("how do I exit vim?"), None);
        assert_eq!(quick("stop the music in Spotify"), None);
        assert_eq!(quick("quit Safari"), None);
        assert_eq!(quick("Start a new chat"), Some(Quick::NewChat));
        assert_eq!(quick("Switch to Eon."), Some(Quick::Persona("eon")));
        assert_eq!(quick("Switch back to Pix."), Some(Quick::Persona("pix")));
        assert_eq!(quick("Hey Pixie, be Xtrem"), Some(Quick::Persona("xtrem")));
        assert_eq!(quick("Inos mode"), Some(Quick::Persona("inos")));
        assert_eq!(quick("switch back to pixie"), Some(Quick::Persona("pix")));
        assert_eq!(quick("what does eon mean"), None);
        assert_eq!(quick("use the eon theme in VS Code"), None);
        assert_eq!(quick("talk like the nerdy one"), Some(Quick::Persona("inos")));
        assert_eq!(quick("go back to normal pixie"), Some(Quick::Persona("pix")));
        assert_eq!(quick("can you be the calm one please"), Some(Quick::Persona("eon")));
        assert_eq!(quick("switch to extreme mode"), Some(Quick::Persona("xtrem")));
        assert_eq!(quick("Switch to Eno's."), Some(Quick::Persona("inos")));
        assert_eq!(quick("hey pixie what's the weather"), None);
        assert_eq!(quick("who is inos"), None);
        assert_eq!(persona_intent("ok switch back to Pix and tell me a joke"), Some("pix"));
        assert_eq!(persona_intent("change your personality to Eon, then open Safari"), Some("eon"));
        assert_eq!(persona_intent("open the extreme sports video"), None);
        assert_eq!(persona_intent("put on the extreme sports video"), None);
        assert_eq!(persona_intent("what does eon mean"), None);
        assert_eq!(hour_of(&json!("midnight")), Some(0));
        assert_eq!(hour_of(&json!("4am")), Some(4));
        assert_eq!(hour_of(&json!("6 pm")), Some(18));
        assert_eq!(hour_of(&json!("06:00")), Some(6));
        assert_eq!(hour_of(&json!(25)), None);
    }

    #[test]
    fn model_names_resolve_to_real_ids() {
        let list = vec![("deepseek-flash".to_string(), "DeepSeek-V4.1-Flash".to_string()), ("deepseek-v4-pro".to_string(), "DeepSeek V4 Pro".to_string())];
        assert_eq!(pick_model(&list, "deepseek-flash").as_deref(), Some("deepseek-flash"));
        assert_eq!(pick_model(&list, "deepseek v4.1 flash").as_deref(), Some("deepseek-flash"));
        assert_eq!(pick_model(&list, "DeepSeek V4 Pro").as_deref(), Some("deepseek-v4-pro"));
        assert_eq!(pick_model(&list, "pro").as_deref(), Some("deepseek-v4-pro"));
        // An opencode id isn't a DeepSeek model.
        assert_eq!(pick_model(&list, "opencode/deepseek-v4.1-flash-free"), None);
    }

    #[test]
    fn spoken_values_parse() {
        assert_eq!(as_bool(&json!("off")), Some(false));
        assert_eq!(as_bool(&json!("On")), Some(true));
        assert_eq!(as_bool(&json!(true)), Some(true));
        assert_eq!(as_bool(&json!("maybe")), None);
        assert_eq!(as_num(&json!("80%")), Some(80.0));
        assert_eq!(as_num(&json!("1.1x")), Some(1.1));
        assert_eq!(as_num(&json!(0.9)), Some(0.9));
    }

    #[test]
    fn every_page_has_a_tab() {
        for page in ["general", "voice", "sounds", "models", "usage", "coding", "permissions", "apps", "connectors", "experimental", "about"] {
            assert!(TABS.iter().any(|(k, _)| *k == page), "{page}");
        }
    }
}

#[cfg(test)]
mod tool_sizes {
    #[test]
    #[ignore]
    fn print_voice_tool_sizes() {
        let mut tools = openclicky_tools::defs::for_mode(openclicky_core::prompt::Mode::Voice, true);
        tools.push(super::tool());
        tools.push(crate::memory::tool());
        tools.push(crate::canvas::tool());
        let mut total = 0;
        for t in &tools {
            let n = serde_json::to_string(t).unwrap().len();
            total += n;
            println!("{:>6} {}", n, t.name);
        }
        println!("{total:>6} total");
    }
}


/// Problems Pixie worked around lately, for "what went wrong?" and before a report.
fn issues_report(app: &AppHandle) -> String {
    let st = app.state::<AppState>();
    let v = crate::heal::recent(&st.data_dir, 8);
    if v.is_empty() {
        return "No problems noted lately.".into();
    }
    v.iter()
        .rev()
        .map(|i| format!("{} · {}{}: {}. Workaround: {}", i.when, i.kind, if i.times > 1 { format!(" ({}x)", i.times) } else { String::new() }, i.what, i.workaround))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Open a pre-filled GitHub issue in their browser. They post it, not Pixie.
pub fn open_report(app: &AppHandle, what: &str, browser: &str) -> Result<String, String> {
    let st = app.state::<AppState>();
    let (title, body) = crate::heal::draft(&st.data_dir, &st.os_version, what);
    let url = crate::heal::issue_url(&title, &body);
    log::info!("heal: report drafted ({} chars), opening in {}", url.len(), if browser.is_empty() { "default" } else { browser });
    crate::heal::open_in(browser, &url)
}

/// "Check yourself": each part, fixing what can be fixed on the spot.
pub async fn checkup(app: &AppHandle) -> Vec<crate::heal::Check> {
    use crate::heal::Check;
    let st = app.state::<AppState>();
    let mut out = vec![];
    // The helper: screen, speech, clicks (on Windows and Linux they're built in).
    if cfg!(target_os = "macos") {
        let ok = st.helper.call("cursor", json!({}), Duration::from_secs(2)).await.is_ok();
        if ok {
            out.push(Check { name: "Screen and speech helper", ok: true, detail: "answering".into() });
        } else {
            crate::helper::force_restart(app);
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let again = st.helper.call("cursor", json!({}), Duration::from_secs(3)).await.is_ok();
            crate::heal::note("helper", "The helper didn't answer a checkup", if again { "Restarted it" } else { "Restarted it; still not answering" }, "");
            out.push(Check { name: "Screen and speech helper", ok: again, detail: if again { "wasn't answering; restarted it".into() } else { "not answering even after a restart; quit and reopen Pixie".into() } });
        }
    }
    // Permissions.
    let perms = crate::permissions::permissions_status(app.clone()).await;
    let missing: Vec<String> = perms.iter().filter(|p| p.required && p.state != crate::permissions::PermState::Granted).map(|p| p.label.to_string()).collect();
    out.push(Check {
        name: "Permissions",
        ok: missing.is_empty(),
        detail: if missing.is_empty() { "all allowed".into() } else { format!("{} not allowed (Settings → Permissions)", missing.join(", ")) },
    });
    // One tiny model call.
    match st.provider() {
        None => out.push(Check { name: "Model", ok: false, detail: "no model set up (Settings → Models)".into() }),
        Some(p) => {
            let mut req = openclicky_llm::Request::new("Reply with the single word ok.", vec![openclicky_llm::Message::user_text("ok?")]);
            req.max_tokens = 5;
            req.effort = openclicky_llm::Effort::Fast;
            let started = std::time::Instant::now();
            let r = tokio::time::timeout(Duration::from_secs(25), p.complete(&req, &mut |_| {})).await;
            let label = p.label();
            out.push(match r {
                Ok(Ok(_)) => Check { name: "Model", ok: true, detail: format!("{label} answered in {:.1} s", started.elapsed().as_secs_f64()) },
                Ok(Err(e)) => {
                    let why = crate::heal::classify(&e.to_string()).map(|(w, f)| format!("{w}. {f}")).unwrap_or_else(|| e.to_string().chars().take(160).collect());
                    Check { name: "Model", ok: false, detail: format!("{label}: {why}") }
                }
                Err(_) => Check { name: "Model", ok: false, detail: format!("{label} didn't answer in 25 s") },
            });
        }
    }
    // The voice.
    let s = st.settings.lock().map(|s| s.clone()).unwrap_or_default();
    let v = crate::kokoro::status(app);
    out.push(if s.voice_engine == "apple" || !v.supported {
        Check { name: "Voice", ok: true, detail: "system voice".into() }
    } else if v.ready {
        Check { name: "Voice", ok: true, detail: "natural voice installed".into() }
    } else {
        Check { name: "Voice", ok: true, detail: "natural voice not installed, using the system voice (Settings → Voice)".into() }
    });
    // Connectors.
    if crate::connectors::enabled(app) {
        let n = crate::connectors::connected_names().len();
        out.push(Check { name: "Connectors", ok: true, detail: format!("{n} connected") });
    }
    out.push(crate::heal::disk_check(&st.data_dir));
    // What went wrong lately.
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let recent: Vec<_> = crate::heal::recent(&st.data_dir, 20).into_iter().filter(|i| i.when.starts_with(&today)).collect();
    out.push(Check {
        name: "Problems today",
        ok: recent.is_empty(),
        detail: if recent.is_empty() { "none".into() } else { recent.iter().map(|i| i.what.clone()).collect::<Vec<_>>().join("; ") },
    });
    log::info!("heal: checkup done, {} problem(s)", out.iter().filter(|c| !c.ok).count());
    out
}

#[cfg(test)]
mod successor_tests {
    use super::successor;

    #[test]
    fn retired_models_go_to_the_newest_of_their_family() {
        let ids: Vec<String> = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-sonnet-5", "claude-sonnet-5-5-20260801", "claude-haiku-5-5", "deepseek-chat", "deepseek-flash"].map(String::from).to_vec();
        assert_eq!(successor("claude-sonnet-4-5-20250929", &ids).as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(successor("claude-3-5-haiku-20241022", &ids).as_deref(), Some("claude-haiku-5-5"));
        assert_eq!(successor("deepseek-chat-v2", &ids).as_deref(), Some("deepseek-chat"));
        assert_eq!(successor("gpt-4o", &ids), None);
    }
}
