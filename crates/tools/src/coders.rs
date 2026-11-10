//! Local coding agents a job can hand work to: Codex CLI, Claude Code, opencode.
//! They run non-interactively in a folder; their file edits are the deliverable.
//! (T3 Code is a GUI over these same agents, so it's driven through them.)

use crate::{guard, ToolCtx, ToolOutput};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// What the user picked in Settings → Coding.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// "auto", "codex", "claude" or "opencode".
    pub agent: String,
    /// Per-agent model, e.g. {"codex": "gpt-5-codex"}; empty = the agent's own default.
    pub models: std::collections::BTreeMap<String, String>,
    /// Codex sandbox: read-only, workspace-write, danger-full-access.
    pub codex_sandbox: String,
    /// Claude Code permission mode: acceptEdits, bypassPermissions, plan.
    pub claude_permission: String,
    /// Default folder when a job doesn't name one.
    pub dir: String,
    /// What they say is left on each paid agent's plan (Settings → Coding), per agent name.
    pub limits: std::collections::BTreeMap<String, Limit>,
}

/// Usage left on a plan, per window: "" (not set), "ok", "low" or "out".
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Limit {
    pub five_hour: String,
    pub weekly: String,
}

/// How much of a plan's windows is used, read from the agent itself (Codex writes it to its
/// session logs): percent used and when each window resets (unix seconds).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Usage {
    pub five_hour_used: Option<f64>,
    pub five_hour_resets: Option<i64>,
    pub weekly_used: Option<f64>,
    pub weekly_resets: Option<i64>,
}

/// Codex's latest rate-limit reading from `~/.codex/sessions` (no request made).
pub fn codex_usage() -> Option<Usage> {
    let root = guard::home().join(".codex/sessions");
    // sessions/YYYY/MM/DD/rollout-….jsonl: walk to the newest day, then the newest file.
    let newest_dir = |d: &std::path::Path| -> Option<PathBuf> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        v.sort();
        v.pop()
    };
    let day = newest_dir(&newest_dir(&newest_dir(&root)?)?)?;
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&day)
        .ok()?
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(_, p)| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    for (_, f) in files.iter().rev().take(3) {
        let text = std::fs::read_to_string(f).ok()?;
        for line in text.lines().rev().filter(|l| l.contains("\"rate_limits\"")) {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if let Some(rl) = find_key(&v, "rate_limits") {
                return Some(usage_from(rl));
            }
        }
    }
    None
}

fn find_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(key).or_else(|| m.values().find_map(|x| find_key(x, key))),
        Value::Array(a) => a.iter().find_map(|x| find_key(x, key)),
        _ => None,
    }
}

