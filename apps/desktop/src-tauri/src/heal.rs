//! Self-healing: a log file, crash notes, last-good copies of the files Pixie can't lose, a
//! "to fix" list of workarounds, a checkup, and an issue report the user posts themselves.
//!
//! Everything stays on this computer. Nothing is ever sent anywhere by Pixie: a report opens a
//! pre-filled GitHub issue page in their browser, and they read it and press Submit.

use pixie_core::sync::LockOk;
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static DATA: OnceLock<PathBuf> = OnceLock::new();
static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);
/// Things to tell them once the notch is up (a crash last time, a file restored).
static NOTICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

const LOG_MAX: u64 = 2 * 1024 * 1024;
const ISSUES_MAX: usize = 60;
pub const REPO_ISSUES: &str = "https://github.com/s41r4j/pixie/issues/new";

fn logs_dir(data: &Path) -> PathBuf {
    data.join("logs")
}

pub fn log_path(data: &Path) -> PathBuf {
    logs_dir(data).join("pixie.log")
}

fn issues_path(data: &Path) -> PathBuf {
    data.join("issues.md")
}

/// Bytes logged since the size was last checked.
static WRITTEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const LOG_CHECK: u64 = 64 * 1024;

/// When the log is over [`LOG_MAX`]: keep it as pixie.1.log (replacing the one before) and
/// return a fresh pixie.log. None when it isn't full yet.
fn rotate(data: &Path) -> Option<std::fs::File> {
    let path = log_path(data);
    if !std::fs::metadata(&path).map(|m| m.len() > LOG_MAX).unwrap_or(false) {
        return None;
    }
    let _ = std::fs::rename(&path, logs_dir(data).join("pixie.1.log"));
    std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()
}

/// Log lines go to stderr and, once [`init`] ran, to `<app_data>/logs/pixie.log`.
pub struct Tee;

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        if let Ok(mut g) = LOG.lock() {
            if let Some(f) = g.as_mut() {
                let _ = f.write_all(buf);
            }
            // A Pixie left running for weeks: start a new file when this one is full, the same
            // way a launch does (checked once per LOG_CHECK bytes written).
            let written = WRITTEN.fetch_add(buf.len() as u64, std::sync::atomic::Ordering::Relaxed) + buf.len() as u64;
            if written >= LOG_CHECK {
                WRITTEN.store(0, std::sync::atomic::Ordering::Relaxed);
                if let Some(data) = DATA.get() {
                    if let Some(f) = rotate(data) {
                        *g = Some(f);
                    }
                }
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if let Ok(mut g) = LOG.lock() {
            if let Some(f) = g.as_mut() {
                let _ = f.flush();
            }
        }
        std::io::stderr().flush()
    }
}

/// At launch: open the log (the previous one is kept as pixie.1.log when it's big), install
/// the crash-note hook, and pick up a crash note from last time.
pub fn init(data: &Path, version: &str) {
    let _ = DATA.set(data.to_path_buf());
    let dir = logs_dir(data);
    let _ = std::fs::create_dir_all(&dir);
    let path = log_path(data);
    let f = rotate(data).or_else(|| std::fs::OpenOptions::new().create(true).append(true).open(&path).ok());
    if let Some(f) = f {
        *LOG.lock_ok() = Some(f);
    }
    log::info!("Pixie {version} starting");
    let crash = dir.join("crash.txt");
    if let Ok(text) = std::fs::read_to_string(&crash) {
        let first = text.lines().find(|l| l.starts_with("message:")).unwrap_or("").trim_start_matches("message:").trim().to_string();
        note("crash", &format!("Pixie closed unexpectedly: {first}"), "Restarted", &text.chars().take(1500).collect::<String>());
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let _ = std::fs::rename(&crash, dir.join(format!("crash-{stamp}.txt")));
        prune_crashes(&dir);
        notice("Pixie closed unexpectedly last time, so I restarted. Say “report this” to tell the developers.");
    }
    let version = version.to_string();
    let dir2 = dir.clone();
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let bt = std::backtrace::Backtrace::force_capture();
        let text = format!(
            "time: {}\nversion: {version}\nthread: {thread}\nmessage: {message}\nat: {at}\n\n{bt}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        // Only the first panic of a run is the interesting one.
        if !dir2.join("crash.txt").exists() {
            let _ = std::fs::write(dir2.join("crash.txt"), &text);
        }
        log::error!("panic in {thread} at {at}: {message}");
        prev(info);
    }));
}

