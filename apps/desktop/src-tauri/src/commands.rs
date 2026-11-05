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

/// Hovering the resting notch opens the peek; leaving closes it.
#[tauri::command]
pub fn panel_peek(app: AppHandle, on: bool) {
    match (on, panel::mode(&app)) {
        (true, PanelMode::Hidden) => panel::set_mode(&app, PanelMode::Peek),
        (false, PanelMode::Peek) => panel::set_mode(&app, PanelMode::Hidden),
        _ => {}
    }
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
pub fn job_start(app: AppHandle, title: String, instructions: String) -> Result<String, String> {
    jobs::start(&app, &title, &instructions)
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
        key_hint: cfg.map(|c| mask(&c.api_key)).unwrap_or_default(),
        settings: Settings { deepseek_key: mask_if(&settings.deepseek_key), anthropic_key: mask_if(&settings.anthropic_key), ..settings },
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
                if (k == "deepseek_key" || k == "anthropic_key") && v.as_str().map(|x| x.contains('•')).unwrap_or(false) {
                    continue;
                }
                obj.insert(k.clone(), v.clone());
            }
        }
        *s = serde_json::from_value(cur).map_err(|e| e.to_string())?;
        s.save(&state.data_dir);
        if let Ok(mut bus) = state.audio.lock() {
            bus.set_muted(s.muted);
        }
    }
    state.reload_provider();
    Ok(settings_get(state))
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