fn usage_from(rl: &Value) -> Usage {
    let now = chrono_now();
    let window = |w: &Value| -> (Option<f64>, Option<i64>) {
        let resets = w["resets_at"].as_i64();
        // A window that has reset since the reading starts empty again.
        let used = w["used_percent"].as_f64().map(|u| if resets.is_some_and(|r| r <= now) { 0.0 } else { u });
        (used, resets)
    };
    let (fu, fr) = window(&rl["primary"]);
    let (wu, wr) = window(&rl["secondary"]);
    Usage { five_hour_used: fu, five_hour_resets: fr, weekly_used: wu, weekly_resets: wr }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// How usable an agent is now: 2 plenty, 1 running low, 0 out. opencode's free models count as
/// plenty but come last in the order.
pub fn headroom(name: &str, prefs: &Prefs, usage: Option<&Usage>) -> u8 {
    let mut level = 2u8;
    if let Some(l) = prefs.limits.get(name) {
        for w in [&l.five_hour, &l.weekly] {
            level = level.min(match w.as_str() {
                "out" => 0,
                "low" => 1,
                _ => 2,
            });
        }
    }
    if let Some(u) = usage {
        for used in [u.five_hour_used, u.weekly_used].into_iter().flatten() {
            level = level.min(if used >= 95.0 { 0 } else if used >= 80.0 { 1 } else { 2 });
        }
    }
    level
}

/// The agent for a task nobody named: their default first, then Codex, Claude Code, opencode;
/// skipping one that's out of usage and preferring one that isn't running low. Falls back to
/// opencode when the paid ones are out.
pub fn choose<'a>(coders: &'a [Coder], prefs: &Prefs, usage_of: &dyn Fn(&str) -> Option<Usage>) -> Option<(&'a Coder, String)> {
    let mut order: Vec<&Coder> = Vec::new();
    if let Some(c) = coders.iter().find(|c| c.name == prefs.agent) {
        order.push(c);
    }
    for name in ["codex", "claude", "opencode"] {
        if let Some(c) = coders.iter().find(|c| c.name == name && !order.iter().any(|o| o.name == name)) {
            order.push(c);
        }
    }
    let rated: Vec<(&Coder, u8, String)> = order
        .into_iter()
        .map(|c| {
            let u = usage_of(c.name);
            let h = headroom(c.name, prefs, u.as_ref());
            let why = match (&u, h) {
                (Some(u), _) => format!(
                    "{}: 5-hour {}, weekly {}",
                    c.label,
                    u.five_hour_used.map(|x| format!("{x:.0}% used")).unwrap_or_else(|| "?".into()),
                    u.weekly_used.map(|x| format!("{x:.0}% used")).unwrap_or_else(|| "?".into())
                ),
                (None, 0) => format!("{}: out of usage (Settings → Coding)", c.label),
                (None, 1) => format!("{}: running low (Settings → Coding)", c.label),
                _ => format!("{}: no limit known", c.label),
            };
            (c, h, why)
        })
        .collect();
    let notes = rated.iter().map(|r| r.2.clone()).collect::<Vec<_>>().join("; ");
    rated
        .iter()
        .find(|r| r.1 == 2)
        .or_else(|| rated.iter().find(|r| r.1 == 1))
        .map(|r| (r.0, notes.clone()))
        .or_else(|| rated.iter().find(|r| r.0.name == "opencode").map(|r| (r.0, notes.clone())))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Coder {
    pub name: &'static str,
    pub label: &'static str,
    pub path: PathBuf,
}

const KNOWN: [(&str, &str); 3] = [("codex", "Codex CLI"), ("claude", "Claude Code"), ("opencode", "opencode")];

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let home = guard::home();
    for d in ["/opt/homebrew/bin", "/usr/local/bin"] {
        dirs.push(PathBuf::from(d));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    for d in [".local/bin", ".npm-global/bin", ".bun/bin", ".opencode/bin", ".claude/local"] {
        dirs.push(home.join(d));
    }
    dirs
}

/// Agents installed on this Mac, in preference order.
pub fn detect() -> Vec<Coder> {
    let dirs = search_dirs();
    KNOWN
        .iter()
        .filter_map(|(name, label)| {
            let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ""] } else { &[""] };
            dirs.iter().flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}")))).find(|p| p.is_file()).map(|path| Coder { name, label, path })
        })
        .collect()
}

fn or<'a>(s: &'a str, d: &'a str) -> &'a str {
    if s.trim().is_empty() { d } else { s }
}

/// Command line for one agent run.
pub fn command(coder: &Coder, prefs: &Prefs, task: &str, dir: &std::path::Path, last_message: &std::path::Path) -> Command {
    let mut cmd = Command::new(&coder.path);
    let model = prefs.models.get(coder.name).map(|m| m.trim()).filter(|m| !m.is_empty());
    match coder.name {
        "codex" => {
            cmd.args(["exec", "-s", or(&prefs.codex_sandbox, "workspace-write"), "--skip-git-repo-check"]);
            if let Some(m) = model {
                cmd.args(["-m", m]);
            }
            cmd.arg("-C").arg(dir).arg("-o").arg(last_message).arg(task);
        }
        "claude" => {
            cmd.args(["-p", task, "--permission-mode", or(&prefs.claude_permission, "acceptEdits"), "--output-format", "text"]);
            if let Some(m) = model {
                cmd.args(["--model", m]);
            }
            cmd.args(["--allowedTools", "Bash", "Read", "Edit", "Write", "Glob", "Grep"]);
        }
        _ => {
            cmd.args(["run", "--auto"]);
            if let Some(m) = model {
                cmd.args(["-m", m]);
            }
            cmd.arg(task);
        }
    }
    cmd.current_dir(dir);
    cmd
}

/// `<agent> --version`, for Settings.
pub fn version(c: &Coder) -> String {
    std::process::Command::new(&c.path)
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().lines().next().unwrap_or("").to_string())
        .unwrap_or_default()
}

