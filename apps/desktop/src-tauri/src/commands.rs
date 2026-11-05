//! Tauri commands called from the UI. Permission commands live in `permissions.rs`.

use crate::config::Settings;
use crate::jobs::{self, JobView};
use crate::macos::Rect;
use crate::panel::{self, PanelMode};
use crate::{hit, session, windows, AppState, PANEL, SHELF};
use openclicky_core::resources::{MissingFile, ResourceKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn panel_mode(app: AppHandle) -> PanelMode {
    panel::mode(&app)
}

#[tauri::command]
pub fn panel_screen(app: AppHandle) -> crate::macos::ScreenInfo {
    panel::screen(&app)
}

#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    session::escape(&app);
}

#[derive(Deserialize)]
pub struct HitRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// The UI reports where it actually draws, so the rest of the window lets clicks through.
#[tauri::command]
pub fn hit_rects(app: AppHandle, window: String, rects: Vec<HitRect>) {
    let label = if window == SHELF { SHELF } else { PANEL };
    hit::set_rects(&app, label, rects.into_iter().map(|r| Rect { x: r.x, y: r.y, w: r.w, h: r.h }).collect());
}

#[tauri::command]
pub fn panel_hover(state: State<'_, AppState>, hovered: bool) {
    state.session.set_hovered(hovered);
}

/// The running app bundle (what to drag into a Privacy list).
fn app_bundle() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.ancestors()
        .find(|p| p.extension().map(|e| e == "app").unwrap_or(false))
        .map(|p| p.to_path_buf())
        .unwrap_or(exe)
}

/// HeyClicky-style helper: a card under System Settings with a draggable app tile.
#[tauri::command]
pub fn guide_show(app: AppHandle, id: String) {
    let state = app.state::<AppState>();
    let icon = state.store.resolve(ResourceKind::Icon, "app-icon-1024").map(|p| p.display().to_string()).unwrap_or_default();
    let bundle = app_bundle().display().to_string();
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = h.get_webview_window(crate::GUIDE) {
            let s = crate::macos::screens().into_iter().next().unwrap_or_default();
            let (gw, gh) = (440.0, 130.0);
            let frame = Rect { x: s.frame.x + (s.frame.w - gw) / 2.0, y: s.frame.y + s.frame.h - gh - 70.0, w: gw, h: gh };
            crate::macos::configure_overlay(&w, crate::macos::PANEL_LEVEL);
            crate::macos::set_frame(&w, frame);
            crate::macos::order_front_without_focus(&w);
        }
    });
    crate::emit(&app, openclicky_core::Event::new("guide.show", json!({ "perm": id, "app": bundle, "icon": icon })));
}

#[tauri::command]
pub fn guide_hide(app: AppHandle) {
    crate::emit(&app, openclicky_core::Event::new("guide.hide", json!({})));
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = h.get_webview_window(crate::GUIDE) {
            crate::macos::order_out(&w);
        }
    });
}

#[tauri::command]
pub fn board_close(app: AppHandle) {
    crate::overlay::board_close(&app);
}

/// Hovering the resting notch opens the peek; leaving closes it.
#[tauri::command]
pub fn panel_peek(app: AppHandle, on: bool) {
    match (on, panel::mode(&app)) {
        (true, PanelMode::Hidden) => panel::set_mode(&app, PanelMode::Peek),
        (false, PanelMode::Peek) => panel::set_mode(&app, PanelMode::Hidden),
        _ => {}
    }
}

/// The Chat tab's log of this conversation.
#[tauri::command]
pub fn chat_log(state: State<'_, AppState>) -> Vec<crate::session::ChatEntry> {
    state.session.chat_log()
}

/// "New chat": forget the conversation.
#[tauri::command]
pub fn chat_clear(app: AppHandle, state: State<'_, AppState>) {
    state.session.chat_clear();
    crate::emit(&app, openclicky_core::Event::new("chat.updated", serde_json::json!({ "log": [] })));
}

