//! Rolling back a bad update (self-healing 5, macOS).
//!
//! Before an update replaces the app, the running version is copied to `<app_data>/previous/`
//! (about 60 MB). Each launch is noted in `launches.json`; a launch that neither lasts a minute
//! nor ends with Quit counts as an early crash. After three early crashes in a row on the
//! version an update brought, the next launch puts the previous version back, opens it, and
//! skips that version in later update checks (the next release is still offered).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

const EARLY: Duration = Duration::from_secs(60);
const STRIKES: u32 = 3;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Launches {
    pub version: String,
    /// Early crashes in a row on this version.
    pub early: u32,
    /// The last launch lasted a minute or ended with Quit.
    pub fine: bool,
}

/// This launch, given the last one: how many early crashes in a row now.
pub fn next(prev: Option<&Launches>, version: &str) -> Launches {
    let early = match prev {
        Some(p) if p.version == version && !p.fine => p.early + 1,
        _ => 0,
    };
    Launches { version: version.to_string(), early, fine: false }
}

fn launches_file(data: &Path) -> PathBuf {
    data.join("launches.json")
}

fn previous_dir(data: &Path) -> PathBuf {
    data.join("previous")
}

fn save(data: &Path, l: &Launches) {
    if let Ok(s) = serde_json::to_string(l) {
        let _ = std::fs::write(launches_file(data), s);
    }
}

/// The running app bundle (`…/Pixie.app`), when Pixie runs from one.
fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app = exe.parent()?.parent()?.parent()?.to_path_buf();
    (app.extension().map(|e| e == "app").unwrap_or(false)).then_some(app)
}

/// The version an update must not bring back (it was rolled back).
pub fn skipped(data: &Path) -> Option<String> {
    std::fs::read_to_string(data.join("skip-version")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Before an update replaces the app: keep the running version.
pub fn keep_current(data: &Path, current: &str, to: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let Some(app) = bundle() else { return };
    let dir = previous_dir(data);
    let _ = std::fs::remove_dir_all(&dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let dest = dir.join(app.file_name().unwrap_or_default());
    // ditto keeps the signature and extended attributes, so the copy still opens.
    let ok = std::process::Command::new("/usr/bin/ditto").arg(&app).arg(&dest).status().map(|s| s.success()).unwrap_or(false);
    if ok {
        let _ = std::fs::write(dir.join("version"), current);
        let _ = std::fs::write(dir.join("to"), to);
        log::info!("update: kept {current} in case {to} doesn't start");
    } else {
        log::warn!("update: couldn't keep a copy of {current}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// At launch. True when Pixie is rolling back and should quit now.
pub fn on_launch(data: &Path, version: &str) -> bool {
    let prev: Option<Launches> = std::fs::read_to_string(launches_file(data)).ok().and_then(|s| serde_json::from_str(&s).ok());
    let now = next(prev.as_ref(), version);
    save(data, &now);
    if let Ok(note) = std::fs::read_to_string(data.join("rolled-back.txt")) {
        let _ = std::fs::remove_file(data.join("rolled-back.txt"));
        crate::heal::notice(note.trim());
    }
    let data2 = data.to_path_buf();
    let v = version.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(EARLY);
        save(&data2, &Launches { version: v, early: 0, fine: true });
    });
    if now.early < STRIKES {
        return false;
    }
    let dir = previous_dir(data);
    let to = std::fs::read_to_string(dir.join("to")).unwrap_or_default();
    let old = std::fs::read_to_string(dir.join("version")).unwrap_or_default();
    let (Some(app), true) = (bundle(), to.trim() == version) else { return false };
    let Some(copy) = std::fs::read_dir(&dir).ok().and_then(|r| r.flatten().map(|e| e.path()).find(|p| p.extension().map(|e| e == "app").unwrap_or(false))) else {
        return false;
    };
    log::error!("update: {version} closed early {} times in a row; putting {old} back", now.early);
    crate::heal::note("update", &format!("Pixie {version} kept closing right after it started"), &format!("Put {old} back and skipped {version}"), "");
    let _ = std::fs::write(data.join("skip-version"), version);
    let _ = std::fs::write(
        data.join("rolled-back.txt"),
        format!("Pixie {version} kept closing right after it started, so {old} is back. Updates skip {version}; the next release will be offered."),
    );
    // Swap the apps once this process has gone, then open the old one.
    let script = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; rm -rf "$2" && /usr/bin/ditto "$3" "$2" && rm -rf "$4"; /usr/bin/open "$2""#;
    let spawned = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg("pixie-rollback")
        .arg(std::process::id().to_string())
        .arg(&app)
        .arg(&copy)
        .arg(&dir)
        .spawn();
    spawned.is_ok()
}

/// Quit on purpose: not a crash.
pub fn on_quit(data: &Path, version: &str) {
    save(data, &Launches { version: version.to_string(), early: 0, fine: true });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_crashes_are_counted_per_version() {
        let a = next(None, "0.8.0");
        assert_eq!(a.early, 0);
        // Closed before a minute without Quit, three times.
        let b = next(Some(&a), "0.8.0");
        let c = next(Some(&b), "0.8.0");
        let d = next(Some(&c), "0.8.0");
        assert_eq!(d.early, 3);
        // A launch that lasted resets it; so does a new version.
        assert_eq!(next(Some(&Launches { fine: true, ..d.clone() }), "0.8.0").early, 0);
        assert_eq!(next(Some(&d), "0.8.1").early, 0);
    }
}