fn tail(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let start = (s.len() - n..s.len()).find(|i| s.is_char_boundary(*i)).unwrap_or(0);
    format!("…{}", &s[start..])
}

/// The `code` tool.
pub async fn run(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let task = input.get("task").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if task.is_empty() {
        return ToolOutput::err("Give the coding agent a task.");
    }
    let coders = detect();
    if coders.is_empty() {
        return ToolOutput::err("No local coding agent is installed (Codex CLI, Claude Code or opencode). Do the work with bash/write instead.");
    }
    let prefs = &ctx.coding;
    // Named in the task: that one. Otherwise the one with usage left (see `choose`).
    let named = input.get("agent").and_then(Value::as_str).filter(|a| *a != "auto" && !a.is_empty());
    let (coder, why) = match named.and_then(|n| coders.iter().find(|c| c.name == n)) {
        Some(c) => (c.clone(), format!("{} (asked for)", c.label)),
        None => match choose(&coders, prefs, &|name| if name == "codex" { codex_usage() } else { None }) {
            Some((c, notes)) => (c.clone(), notes),
            None => {
                let c = coders.first().cloned().unwrap();
                (c, "every agent is out of usage; trying anyway".into())
            }
        },
    };
    let dir = match input.get("dir").and_then(Value::as_str).filter(|d| !d.trim().is_empty()) {
        Some(d) => {
            let p = guard::expand(d);
            let p = if p.is_absolute() { p } else { ctx.cwd.join(p) };
            match guard::check_path(&p) {
                Ok(p) => p,
                Err(e) => return ToolOutput::err(e.to_string()),
            }
        }
        None if !prefs.dir.trim().is_empty() => guard::expand(&prefs.dir),
        None => ctx.cwd.join("output"),
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return ToolOutput::err(format!("Couldn't use {}: {e}", dir.display()));
    }
    let minutes = input.get("timeout_minutes").and_then(Value::as_u64).unwrap_or(20).clamp(1, 120);
    let last_message = ctx.cwd.join("tmp").join(format!("{}-last.txt", coder.name));
    let _ = std::fs::create_dir_all(ctx.cwd.join("tmp"));
    // Paths in the brief are relative to where the agent runs; say so, or "output/x" ends up in
    // output/output/x.
    let task = format!("You are working in {} (your current folder). Paths in this task are relative to it; don't add the folder's own name again.\n\n{task}", dir.display());
    let mut cmd = command(&coder, prefs, &task, &dir, &last_message);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let base = std::env::var_os("PATH").unwrap_or_default();
    let extra = if cfg!(target_os = "macos") { vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")] } else { vec![] };
    if let Ok(path) = std::env::join_paths(extra.into_iter().chain(std::env::split_paths(&base))) {
        cmd.env("PATH", path);
    }
    crate::children::prepare(&mut cmd);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return ToolOutput::err(format!("Couldn't start {}: {e}", coder.label)),
    };
    // Ends the agent and everything it started if this returns early (stopped, timed out).
    let guard = crate::children::add(child.id());
    let _ = ctx.host.call(ctx, "progress", &json!({ "note": format!("{} is coding", coder.label) })).await;
    let mut out = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut err = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut log = String::new();
    let started = Instant::now();
    let mut last_note = Instant::now();
    let deadline = tokio::time::sleep(Duration::from_secs(minutes * 60));
    tokio::pin!(deadline);
    let status = loop {
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                let _ = child.kill().await;
                return ToolOutput::err("Stopped.");
            }
            _ = &mut deadline => {
                let _ = child.kill().await;
                return ToolOutput::err(format!("{} ran past {minutes} min and was stopped. Output so far:\n{}", coder.label, tail(&log, 8000)));
            }
            line = out.next_line() => match line {
                Ok(Some(l)) => {
                    log.push_str(&l);
                    log.push('\n');
                    if last_note.elapsed() > Duration::from_secs(4) && !l.trim().is_empty() {
                        last_note = Instant::now();
                        let short: String = l.trim().chars().take(60).collect();
                        let _ = ctx.host.call(ctx, "progress", &json!({ "note": format!("{}: {short}", coder.label) })).await;
                    }
                }
                _ => break child.wait().await,
            },
            line = err.next_line() => {
                if let Ok(Some(l)) = line { log.push_str(&l); log.push('\n'); }
            }
        }
    };
    guard.done();
    let ok = status.map(|s| s.success()).unwrap_or(false);
    let summary = std::fs::read_to_string(&last_message).unwrap_or_default();
    let body = if summary.trim().is_empty() { tail(&log, 20_000) } else { format!("{}\n\n(log tail)\n{}", summary.trim(), tail(&log, 4000)) };
    let text = format!(
        "{} worked in {} for {}s and {}. (Chosen: {why}.)\n\n{}",
        coder.label,
        dir.display(),
        started.elapsed().as_secs(),
        if ok { "finished" } else { "exited with an error" },
        body
    );
    if ok { ToolOutput::ok(text) } else { ToolOutput::err(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_agent_with_usage_left() {
        let coders = vec![
            Coder { name: "codex", label: "Codex CLI", path: "/x".into() },
            Coder { name: "claude", label: "Claude Code", path: "/y".into() },
            Coder { name: "opencode", label: "opencode", path: "/z".into() },
        ];
        let mut p = Prefs::default();
        let busy = |used: f64| Some(Usage { five_hour_used: Some(used), five_hour_resets: Some(i64::MAX), weekly_used: Some(10.0), weekly_resets: Some(i64::MAX) });
        assert_eq!(choose(&coders, &p, &|_| None).unwrap().0.name, "codex");
        // Codex out (from its logs): Claude Code.
        assert_eq!(choose(&coders, &p, &|n| if n == "codex" { busy(99.0) } else { None }).unwrap().0.name, "claude");
        // Claude Code out too (from Settings): opencode.
        p.limits.insert("claude".into(), Limit { five_hour: "ok".into(), weekly: "out".into() });
        assert_eq!(choose(&coders, &p, &|n| if n == "codex" { busy(99.0) } else { None }).unwrap().0.name, "opencode");
        // Their default comes first when it has usage.
        p.agent = "claude".into();
        p.limits.clear();
        assert_eq!(choose(&coders, &p, &|_| None).unwrap().0.name, "claude");
        // Running low loses to plenty.
        p.limits.insert("claude".into(), Limit { five_hour: "low".into(), weekly: String::new() });
        assert_eq!(choose(&coders, &p, &|_| None).unwrap().0.name, "codex");
    }

    #[test]
    #[ignore]
    fn print_codex_usage_here() {
        println!("{:?}", codex_usage());
    }

    #[test]
    fn codex_limits_parse() {
        let v: Value = serde_json::from_str(r#"{"payload":{"rate_limits":{"primary":{"used_percent":11.0,"window_minutes":300,"resets_at":99999999999},"secondary":{"used_percent":7.0,"resets_at":1}}}}"#).unwrap();
        let u = usage_from(find_key(&v, "rate_limits").unwrap());
        assert_eq!(u.five_hour_used, Some(11.0));
        assert_eq!(u.weekly_used, Some(0.0), "a window that already reset counts as unused");
    }

    #[test]
    fn command_lines() {
        let c = Coder { name: "codex", label: "Codex CLI", path: "/x/codex".into() };
        let p = Prefs::default();
        let cmd = command(&c, &p, "add tests", std::path::Path::new("/repo"), std::path::Path::new("/tmp/l.txt"));
        let args: Vec<String> = cmd.as_std().get_args().map(|a| a.to_string_lossy().into()).collect();
        assert_eq!(args, ["exec", "-s", "workspace-write", "--skip-git-repo-check", "-C", "/repo", "-o", "/tmp/l.txt", "add tests"]);
        let mut p2 = Prefs { codex_sandbox: "read-only".into(), ..Default::default() };
        p2.models.insert("codex".into(), "gpt-x".into());
        let args: Vec<String> = command(&c, &p2, "t", std::path::Path::new("/r"), std::path::Path::new("/l")).as_std().get_args().map(|a| a.to_string_lossy().into()).collect();
        assert_eq!(&args[..6], ["exec", "-s", "read-only", "--skip-git-repo-check", "-m", "gpt-x"]);
        let c = Coder { name: "claude", label: "Claude Code", path: "/x/claude".into() };
        let args: Vec<String> = command(&c, &p, "t", std::path::Path::new("/r"), std::path::Path::new("/l")).as_std().get_args().map(|a| a.to_string_lossy().into()).collect();
        assert_eq!(&args[..4], ["-p", "t", "--permission-mode", "acceptEdits"]);
    }
}