/// Use a file of theirs for a sound slot (bytes come from the file picker in Settings).
/// Empty data = back to the default sound.
#[tauri::command]
pub fn sound_custom(app: AppHandle, slot: String, name: String, data: String) -> Result<SettingsView, String> {
    use base64::Engine;
    let state = app.state::<AppState>();
    if !slot.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || slot.is_empty() {
        return Err("Unknown sound.".into());
    }
    let dir = state.data_dir.join("custom-sounds");
    // Remove an earlier custom file for this slot.
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().file_stem().and_then(|s| s.to_str()) == Some(slot.as_str()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let custom = if data.is_empty() {
        String::new()
    } else {
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        if !["wav", "mp3", "flac", "ogg"].contains(&ext.as_str()) {
            return Err("Use a .wav, .mp3, .flac or .ogg file.".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|e| e.to_string())?;
        if bytes.len() > 5 * 1024 * 1024 {
            return Err("Keep sounds under 5 MB.".into());
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{slot}.{ext}"));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        path.display().to_string()
    };
    {
        let mut s = state.settings.lock().unwrap();
        s.sounds.entry(slot.clone()).or_default().custom = custom;
        s.save(&state.data_dir);
        if let Ok(mut bus) = state.audio.lock() {
            crate::config::apply_audio(&mut bus, &s);
        }
    }
    state.play(&slot);
    Ok(settings_get(state))
}

/// Hover tips from Pixie's own windows, shown beside the buddy.
#[tauri::command]
pub async fn buddy_tip(app: AppHandle, text: String) {
    crate::buddy::tip(&app, &text);
}

/// Past conversations, newest first.
#[tauri::command]
pub fn chats_list(state: State<'_, AppState>) -> Vec<crate::chats::ChatSummary> {
    crate::chats::list(&state.data_dir)
}

#[tauri::command]
pub fn chat_get(state: State<'_, AppState>, id: String) -> Option<crate::chats::Chat> {
    crate::chats::get(&state.data_dir, &id)
}

#[tauri::command]
pub fn chat_delete(app: AppHandle, state: State<'_, AppState>, id: String) {
    crate::chats::delete(&state.data_dir, &id);
    crate::emit(&app, openclicky_core::Event::new("chats.changed", serde_json::json!({})));
}

/// Click-to-talk from the peek (same as holding ⌃⌥).
#[tauri::command]
pub fn talk_start(app: AppHandle) {
    session::talk_down(&app);
}

#[tauri::command]
pub fn talk_stop(app: AppHandle) {
    session::talk_up(&app, false, true);
}

/// Open the text box (the panel becomes key so it can take typing).
#[tauri::command]
pub fn panel_type(app: AppHandle) {
    panel::set_mode(&app, PanelMode::Typing);
}

#[tauri::command]
pub fn ask(app: AppHandle, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Type something first.".into());
    }
    session::ask_text(&app, text);
    Ok(())
}

#[tauri::command]
pub fn session_note(state: State<'_, AppState>) -> Value {
    let src = std::fs::read_to_string(state.data_dir.join("session/note.md")).unwrap_or_default();
    if src.trim().is_empty() {
        return Value::Null;
    }
    serde_json::to_value(openclicky_core::note::parse(&src)).unwrap_or(Value::Null)
}

/// Play a UI sound by slot. Only slots in the manifest can play.
#[tauri::command]
pub fn play_sound(state: State<'_, AppState>, slot: String) -> Result<(), String> {
    if state.store.entry(ResourceKind::Sound, &slot).is_none() {
        return Err(format!("unknown sound slot `{slot}`"));
    }
    state.play(&slot);
    Ok(())
}

#[tauri::command]
pub fn stop_bed(state: State<'_, AppState>) {
    if let Ok(mut bus) = state.audio.lock() {
        bus.stop_bed();
    }
}

#[tauri::command]
pub fn show_settings(app: AppHandle, tab: Option<String>) {
    windows::open_settings(&app, tab.as_deref().unwrap_or(""));
}

#[derive(Serialize)]
pub struct ResourcesReport {
    root: String,
    pack: crate::resources::PackSource,
    sounds: usize,
    missing: Vec<MissingFile>,
}

#[tauri::command]
pub fn resources_report(state: State<'_, AppState>) -> ResourcesReport {
    ResourcesReport {
        root: state.store.root().display().to_string(),
        pack: state.pack,
        sounds: state.store.manifest().sounds.len(),
        missing: state.store.missing(),
    }
}

/// Design tokens (`theme.json`) for the UI to apply as CSS custom properties.
#[tauri::command]
pub fn theme(state: State<'_, AppState>) -> Value {
    state.theme.clone()
}

// ---- jobs ------------------------------------------------------------------------------

#[tauri::command]
pub fn jobs_list(state: State<'_, AppState>) -> Vec<JobView> {
    state.jobs.list()
}

#[tauri::command]
pub fn job_get(state: State<'_, AppState>, id: String) -> Option<JobView> {
    state.jobs.get(&id)
}

#[tauri::command]
pub fn job_note(app: AppHandle, id: String) -> Option<Value> {
    jobs::note_doc(&app, &id)
}

#[tauri::command]
pub fn job_start(app: AppHandle, title: String, instructions: String, kind: Option<String>) -> Result<String, String> {
    jobs::start(&app, &title, &instructions, kind.as_deref().unwrap_or("background"))
}

#[tauri::command]
pub fn job_answer(app: AppHandle, id: String, text: String) -> Result<(), String> {
    jobs::answer(&app, &id, &text)
}

#[tauri::command]
pub fn job_cancel(app: AppHandle, id: String) {
    jobs::cancel(&app, &id);
}

#[tauri::command]
pub fn job_dismiss(app: AppHandle, id: String) {
    jobs::dismiss(&app, &id);
}

#[tauri::command]
pub fn job_delete(app: AppHandle, id: String) {
    jobs::delete(&app, &id);
}

#[tauri::command]
pub fn open_note(app: AppHandle, id: String) {
    jobs::dismiss(&app, &id);
    windows::open_note(&app, &id);
}

/// The Jobs window on its Chats side (optionally a given conversation).
#[tauri::command]
pub fn open_chats(app: AppHandle, id: Option<String>) {
    windows::open_note(&app, &format!("chats:{}", id.unwrap_or_default()));
}

/// Open an artifact (path or URL) the way Finder would.
#[tauri::command]
pub fn open_target(target: String, reveal: Option<bool>) -> Result<(), String> {
    let mut cmd = std::process::Command::new("/usr/bin/open");
    if reveal.unwrap_or(false) {
        cmd.arg("-R");
    }
    let status = cmd.arg(&target).status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("Couldn't open {target}")) }
}