fn prune_crashes(dir: &Path) {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-")))
        .collect();
    v.sort();
    let extra = v.len().saturating_sub(5);
    for p in v.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
}

/// Something to say once the notch is up.
pub fn notice(text: &str) {
    if let Ok(mut n) = NOTICES.lock() {
        if !n.iter().any(|t| t == text) {
            n.push(text.to_string());
        }
    }
}

pub fn take_notices() -> Vec<String> {
    NOTICES.lock().map(|mut n| std::mem::take(&mut *n)).unwrap_or_default()
}

// ---- last-good copies ------------------------------------------------------------------------

fn last_good(path: &Path) -> PathBuf {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    path.with_file_name(format!("{name}.last-good"))
}

/// Write `text` safely (a temporary file, then a rename) and keep it as the last-good copy.
pub fn save_safely(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp-write");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    let _ = std::fs::write(last_good(path), text);
    Ok(())
}

/// Read and parse a file Pixie can't lose. A file that's there but won't parse is set aside
/// (`.damaged`) and the last-good copy is used instead, and they're told. None when there's
/// neither (first run).
pub fn load_safely<T: serde::de::DeserializeOwned>(path: &Path, what: &str) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<T>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            let backup = last_good(path);
            let restored = std::fs::read_to_string(&backup).ok().and_then(|t| serde_json::from_str::<T>(&t).ok().map(|v| (v, t)));
            let _ = std::fs::rename(path, path.with_extension("damaged"));
            match restored {
                Some((v, t)) => {
                    let _ = std::fs::write(path, t);
                    note("file", &format!("{what} couldn't be read ({e})"), "Restored the last good copy; the damaged one is kept as .damaged", "");
                    notice(&format!("Your {what} file was damaged, so I put back the last good copy."));
                    Some(v)
                }
                None => {
                    note("file", &format!("{what} couldn't be read ({e})"), "No good copy; started fresh. The damaged one is kept as .damaged", "");
                    notice(&format!("Your {what} file was damaged and there was no good copy, so I started fresh. The old one is kept."));
                    None
                }
            }
        }
    }
}

// ---- the "to fix" list -----------------------------------------------------------------------

/// Note a problem Pixie worked around, in `<app_data>/issues.md` (newest last). The same problem
/// within a day is counted, not repeated.
pub fn note(kind: &str, what: &str, workaround: &str, detail: &str) {
    let Some(data) = DATA.get() else { return };
    note_in(data, kind, what, workaround, detail);
}

fn note_in(data: &Path, kind: &str, what: &str, workaround: &str, detail: &str) {
    let path = issues_path(data);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut entries = parse(&text);
    let now = chrono::Local::now();
    let what = redact(what);
    if let Some(e) = entries.iter_mut().rev().find(|e| e.kind == kind && e.what == what) {
        if chrono::NaiveDateTime::parse_from_str(&e.when, "%Y-%m-%d %H:%M").map(|t| (now.naive_local() - t).num_hours() < 24).unwrap_or(false) {
            e.times += 1;
            return write_issues(&path, &entries);
        }
    }
    entries.push(Issue {
        when: now.format("%Y-%m-%d %H:%M").to_string(),
        kind: kind.into(),
        what,
        workaround: workaround.into(),
        detail: redact(detail).chars().take(1500).collect(),
        times: 1,
        version: env!("CARGO_PKG_VERSION").into(),
    });
    let extra = entries.len().saturating_sub(ISSUES_MAX);
    entries.drain(..extra);
    log::info!("heal: {kind}: {}", entries.last().map(|e| e.what.as_str()).unwrap_or(""));
    write_issues(&path, &entries);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub when: String,
    pub kind: String,
    pub what: String,
    pub workaround: String,
    pub detail: String,
    pub times: u32,
    pub version: String,
}

