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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Coder {
    pub name: &'static str,
    pub label: &'static str,
    pub path: PathBuf,
}

const KNOWN: [(&str, &str); 3] = [("codex", "Codex CLI"), ("claude", "Claude Code"), ("opencode", "opencode")];

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("PATH").unwrap_or_default().split(':').map(PathBuf::from).collect();
    let home = guard::home();
    for d in ["/opt/homebrew/bin", "/usr/local/bin"] {
        dirs.push(PathBuf::from(d));
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
            dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()).map(|path| Coder { name, label, path })
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
    let wanted = input.get("agent").and_then(Value::as_str).filter(|a| *a != "auto").unwrap_or(or(&prefs.agent, "auto"));
    let coder = coders.iter().find(|c| c.name == wanted).or(coders.first()).cloned().unwrap();
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
    let mut cmd = command(&coder, prefs, &task, &dir, &last_message);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null()).kill_on_drop(true);
    let path = format!("/opt/homebrew/bin:/usr/local/bin:{}", std::env::var("PATH").unwrap_or_default());
    cmd.env("PATH", path);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return ToolOutput::err(format!("Couldn't start {}: {e}", coder.label)),
    };
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
    let ok = status.map(|s| s.success()).unwrap_or(false);
    let summary = std::fs::read_to_string(&last_message).unwrap_or_default();
    let body = if summary.trim().is_empty() { tail(&log, 20_000) } else { format!("{}\n\n(log tail)\n{}", summary.trim(), tail(&log, 4000)) };
    let text = format!("{} worked in {} for {}s and {}.\n\n{}", coder.label, dir.display(), started.elapsed().as_secs(), if ok { "finished" } else { "exited with an error" }, body);
    if ok { ToolOutput::ok(text) } else { ToolOutput::err(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