// ---- settings --------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SettingsView {
    settings: Settings,
    provider: Option<String>,
    /// Where the key came from (masked), for the UI.
    key_hint: String,
    /// The provider answering now ("claude" / "openai" / "deepseek").
    active: Option<String>,
    /// The models each provider would use (choices, else env, else built-in defaults).
    effective: std::collections::BTreeMap<String, crate::config::ModelChoice>,
}

fn mask(k: &str) -> String {
    if k.len() <= 8 {
        return "••••".into();
    }
    format!("{}••••{}", &k[..4], &k[k.len() - 4..])
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> SettingsView {
    let settings = state.settings.lock().unwrap().clone();
    let cfg = crate::config::provider_config(&state.data_dir, &settings);
    SettingsView {
        provider: state.provider().map(|p| p.label()),
        active: cfg.as_ref().map(|c| c.kind.id().to_string()),
        effective: crate::config::effective_models(&state.data_dir, &settings),
        key_hint: cfg.map(|c| mask(&c.api_key)).unwrap_or_default(),
        settings: Settings {
            deepseek_key: mask_if(&settings.deepseek_key),
            anthropic_key: mask_if(&settings.anthropic_key),
            openai_key: mask_if(&settings.openai_key),
            google_client_secret: mask_if(&settings.google_client_secret),
            ..settings
        },
    }
}

fn mask_if(k: &str) -> String {
    if k.is_empty() { String::new() } else { mask(k) }
}

/// Merge a partial settings object. Masked keys coming back unchanged are ignored.
#[tauri::command]
pub fn settings_set(app: AppHandle, patch: Value) -> Result<SettingsView, String> {
    let state = app.state::<AppState>();
    {
        let mut s = state.settings.lock().unwrap();
        let mut cur = serde_json::to_value(&*s).map_err(|e| e.to_string())?;
        if let (Some(obj), Some(p)) = (cur.as_object_mut(), patch.as_object()) {
            for (k, v) in p {
                if (k == "deepseek_key" || k == "anthropic_key" || k == "openai_key" || k == "google_client_secret") && v.as_str().map(|x| x.contains('•')).unwrap_or(false) {
                    continue;
                }
                obj.insert(k.clone(), v.clone());
            }
        }
        *s = serde_json::from_value(cur).map_err(|e| e.to_string())?;
        s.save(&state.data_dir);
        if let Ok(mut bus) = state.audio.lock() {
            crate::config::apply_audio(&mut bus, &s);
        }
        crate::hotkey::configure(&s.shortcut_talk, &s.shortcut_dictate);
    }
    state.reload_provider();
    crate::emit(&app, openclicky_core::Event::new("settings.changed", json!({})));
    Ok(settings_get(state))
}

/// Live model list for a provider, from its official endpoint (uses that provider's key).
#[tauri::command]
pub async fn models_available(app: AppHandle, provider: String) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let models = crate::usage::fetch_models(&state.data_dir, &settings, &provider).await?;
    let at = crate::usage::catalog().lock().map(|c| c.fetched_at).unwrap_or(0);
    Ok(json!({ "models": models, "prices_at": at }))
}