fn write_issues(path: &Path, entries: &[Issue]) {
    let mut s = String::from("# Problems Pixie worked around\n\nKept on this computer only. Say “report this” to Pixie to open a GitHub issue about one.\n");
    for e in entries {
        s.push_str(&format!("\n## {} · {} · {}\n- What: {}\n- Workaround: {}\n", e.when, e.kind, e.version, e.what, e.workaround));
        if e.times > 1 {
            s.push_str(&format!("- Times: {}\n", e.times));
        }
        if !e.detail.trim().is_empty() {
            s.push_str("- Detail:\n```\n");
            s.push_str(e.detail.trim());
            s.push_str("\n```\n");
        }
    }
    let _ = std::fs::write(path, s);
}

fn parse(text: &str) -> Vec<Issue> {
    let mut out: Vec<Issue> = vec![];
    let mut in_detail = false;
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            let p: Vec<&str> = h.split(" · ").collect();
            out.push(Issue {
                when: p.first().unwrap_or(&"").to_string(),
                kind: p.get(1).unwrap_or(&"").to_string(),
                version: p.get(2).unwrap_or(&"").to_string(),
                what: String::new(),
                workaround: String::new(),
                detail: String::new(),
                times: 1,
            });
            in_detail = false;
            continue;
        }
        let Some(e) = out.last_mut() else { continue };
        if in_detail {
            if line.starts_with("```") {
                in_detail = false;
            } else {
                e.detail.push_str(line);
                e.detail.push('\n');
            }
        } else if let Some(v) = line.strip_prefix("- What: ") {
            e.what = v.to_string();
        } else if let Some(v) = line.strip_prefix("- Workaround: ") {
            e.workaround = v.to_string();
        } else if let Some(v) = line.strip_prefix("- Times: ") {
            e.times = v.trim().parse().unwrap_or(1);
        } else if line.starts_with("```") {
            in_detail = true;
        }
    }
    out
}

pub fn recent(data: &Path, n: usize) -> Vec<Issue> {
    let mut v = parse(&std::fs::read_to_string(issues_path(data)).unwrap_or_default());
    let skip = v.len().saturating_sub(n);
    v.drain(..skip);
    v
}

/// What a model error means, in their words, and what Pixie does about it.
pub fn classify(err: &str) -> Option<(&'static str, &'static str)> {
    let e = err.to_lowercase();
    let has = |k: &[&str]| k.iter().any(|w| e.contains(w));
    if has(&["401", "invalid api key", "invalid x-api-key", "authentication", "unauthorized", "incorrect api key"]) {
        Some(("The model key was refused", "Check the key in Settings → Models"))
    } else if has(&["402", "insufficient", "credit", "balance", "billing", "quota"]) && !has(&["free usage limit"]) {
        Some(("The model account is out of credit", "Top up with the provider, or pick another provider"))
    } else if has(&["429", "rate limit", "too many requests", "usage limit", "overloaded", "529"]) {
        Some(("The model service is busy or rate-limited", "Retried, then used the backup model if one is set"))
    } else if has(&["model_not_found", "does not exist", "unknown model", "no such model", "model not found", "invalid model"]) {
        Some(("The chosen model doesn't exist (any more)", "Switched to the newest model of the same family (or the default)"))
    } else if has(&["timed out", "timeout", "connection", "dns", "network", "error sending request"]) {
        Some(("Couldn't reach the model service", "Check the internet connection; it retried"))
    } else {
        None
    }
}

// ---- reports ---------------------------------------------------------------------------------

/// Personal paths, keys, tokens and email addresses out of text that may be shared.
/// The home folder, whichever variable the platform sets.
fn home_dir() -> String {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default()
}

