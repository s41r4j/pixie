//! Permissions wizard backend (BUILD-SPEC §9): detect each state, request it, or deep-link
//! to the right System Settings pane.

use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub enum PermState {
    Granted,
    Denied,
    Restricted,
    NotDetermined,
    /// API can't distinguish denied from not-yet-asked (Screen Recording, Accessibility).
    NotGranted,
    /// Not checked yet: checking would itself trigger the prompt (Files & Folders).
    Unchecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermId {
    Microphone,
    Screen,
    Accessibility,
    Speech,
    Files,
}

impl PermId {
    const ALL: [PermId; 5] =
        [PermId::Microphone, PermId::Screen, PermId::Accessibility, PermId::Speech, PermId::Files];

    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.meta().0 == s)
    }

    /// (id, label, why, required, settings anchor)
    fn meta(self) -> (&'static str, &'static str, &'static str, bool, &'static str) {
        match self {
            PermId::Microphone => ("microphone", "Microphone", "Hear you while you hold ⌃⌥.", true, "Privacy_Microphone"),
            PermId::Screen => ("screen", "Screen Recording", "See what you're looking at when you ask about it.", true, "Privacy_ScreenCapture"),
            PermId::Accessibility => ("accessibility", "Accessibility", "Point at buttons and type into fields for you.", true, "Privacy_Accessibility"),
            PermId::Speech => ("speech", "Speech Recognition", "Turn your voice into text, on this Mac.", true, "Privacy_SpeechRecognition"),
            PermId::Files => ("files", "Desktop, Documents & Downloads", "Find and save files where you keep them.", false, "Privacy_FilesAndFolders"),
        }
    }

    fn settings_url(self) -> String {
        format!("x-apple.systempreferences:com.apple.preference.security?{}", self.meta().4)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionInfo {
    id: &'static str,
    label: &'static str,
    why: &'static str,
    required: bool,
    state: PermState,
    /// Screen Recording only takes effect after a relaunch.
    restart_hint: bool,
}

static FILES_STATE: Mutex<Option<PermState>> = Mutex::new(None);

fn info(id: PermId) -> PermissionInfo {
    let (sid, label, why, required, _) = id.meta();
    PermissionInfo {
        id: sid,
        label,
        why,
        required,
        state: state(id),
        restart_hint: id == PermId::Screen,
    }
}

fn state(id: PermId) -> PermState {
    match id {
        PermId::Files => FILES_STATE.lock().ok().and_then(|s| *s).unwrap_or(PermState::Unchecked),
        #[cfg(target_os = "macos")]
        _ => native::state(id),
        #[cfg(not(target_os = "macos"))]
        _ => PermState::Granted,
    }
}

/// Mic, Screen Recording and Accessibility are needed for the core loop.
pub fn required_granted() -> bool {
    PermId::ALL
        .into_iter()
        .filter(|p| p.meta().3)
        .all(|p| state(p) == PermState::Granted)
}

/// Reading the three folders is the only way to learn (and trigger) their TCC state.
fn check_files() -> PermState {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return PermState::Unchecked;
    };
    let mut result = PermState::Granted;
    for dir in ["Desktop", "Documents", "Downloads"] {
        if let Err(e) = std::fs::read_dir(home.join(dir)) {
            log::info!("files permission: ~/{dir}: {e}");
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                result = PermState::Denied;
            }
        }
    }
    if let Ok(mut s) = FILES_STATE.lock() {
        *s = Some(result);
    }
    result
}

fn open_settings(id: PermId) {
    let url = id.settings_url();
    if let Err(e) = std::process::Command::new("open").arg(&url).spawn() {
        log::warn!("open {url}: {e}");
    }
}

/// Screen Recording as a fresh process sees it. `CGPreflightScreenCaptureAccess` in this
/// process is cached from launch, so a grant made in System Settings shows up here first.
fn screen_check_fresh() -> bool {
    // The UI polls every 1.5 s; one subprocess every 3 s is plenty.
    static LAST: std::sync::Mutex<Option<(std::time::Instant, bool)>> = std::sync::Mutex::new(None);
    if let Some((at, v)) = *LAST.lock().unwrap() {
        if at.elapsed() < std::time::Duration::from_secs(3) {
            return v;
        }
    }
    let v = screen_check_run();
    *LAST.lock().unwrap() = Some((std::time::Instant::now(), v));
    v
}