/// Fetch the live price list now.
#[tauri::command]
pub async fn prices_refresh(app: AppHandle) -> Result<Value, String> {
    let data = app.state::<AppState>().data_dir.clone();
    let n = crate::usage::refresh_catalog(&data).await?;
    Ok(json!({ "models": n, "fetched_at": crate::usage::catalog().lock().map(|c| c.fetched_at).unwrap_or(0) }))
}

#[tauri::command]
pub fn usage_summary(app: AppHandle, days: Option<i64>, provider: Option<String>, kind: Option<String>) -> Value {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let lines = crate::usage::read(&state.data_dir);
    let f = crate::usage::Filter { days: days.unwrap_or(30), provider: provider.unwrap_or_default(), kind: kind.unwrap_or_default() };
    let mut v = crate::usage::summary_with(&lines, &settings, &f, chrono::Utc::now().timestamp_millis());
    v["prices_at"] = json!(crate::usage::catalog().lock().map(|c| c.fetched_at).unwrap_or(0));
    v
}

#[tauri::command]
pub fn coders_status() -> Value {
    let found = openclicky_tools::coders::detect();
    let agents: Vec<Value> = ["codex", "claude", "opencode"]
        .iter()
        .map(|name| {
            let c = found.iter().find(|c| c.name == *name);
            json!({
                "id": name,
                "label": match *name { "codex" => "Codex CLI", "claude" => "Claude Code", _ => "opencode" },
                "installed": c.is_some(),
                "path": c.map(|c| c.path.display().to_string()),
                "version": c.map(openclicky_tools::coders::version),
            })
        })
        .collect();
    let t3 = std::path::Path::new("/Applications/T3 Code (Nightly).app").exists() || std::path::Path::new("/Applications/T3 Code.app").exists();
    json!({ "agents": agents, "t3code": t3 })
}