pub fn redact(s: &str) -> String {
    let mut t = s.to_string();
    // HOME on the Mac and Linux, USERPROFILE (C:\Users\name) on Windows.
    for home in [home_dir(), std::env::var("USERPROFILE").unwrap_or_default()] {
        if home.len() > 3 {
            t = t.replace(&home, "~");
        }
    }
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    if user.len() > 2 {
        t = t.replace(&format!("/Users/{user}"), "~").replace(&format!("/home/{user}"), "~").replace(&format!("C:\\Users\\{user}"), "~");
    }
    t.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == '(' || c == ')')
        .map(|w| {
            let core = w.trim_end_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == '(' || c == ')');
            let tail = &w[core.len()..];
            let secret = ["sk-", "sk_", "ghp_", "gho_", "github_pat_", "xoxp-", "xoxb-", "AIza", "Bearer"].iter().any(|p| core.starts_with(p) && core.len() > 12)
                || (core.len() >= 32 && core.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && core.chars().any(|c| c.is_ascii_digit()) && core.chars().any(|c| c.is_ascii_alphabetic()));
            let email = core.contains('@') && core.contains('.') && !core.starts_with('@');
            if secret {
                format!("[key removed]{tail}")
            } else if email {
                format!("[email removed]{tail}")
            } else {
                w.to_string()
            }
        })
        .collect()
}

fn log_tail(data: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(log_path(data)).unwrap_or_default();
    let v: Vec<&str> = text.lines().filter(|l| l.contains(" WARN ") || l.contains(" ERROR ") || l.contains("heal:") || l.contains("panic")).collect();
    let skip = v.len().saturating_sub(lines);
    redact(&v[skip..].join("\n"))
}

