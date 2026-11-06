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
        description: "You, Pixie: your own settings and facts about you, changed instantly in-process. Never click through your own Settings window to change something; use this.\n\
action=about: everything about you right now (version, what you can do, shortcuts, voice, model, permissions, apps, connectors, experiments, updates, usage, every setting with its value). Use it when they ask about you or before answering how you're set up.\n\
action=set with changes={key: value, …} (several at once). Keys: speak_answers, look_at_screen, can_click (click and type for them), auto_update, muted (bool) · buddy (always | idle | off) · name (what to call them) · voice (a natural voice: Heart, Bella, Nicole, Sarah, Michael, Fenrir, Emma, George; or \"system\") · voice_speed (natural voice, 0.8–1.3) · speech_rate (system voice, 0.4–0.65) · volume (sounds, 0–1.5) · language (listening locale like en-US, or auto) · talk_shortcut, dictate_shortcut (chords like ctrl+option, fn+ctrl, cmd+shift) · provider (claude | openai | deepseek, needs its key) · model, agents_model (model ids for voice / background work) · backup_model (provider:model, or none) · coding_agent (auto | codex | claude | opencode) · coding_folder · memory (auto = also learns from chats | manual = only when they say remember | off) · new_chat_at (when today's chat ends and a new one starts: an hour like 0 or \"midnight\", \"4am\", \"06:00\") · persona (Pix = friendly default | Inos = nerdy | Xtrem = chaotic and witty | Eon = concise) · spicy (Xtrem may swear mildly) · drawing (how readily you draw on the canvas: asked | helps | often) · calendar, reminders, contacts, mail (Apps, Mac) · connector:<id> (switch an added connector on/off). Say what changed in a few words.\n\
action=open page=general | voice | sounds | memory | models | usage | coding | permissions | apps | connectors | experimental | about | canvas: open that page for them (API keys, permissions and sign-ins are done there by them).\n\
action=check_updates, action=restart_to_update. action=new_chat: start a fresh conversation (\"new chat\", \"start over\"); today's chat otherwise carries on all day.\n\
action=quit: quit Pixie (when they say quit, exit, close or turn off Pixie, or that they're done with you for now); say a short goodbye, it quits once you've said it. action=stop_all: stop every running job and live agent (\"stop working\", \"stop everything\").".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["about", "set", "open", "check_updates", "restart_to_update", "quit", "stop_all", "new_chat"] },
                "changes": { "type": "object", "description": "set: key → new value" },
                "page": { "type": "string" }
            },
            "required": ["action"]
        }),
    }
}

pub async fn call(app: &AppHandle, input: &Value) -> Result<String, String> {
    match input.get("action").and_then(Value::as_str).unwrap_or("about") {
        "about" => Ok(about(app).await),
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
                    _ => p,
                };
                if !matches!(p.as_str(), "claude" | "openai" | "deepseek") {
                    fail("claude, openai or deepseek".into());
                } else if crate::config::key_for(&data, &cur, &p).is_none() {
                    fail(format!("there's no {p} key yet; they add it in Settings → Models"));
                } else {
                    patch.insert("provider".into(), json!(p));
                    done.push(format!("using {p}"));
                }
            }
            "model" | "voice_model" | "agents_model" | "deep_model" | "jobs_model" => match as_str(v).filter(|s| !s.is_empty() && !s.contains(char::is_whitespace)) {
                Some(m) => {
                    let provider = patch.get("provider").and_then(Value::as_str).map(str::to_string).or_else(|| crate::config::provider_config(&data, &cur).map(|c| c.kind.id().to_string()));
                    match provider {
                        Some(p) => {
                            let mut models = cur.models.clone();
                            let e = models.entry(p.clone()).or_default();
                            let deep = matches!(key.as_str(), "agents_model" | "deep_model" | "jobs_model");
                            if deep {
                                e.deep = m.clone();
                            } else {
                                e.fast = m.clone();
                            }
                            patch.insert("models".into(), serde_json::to_value(models).unwrap_or_default());
                            done.push(format!("{} model {m} ({p})", if deep { "agents" } else { "voice" }));
                        }
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
        format!("natural voice {name}, speed {:.2}×", s.voice_speed)
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
- Voice: {voice}; speak answers {speak}; listening language {locale}.\n\
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
        buddy = if !s.buddy { "off" } else { s.buddy_mode.as_str() },
        click = on_off(s.can_click),
        screen = on_off(s.share_screen),
        vol = (s.master_volume * 100.0).round(),
        muted = if s.muted { "muted" } else { "not muted" },
        name = if s.user_name.is_empty() { "no name set" } else { &s.user_name },
        google = if google["signed_in"].as_bool().unwrap_or(false) { "signed in" } else { "not signed in" },
        conns = if conns.is_empty() { "none added".to_string() } else { conns.join(", ") },
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

/// "switch to Eon", "switch back to Pix", "be Xtrem", "use Inos", "Inos mode".
fn persona_command(clean: &str) -> Option<&'static str> {
    let words: Vec<&str> = clean.split_whitespace().collect();
    let id = |w: &str| match w {
        "pix" | "pixie" | "pixi" => Some("pix"),
        "inos" | "eenos" | "inose" => Some("inos"),
        "xtrem" | "extreme" | "xtreme" => Some("xtrem"),
        "eon" | "ion" | "eeon" => Some("eon"),
        _ => None,
    };
    const VERB: &[&str] = &["switch", "change", "be", "become", "use", "go", "turn"];
    const FILL: &[&str] = &["hey", "ok", "okay", "please", "now", "back", "to", "into", "the", "can", "you", "could", "would", "your", "personality", "persona", "mode", "style", "set", "as", "and", "just", "again", "pixie", "pixi"];
    let target = words.iter().rev().find_map(|w| id(w))?;
    let has_verb = words.iter().any(|w| VERB.contains(w));
    let mode_word = words.last().is_some_and(|w| matches!(*w, "mode" | "personality"));
    let rest_ok = words.iter().all(|w| VERB.contains(w) || FILL.contains(w) || id(w).is_some());
    (rest_ok && (has_verb || mode_word) && words.len() <= 8).then_some(target)
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
            if let Err(e) = commands::settings_set(app.clone(), json!({ "persona": p })) {
                crate::jobs::toast(app, &e, "error", None);
                return;
            }
            crate::emit(app, openclicky_core::Event::new("settings.external", json!({})));
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
        assert_eq!(hour_of(&json!("midnight")), Some(0));
        assert_eq!(hour_of(&json!("4am")), Some(4));
        assert_eq!(hour_of(&json!("6 pm")), Some(18));
        assert_eq!(hour_of(&json!("06:00")), Some(6));
        assert_eq!(hour_of(&json!(25)), None);
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