/// One tiny request to prove the key works.
#[tauri::command]
pub async fn provider_test(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let p = state.provider().ok_or("No key yet.")?;
    let mut req = openclicky_llm::Request::new("Reply with exactly: ready", vec![openclicky_llm::Message::user_text("ping")]);
    req.max_tokens = 20;
    let started = std::time::Instant::now();
    let r = tokio::time::timeout(Duration::from_secs(20), p.complete(&req, &mut |_| {}))
        .await
        .map_err(|_| "No answer in 20 s.".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(format!("{} answered in {} ms: “{}”", p.label(), started.elapsed().as_millis(), r.text().trim()))
}

#[tauri::command]
pub async fn voices(app: AppHandle) -> Result<Value, String> {
    app.state::<AppState>().helper.call("voices", json!({}), Duration::from_secs(5)).await
}

#[tauri::command]
pub fn voice_test(app: AppHandle, text: Option<String>) {
    let line = text.unwrap_or_else(|| "Hi! Hold control and option, and ask me anything about your screen.".into());
    crate::speech::speak(&app, "test", &line);
}

#[tauri::command]
pub fn voice_status(app: AppHandle) -> crate::kokoro::Status {
    crate::kokoro::status(&app)
}

#[tauri::command]
pub fn voice_install(app: AppHandle) -> Result<(), String> {
    crate::kokoro::install(app)
}

/// Onboarding mic check: listen without starting a turn (levels and words stream as events).
#[tauri::command]
pub async fn mic_test_start(app: AppHandle) -> Result<(), String> {
    let st = app.state::<AppState>();
    let locale = st.settings.lock().map(|s| s.locale.clone()).unwrap_or_default();
    st.helper.call("listen_start", json!({ "locale": locale, "on_device": true }), Duration::from_secs(15)).await.map(|_| ())
}

#[tauri::command]
pub async fn mic_test_stop(app: AppHandle) -> Result<String, String> {
    let st = app.state::<AppState>();
    let v = st.helper.call("listen_stop", json!({}), Duration::from_secs(5)).await?;
    Ok(v["text"].as_str().unwrap_or("").trim().to_string())
}

#[tauri::command]
pub fn onboarding_done(app: AppHandle) {
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    s.onboarded = true;
    s.save(&state.data_dir);
}

#[tauri::command]
pub fn status(app: AppHandle) -> Value {
    let st = app.state::<AppState>();
    json!({
        "helper": st.helper.available(),
        "provider": st.provider().map(|p| p.label()),
        "browser": crate::browser::connected(&app),
        "connectors": st.connected_apps(),
        "data_dir": st.data_dir,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

/// The wizard is a normal window: the user asked for it, so it may take focus.
pub fn open_permissions_window(app: &AppHandle) {
    windows::open_settings(app, "setup");
}

// ---- apps, Google, connectors ----------------------------------------------------------

#[tauri::command]
pub async fn apps_status(app: AppHandle) -> Value {
    let mut v = crate::integrations::status(&app).await;
    v["google"] = crate::google::status(&app);
    v
}

#[tauri::command]
pub async fn app_set(app: AppHandle, kind: String, on: bool) -> Result<String, String> {
    crate::integrations::set(&app, &kind, on).await
}

#[tauri::command]
pub async fn google_sign_in(app: AppHandle) -> Result<String, String> {
    crate::google::sign_in(&app).await
}

#[tauri::command]
pub fn google_sign_out(app: AppHandle) {
    crate::google::sign_out(&app)
}

#[tauri::command]
pub fn connectors_gallery(app: AppHandle) -> Value {
    crate::connectors::gallery(&app)
}

#[tauri::command]
pub fn connector_save(app: AppHandle, id: String, values: Value, on: bool) -> Result<Value, String> {
    crate::connectors::save(&app, &id, &values, on)?;
    Ok(crate::connectors::gallery(&app))
}

#[tauri::command]
pub fn connector_remove(app: AppHandle, id: String) -> Result<Value, String> {
    crate::connectors::remove(&app, &id)?;
    Ok(crate::connectors::gallery(&app))
}