/// A GitHub issue draft: title and body (Markdown), personal details removed.
pub fn draft(data: &Path, os: &str, what: &str) -> (String, String) {
    let issues = recent(data, 3);
    let main = issues.last();
    let title = if !what.trim().is_empty() {
        what.trim().chars().take(90).collect::<String>()
    } else {
        main.map(|i| i.what.chars().take(90).collect::<String>()).unwrap_or_else(|| "Something went wrong".into())
    };
    let mut body = String::new();
    if !what.trim().is_empty() {
        body.push_str(&format!("**What happened**\n{}\n\n", redact(what.trim())));
    }
    if !issues.is_empty() {
        body.push_str("**What Pixie noticed**\n");
        for i in issues.iter().rev() {
            body.push_str(&format!("- {} ({}, {}x): {}. Workaround: {}\n", i.when, i.kind, i.times, i.what, i.workaround));
        }
        body.push('\n');
    }
    body.push_str(&format!("**Version** Pixie {} on {os}\n\n", env!("CARGO_PKG_VERSION")));
    let detail = main.map(|i| i.detail.trim().to_string()).unwrap_or_default();
    let tail = log_tail(data, 20);
    let mut logs = String::new();
    if !detail.is_empty() {
        logs.push_str(&detail);
        logs.push('\n');
    }
    logs.push_str(&tail);
    if !logs.trim().is_empty() {
        // Keep the URL short enough for browsers.
        let logs: String = logs.chars().rev().take(3500).collect::<String>().chars().rev().collect();
        body.push_str(&format!("**Log (personal paths, keys and emails removed)**\n```\n{}\n```\n", logs.trim()));
    }
    body.push_str("\n_Drafted by Pixie. Please check it before you submit._\n");
    (title, body)
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn issue_url(title: &str, body: &str) -> String {
    format!("{REPO_ISSUES}?title={}&body={}", encode(title), encode(body))
}

/// Open a page in the browser they named, or their default browser.
pub fn open_in(browser: &str, url: &str) -> Result<String, String> {
    let b = browser.trim().to_lowercase();
    let app = match b.as_str() {
        "" | "default" => None,
        b if b.contains("chrome") => Some("Google Chrome"),
        b if b.contains("safari") => Some("Safari"),
        b if b.contains("firefox") => Some("Firefox"),
        b if b.contains("arc") => Some("Arc"),
        b if b.contains("brave") => Some("Brave Browser"),
        b if b.contains("edge") => Some("Microsoft Edge"),
        b if b.contains("opera") => Some("Opera"),
        b if b.contains("vivaldi") => Some("Vivaldi"),
        b if b.contains("dia") => Some("Dia"),
        _ => None,
    };
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        if let Some(a) = app {
            c.args(["-a", a]);
        }
        let ok = c.arg(url).status().map(|s| s.success()).unwrap_or(false);
        if !ok && app.is_some() {
            // Not installed: the default browser.
            let _ = std::process::Command::new("open").arg(url).status();
            return Ok("default browser".into());
        }
        Ok(app.unwrap_or("default browser").to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().map_err(|e| e.to_string())?;
        Ok("default browser".into())
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = app;
        std::process::Command::new("xdg-open").arg(url).spawn().map_err(|e| e.to_string())?;
        Ok("default browser".into())
    }
}

// ---- checkup ---------------------------------------------------------------------------------

pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn free_disk_gb(data: &Path) -> Option<f64> {
    let out = std::process::Command::new("df").args(["-k", &data.display().to_string()]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let avail: f64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail / 1024.0 / 1024.0)
}

pub fn disk_check(data: &Path) -> Check {
    match free_disk_gb(data) {
        Some(gb) => Check { name: "Disk space", ok: gb > 2.0, detail: format!("{gb:.1} GB free") },
        None => Check { name: "Disk space", ok: true, detail: "couldn't tell".into() },
    }
}

pub fn summary(checks: &[Check]) -> String {
    let bad: Vec<&Check> = checks.iter().filter(|c| !c.ok).collect();
    let mut s = String::new();
    for c in checks {
        s.push_str(&format!("{} {}: {}\n", if c.ok { "ok" } else { "PROBLEM" }, c.name, c.detail));
    }
    s.push_str(&if bad.is_empty() { "Everything checked out.".to_string() } else { format!("{} problem(s).", bad.len()) });
    s
}

pub fn value_of(checks: &[Check]) -> Value {
    Value::Array(checks.iter().map(|c| serde_json::json!({ "name": c.name, "ok": c.ok, "detail": c.detail })).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issues_are_kept_counted_and_read_back() {
        let d = tempfile::tempdir().unwrap();
        note_in(d.path(), "model", "Model gone", "Went back to the default", "log line /x");
        note_in(d.path(), "model", "Model gone", "Went back to the default", "");
        note_in(d.path(), "file", "settings couldn't be read", "Restored", "");
        let v = recent(d.path(), 10);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].times, 2);
        assert_eq!(v[0].detail.trim(), "log line /x");
        assert_eq!(v[1].kind, "file");
    }

    #[test]
    fn last_good_copy_comes_back() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        save_safely(&p, r#"{"a": 1}"#).unwrap();
        std::fs::write(&p, "{\"a\": 1, broken").unwrap();
        let v: Value = load_safely(&p, "settings").unwrap();
        assert_eq!(v["a"], 1);
        assert!(p.with_extension("damaged").exists());
        let back: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(back["a"], 1);
        assert!(take_notices().iter().any(|n| n.contains("last good copy")));
    }

    #[test]
    fn secrets_and_paths_are_removed() {
        let home = home_dir();
        assert!(!home.is_empty(), "no home folder in the environment");
        let s = redact(&format!("key sk-ant-api03-abcdefghijklmnop at {home}/Library/x, mail me@example.com (token ghp_ABCDEFGHIJKLMNOPQRSTUV)"));
        assert!(!s.contains("sk-ant") && !s.contains("ghp_") && !s.contains("me@example.com") && !s.contains(&home), "{s}");
        assert!(s.contains("~/Library/x"));
        assert!(s.contains("[key removed]") && s.contains("[email removed]"));
    }

    #[test]
    fn model_errors_are_explained() {
        assert_eq!(classify("401 Unauthorized: invalid x-api-key").unwrap().0, "The model key was refused");
        assert_eq!(classify("HTTP 429 Too Many Requests").unwrap().0, "The model service is busy or rate-limited");
        assert!(classify("model_not_found: deepseek-4.1").unwrap().0.contains("doesn't exist"));
        assert!(classify("all good").is_none());
    }

    #[test]
    fn report_urls_are_encoded() {
        let u = issue_url("A & B", "line 1\nline #2");
        assert!(u.starts_with(REPO_ISSUES));
        assert!(u.contains("title=A%20%26%20B") && u.contains("line%201%0Aline%20%232"));
    }
}