fn screen_check_run() -> bool {
    let Some(helper) = crate::helper::locate() else { return false };
    let Ok(mut child) = std::process::Command::new(helper)
        .arg("screen-check")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        if let Ok(Some(status)) = child.try_wait() {
            return status.success();
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    let _ = child.kill();
    false
}

#[tauri::command]
pub async fn permissions_status(app: tauri::AppHandle) -> Vec<PermissionInfo> {
    let list: Vec<PermissionInfo> = tauri::async_runtime::spawn_blocking(|| PermId::ALL.into_iter().map(info).collect())
        .await
        .unwrap_or_default();
    let screen_missing = list.iter().any(|p| p.id == "screen" && p.state != PermState::Granted);
    if !screen_missing {
        return list;
    }
    let fresh = tauri::async_runtime::spawn_blocking(screen_check_fresh).await.unwrap_or(false);
    if !fresh {
        return list;
    }
    // Granted since launch: restart the helper so capture picks it up, and say so.
    crate::helper::restart(&app);
    list.into_iter()
        .map(|mut p| {
            if p.id == "screen" {
                p.state = PermState::Granted;
            }
            p
        })
        .collect()
}

/// Clear openclicky's own stale grant for Screen Recording or Accessibility (a switch that's
/// on for an older build of the app) and ask again. Touches only com.openclicky.app.
#[tauri::command]
pub async fn permission_reset(id: String) -> Result<PermissionInfo, String> {
    let pid = PermId::parse(&id).ok_or_else(|| format!("unknown permission `{id}`"))?;
    let service = match pid {
        PermId::Screen => "ScreenCapture",
        PermId::Accessibility => "Accessibility",
        _ => return Err("Only Screen Recording and Accessibility can be reset here.".into()),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let ok = std::process::Command::new("/usr/bin/tccutil")
            .args(["reset", service, "com.openclicky.app"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            log::warn!("tccutil reset {service} failed");
        }
        #[cfg(target_os = "macos")]
        {
            // Ask fresh so openclicky reappears in the list, then show the pane.
            match pid {
                PermId::Screen => unsafe { native::request_screen_now() },
                _ => {
                    native::request(pid);
                }
            }
        }
        open_settings(pid);
        info(pid)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Quit and reopen openclicky (some grants only apply to a fresh process).
#[tauri::command]
pub fn relaunch(app: tauri::AppHandle) {
    app.restart();
}

/// Ask for a permission. Shows the system prompt when macOS still allows one; otherwise
/// opens the matching System Settings pane. The UI polls `permissions_status` for the result.
#[tauri::command]
pub async fn permission_request(id: String) -> Result<PermissionInfo, String> {
    let pid = PermId::parse(&id).ok_or_else(|| format!("unknown permission `{id}`"))?;
    tauri::async_runtime::spawn_blocking(move || {
        match pid {
            PermId::Files if check_files() == PermState::Denied => open_settings(pid),
            PermId::Files => {}
            #[cfg(target_os = "macos")]
            _ => {
                if !native::request(pid) {
                    open_settings(pid);
                }
            }
            #[cfg(not(target_os = "macos"))]
            _ => {}
        }
        info(pid)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn permission_open_settings(id: String) -> Result<(), String> {
    let pid = PermId::parse(&id).ok_or_else(|| format!("unknown permission `{id}`"))?;
    open_settings(pid);
    Ok(())
}

#[cfg(target_os = "macos")]
mod native {
    use super::{PermId, PermState};
    use block2::RcBlock;
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::{class, msg_send};
    use objc2_foundation::{NSBundle, NSString};
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}
    #[link(name = "Speech", kind = "framework")]
    extern "C" {}

    /// AVMediaTypeAudio's value.
    const AV_MEDIA_TYPE_AUDIO: &str = "soun";

    fn av_status(raw: isize) -> PermState {
        // AVAuthorizationStatus: 0 notDetermined, 1 restricted, 2 denied, 3 authorized
        match raw {
            0 => PermState::NotDetermined,
            1 => PermState::Restricted,
            2 => PermState::Denied,
            _ => PermState::Granted,
        }
    }

    fn speech_status(raw: isize) -> PermState {
        // SFSpeechRecognizerAuthorizationStatus: 0 notDetermined, 1 denied, 2 restricted, 3 authorized
        match raw {
            0 => PermState::NotDetermined,
            1 => PermState::Denied,
            2 => PermState::Restricted,
            _ => PermState::Granted,
        }
    }

    /// Requesting mic/speech access without a usage string aborts the process, so check
    /// the Info.plist (bundled, or embedded by tauri-build in dev) first.
    fn has_usage_string(key: &str) -> bool {
        let bundle = NSBundle::mainBundle();
        let key = NSString::from_str(key);
        let value: Option<Retained<AnyObject>> =
            unsafe { msg_send![&bundle, objectForInfoDictionaryKey: &*key] };
        value.is_some()
    }

    fn mic_class() -> Option<&'static AnyClass> {
        AnyClass::get(c"AVCaptureDevice")
    }

    fn speech_class() -> Option<&'static AnyClass> {
        AnyClass::get(c"SFSpeechRecognizer")
    }

    pub fn state(id: PermId) -> PermState {
        autoreleasepool(|_| match id {
            PermId::Screen => {
                if unsafe { CGPreflightScreenCaptureAccess() } { PermState::Granted } else { PermState::NotGranted }
            }
            PermId::Accessibility => {
                if unsafe { AXIsProcessTrusted() } { PermState::Granted } else { PermState::NotGranted }
            }
            PermId::Microphone => match mic_class() {
                Some(cls) => {
                    let media = NSString::from_str(AV_MEDIA_TYPE_AUDIO);
                    let raw: isize = unsafe { msg_send![cls, authorizationStatusForMediaType: &*media] };
                    av_status(raw)
                }
                None => PermState::NotGranted,
            },
            PermId::Speech => match speech_class() {
                Some(cls) => speech_status(unsafe { msg_send![cls, authorizationStatus] }),
                None => PermState::NotGranted,
            },
            PermId::Files => unreachable!("files handled by caller"),
        })
    }

    pub unsafe fn request_screen_now() {
        CGRequestScreenCaptureAccess();
    }

    /// Returns `true` if a system prompt was (or may have been) shown; `false` means the
    /// user must flip the switch in System Settings themselves.
    pub fn request(id: PermId) -> bool {
        let current = state(id);
        if current == PermState::Granted {
            return true;
        }
        autoreleasepool(|_| match id {
            PermId::Screen => {
                // Ask macOS once so openclicky appears in the list; after that just open the
                // pane (repeating the request only repeats the dialog).
                static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if !ASKED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    unsafe { CGRequestScreenCaptureAccess() };
                    return true;
                }
                false
            }
            PermId::Accessibility => {
                let key = NSString::from_str("AXTrustedCheckOptionPrompt");
                let yes: Retained<AnyObject> = unsafe { msg_send![class!(NSNumber), numberWithBool: true] };
                let opts: Retained<AnyObject> =
                    unsafe { msg_send![class!(NSDictionary), dictionaryWithObject: &*yes, forKey: &*key] };
                unsafe { AXIsProcessTrustedWithOptions(Retained::as_ptr(&opts) as *const c_void) };
                true
            }
            PermId::Microphone => {
                if current != PermState::NotDetermined || !has_usage_string("NSMicrophoneUsageDescription") {
                    return false;
                }
                let Some(cls) = mic_class() else { return false };
                let media = NSString::from_str(AV_MEDIA_TYPE_AUDIO);
                let done = RcBlock::new(|granted: Bool| log::info!("microphone granted: {}", granted.as_bool()));
                unsafe {
                    let _: () = msg_send![cls, requestAccessForMediaType: &*media, completionHandler: &*done];
                }
                true
            }
            PermId::Speech => {
                if current != PermState::NotDetermined || !has_usage_string("NSSpeechRecognitionUsageDescription") {
                    return false;
                }
                let Some(cls) = speech_class() else { return false };
                let done = RcBlock::new(|status: isize| log::info!("speech recognition status: {status}"));
                unsafe {
                    let _: () = msg_send![cls, requestAuthorization: &*done];
                }
                true
            }
            PermId::Files => unreachable!("files handled by caller"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_settings_urls() {
        for p in PermId::ALL {
            assert_eq!(PermId::parse(p.meta().0), Some(p));
            assert!(p.settings_url().starts_with("x-apple.systempreferences:com.apple.preference.security?Privacy_"));
        }
        assert_eq!(PermId::parse("camera"), None);
    }
}
