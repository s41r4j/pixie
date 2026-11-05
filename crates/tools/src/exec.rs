//! Tool implementations. Every path from the model passes `guard::check_path` first.

use crate::{guard, notes, web, ToolCtx, ToolOutput};
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

const BASH_TIMEOUT: u64 = 300;
const OUTPUT_CAP: usize = 50 * 1024;
const READ_CAP: usize = 120_000;

fn s<'a>(input: &'a Value, key: &str) -> &'a str {
    input.get(key).and_then(Value::as_str).unwrap_or("")
}

fn resolve(ctx: &ToolCtx, path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("No path given.".into());
    }
    let p = guard::expand(path);
    let p = if p.is_absolute() { p } else { ctx.cwd.join(p) };
    guard::check_path(&p).map_err(|e| e.to_string())
}

/// Writes stay inside the working folder (or a temp folder); the Mac is reached via device_save_file.
fn resolve_writable(ctx: &ToolCtx, path: &str) -> Result<PathBuf, String> {
    let p = resolve(ctx, path)?;
    let cwd = guard::normalize(&ctx.cwd);
    let ok = p.starts_with(&cwd) || p.starts_with("/tmp") || p.starts_with("/private/tmp") || p.starts_with("/var/folders");
    if !ok {
        return Err(format!(
            "{} is outside the working folder. Write it under output/ and use device_save_file to put it on their Mac.",
            p.display()
        ));
    }
    Ok(p)
}

fn cap(text: String, limit: usize) -> String {
    if text.len() <= limit {
        return text;
    }
    let head_end = (0..=limit * 2 / 5).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
    let tail_start = (text.len() - limit * 3 / 5..text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(text.len());
    format!(
        "{}\n…[{} bytes cut]…\n{}",
        &text[..head_end],
        tail_start - head_end,
        &text[tail_start..]
    )
}

fn path_env(ctx: &ToolCtx) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some(dir) = ctx.helper.as_ref().and_then(|h| h.parent()) {
        parts.push(dir.display().to_string());
    }
    parts.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(String::from));
    parts.push(std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()));
    parts.join(":")
}

async fn run_process(mut cmd: Command, timeout: Duration, ctx: &ToolCtx) -> Result<(i32, String), String> {
    cmd.kill_on_drop(true).stdin(std::process::Stdio::null());
    let child = cmd.output();
    tokio::select! {
        _ = ctx.cancel.cancelled() => Err("Stopped.".into()),
        r = tokio::time::timeout(timeout, child) => match r {
            Err(_) => Err(format!("Timed out after {} s.", timeout.as_secs())),
            Ok(Err(e)) => Err(format!("Couldn't run it: {e}")),
            Ok(Ok(out)) => {
                let mut text = String::from_utf8_lossy(&out.stdout).to_string();
                let err = String::from_utf8_lossy(&out.stderr);
                if !err.trim().is_empty() {
                    if !text.is_empty() { text.push('\n'); }
                    text.push_str(&err);
                }
                Ok((out.status.code().unwrap_or(-1), text))
            }
        }
    }
}

async fn bash(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let command = s(input, "command");
    if command.trim().is_empty() {
        return ToolOutput::err("No command given.");
    }
    if let Err(e) = guard::check_command(command) {
        return ToolOutput::err(e.to_string());
    }
    let timeout = input.get("timeout").and_then(Value::as_u64).unwrap_or(BASH_TIMEOUT).clamp(1, 1800);
    let _ = std::fs::create_dir_all(&ctx.cwd);
    let mut cmd = Command::new("/bin/bash");
    cmd.arg("-c").arg(command).current_dir(&ctx.cwd).env("PATH", path_env(ctx)).env("PIP_DISABLE_PIP_VERSION_CHECK", "1");
    if let Some(h) = &ctx.helper {
        cmd.env("OPENCLICKY_HELPER", h);
    }
    match run_process(cmd, Duration::from_secs(timeout), ctx).await {
        Ok((code, out)) => {
            let out = cap(out, OUTPUT_CAP);
            let body = if out.trim().is_empty() { "(no output)".to_string() } else { out };
            if code == 0 { ToolOutput::ok(body) } else { ToolOutput::err(format!("exit {code}\n{body}")) }
        }
        Err(e) => ToolOutput::err(e),
    }
}

fn read_text(path: &Path, offset: usize, limit: usize) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    if bytes.iter().take(4096).any(|b| *b == 0) {
        return Err(format!("{} is a binary file.", path.display()));
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let start = offset.saturating_sub(1).min(lines.len());
    let end = (start + limit).min(lines.len());
    let mut out = lines[start..end].join("\n");
    if end < lines.len() {
        out.push_str(&format!("\n…[lines {}–{} of {}; read on with offset={}]", start + 1, end, lines.len(), end + 1));
    }
    Ok(cap(out, READ_CAP))
}

async fn read(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let path = match resolve(ctx, s(input, "path")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    let offset = input.get("offset").and_then(Value::as_u64).unwrap_or(1) as usize;
    let limit = input.get("limit").and_then(Value::as_u64).unwrap_or(2000) as usize;
    read_text(&path, offset, limit).into()
}

async fn write(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let path = match resolve_writable(ctx, s(input, "path")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    let content = s(input, "content");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, content) {
        Ok(()) => {
            if path == ctx.note_path {
                ctx.host.note_changed(ctx);
            }
            ToolOutput::ok(format!("Wrote {} ({} bytes).", path.display(), content.len()))
        }
        Err(e) => ToolOutput::err(format!("Couldn't write {}: {e}", path.display())),
    }
}

async fn edit(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let path = match resolve_writable(ctx, s(input, "path")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    let Ok(mut text) = std::fs::read_to_string(&path) else {
        return ToolOutput::err(format!("Couldn't read {}.", path.display()));
    };
    let edits = input.get("edits").and_then(Value::as_array).cloned().unwrap_or_default();
    if edits.is_empty() {
        return ToolOutput::err("No edits given.");
    }
    for (i, e) in edits.iter().enumerate() {
        let old = s(e, "old");
        let new = s(e, "new");
        match text.matches(old).count() {
            1 => text = text.replacen(old, new, 1),
            0 => return ToolOutput::err(format!("Edit {}: the old text isn't in the file. Nothing was changed.", i + 1)),
            n => return ToolOutput::err(format!("Edit {}: the old text appears {n} times; include more context. Nothing was changed.", i + 1)),
        }
    }
    match std::fs::write(&path, &text) {
        Ok(()) => {
            if path == ctx.note_path {
                ctx.host.note_changed(ctx);
            }
            ToolOutput::ok(format!("Applied {} edit(s) to {}.", edits.len(), path.display()))
        }
        Err(e) => ToolOutput::err(e.to_string()),
    }
}

async fn grep(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let pattern = s(input, "pattern");
    let path = match s(input, "path") {
        "" => ctx.cwd.clone(),
        p => match resolve(ctx, p) {
            Ok(p) => p,
            Err(e) => return ToolOutput::err(e),
        },
    };
    let mut cmd = Command::new("/usr/bin/grep");
    cmd.args(["-rnEI", "--exclude-dir=node_modules", "--exclude-dir=.git", "-m", "50"]);
    let glob = s(input, "glob");
    if !glob.is_empty() {
        cmd.arg(format!("--include={glob}"));
    }
    cmd.arg("-e").arg(pattern).arg(&path);
    match run_process(cmd, Duration::from_secs(60), ctx).await {
        Ok((_, out)) => {
            let lines: Vec<&str> = out.lines().filter(|l| guard::check_path(Path::new(l.split(':').next().unwrap_or(""))).is_ok()).take(200).collect();
            if lines.is_empty() { ToolOutput::ok("No matches.") } else { ToolOutput::ok(cap(lines.join("\n"), OUTPUT_CAP)) }
        }
        Err(e) => ToolOutput::err(e),
    }
}

fn glob_match(pattern: &str, name: &str) -> bool {
    // `*` only, case-insensitive.
    let p = pattern.to_lowercase();
    let n = name.to_lowercase();
    if !p.contains('*') {
        return n.contains(&p);
    }
    let parts: Vec<&str> = p.split('*').collect();
    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match n[pos..].find(part) {
            Some(at) if i > 0 || at == 0 => pos += at + part.len(),
            _ => return false,
        }
    }
    parts.last().map(|l| l.is_empty() || n.ends_with(l)).unwrap_or(true)
}

fn walk(dir: &Path, query: &str, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 || out.len() >= 200 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name == "node_modules" || name == ".git" || guard::check_path(&p).is_err() {
            continue;
        }
        if glob_match(query, &name) {
            out.push(p.clone());
        }
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            walk(&p, query, depth - 1, out);
        }
    }
}

async fn find(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let mut out = vec![];
    walk(&ctx.cwd, s(input, "query"), 12, &mut out);
    if out.is_empty() {
        return ToolOutput::ok("Nothing found.");
    }
    ToolOutput::ok(out.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n"))
}

/// Load an image as base64 JPEG/PNG, converting/downscaling with `sips` when needed.
async fn image_payload(ctx: &ToolCtx, path: &Path) -> Result<(String, String), String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let size = std::fs::metadata(path).map(|m| m.len()).map_err(|e| format!("Couldn't open {}: {e}", path.display()))?;
    let (file, media) = if (ext == "png" || ext == "jpg" || ext == "jpeg") && size < 1_500_000 {
        (path.to_path_buf(), if ext == "png" { "image/png" } else { "image/jpeg" })
    } else {
        let tmp = ctx.cwd.join("tmp");
        let _ = std::fs::create_dir_all(&tmp);
        let out = tmp.join(format!("view-{}.jpg", std::process::id() as u64 ^ size));
        let mut cmd = Command::new("/usr/bin/sips");
        cmd.args(["-s", "format", "jpeg", "-Z", "1568"]).arg(path).arg("--out").arg(&out);
        run_process(cmd, Duration::from_secs(30), ctx).await?;
        if !out.exists() {
            return Err(format!("Couldn't turn {} into an image.", path.display()));
        }
        (out, "image/jpeg")
    };
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    Ok((media.to_string(), base64::engine::general_purpose::STANDARD.encode(bytes)))
}

async fn view_image(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let path = match resolve(ctx, s(input, "path")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    match image_payload(ctx, &path).await {
        Ok(img) => ToolOutput { text: format!("{}:", path.display()), images: vec![img], is_error: false },
        Err(e) => ToolOutput::err(e),
    }
}

async fn fetch_url(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let url = s(input, "url");
    let offset = input.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let max = input.get("max_chars").and_then(Value::as_u64).unwrap_or(20_000).clamp(500, 100_000) as usize;
    let fetched = tokio::select! {
        _ = ctx.cancel.cancelled() => return ToolOutput::err("Stopped."),
        r = web::fetch(url) => r,
    };
    match fetched {
        Err(e) => ToolOutput::err(e),
        Ok(f) if f.bytes.is_some() => {
            let bytes = f.bytes.unwrap();
            let ext = if f.content_type.contains("pdf") { "pdf" } else if f.content_type.contains("png") { "png" } else if f.content_type.starts_with("image/") { "jpg" } else { "bin" };
            let name = url.rsplit('/').next().filter(|n| n.contains('.') && n.len() < 80).map(str::to_string).unwrap_or_else(|| format!("download.{ext}"));
            let dir = ctx.cwd.join("tmp");
            let _ = std::fs::create_dir_all(&dir);
            let dest = guard::unique_destination(&dir.join(name));
            if let Err(e) = std::fs::write(&dest, &bytes) {
                return ToolOutput::err(e.to_string());
            }
            let mut text = format!("Downloaded {} ({} KB) to {}.", f.content_type, bytes.len() / 1024, dest.display());
            if ext == "pdf" {
                if let Ok(t) = doc_text(ctx, &dest).await {
                    text.push_str("\n\n");
                    text.push_str(&cap(t, max));
                }
            }
            ToolOutput::ok(text)
        }
        Ok(f) => {
            let chars: Vec<char> = f.text.chars().collect();
            let start = offset.min(chars.len());
            let end = (start + max).min(chars.len());
            let mut out: String = chars[start..end].iter().collect();
            if end < chars.len() {
                out.push_str(&format!("\n…[{} more characters; continue with offset={end}]", chars.len() - end));
            }
            ToolOutput::ok(out)
        }
    }
}

async fn web_search(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let q = s(input, "query");
    let count = input.get("count").and_then(Value::as_u64).unwrap_or(6).clamp(1, 12) as usize;
    let r = tokio::select! {
        _ = ctx.cancel.cancelled() => return ToolOutput::err("Stopped."),
        r = web::search(q, count) => r,
    };
    match r {
        Ok(hits) => ToolOutput::ok(
            hits.iter()
                .enumerate()
                .map(|(i, h)| format!("{}. {}\n   {}\n   {}", i + 1, h.title, h.url, h.snippet))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Err(e) => ToolOutput::err(e),
    }
}

/// Text of PDF / Word / RTF / Pages via the Swift helper (`openclicky-helper text`) or textutil.
async fn doc_text(ctx: &ToolCtx, path: &Path) -> Result<String, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if let Some(helper) = &ctx.helper {
        let mut cmd = Command::new(helper);
        cmd.arg("text").arg(path);
        if let Ok((0, out)) = run_process(cmd, Duration::from_secs(60), ctx).await {
            if !out.trim().is_empty() {
                return Ok(out);
            }
        }
    }
    if matches!(ext.as_str(), "doc" | "docx" | "rtf" | "rtfd" | "odt" | "html" | "webarchive") {
        let mut cmd = Command::new("/usr/bin/textutil");
        cmd.args(["-convert", "txt", "-stdout"]).arg(path);
        if let Ok((0, out)) = run_process(cmd, Duration::from_secs(60), ctx).await {
            return Ok(out);
        }
    }
    Err(format!("Couldn't get text out of {}.", path.display()))
}

async fn device_list_folder(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let path = match resolve(ctx, s(input, "path")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    let rd = match std::fs::read_dir(&path) {
        Ok(rd) => rd,
        Err(e) => return ToolOutput::err(format!("Couldn't open {}: {e}", path.display())),
    };
    let mut rows: Vec<(std::time::SystemTime, String)> = rd
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| guard::check_path(&e.path()).is_ok())
        .map(|e| {
            let md = e.metadata().ok();
            let modified = md.as_ref().and_then(|m| m.modified().ok()).unwrap_or(std::time::UNIX_EPOCH);
            let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = md.map(|m| m.len()).unwrap_or(0);
            let age = modified.elapsed().map(|d| human_age(d.as_secs())).unwrap_or_default();
            let name = e.file_name().to_string_lossy().to_string();
            let line = if is_dir { format!("{name}/  ({age})") } else { format!("{name}  {}  ({age})", human_size(size)) };
            (modified, line)
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    let total = rows.len();
    let mut out: Vec<String> = rows.into_iter().take(300).map(|r| r.1).collect();
    if total > 300 {
        out.push(format!("…and {} more", total - 300));
    }
    ToolOutput::ok(format!("{} ({} items, newest first)\n{}", path.display(), total, out.join("\n")))
}

fn human_size(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{} KB", b >> 10),
        b => format!("{b} B"),
    }
}

fn human_age(secs: u64) -> String {
    match secs {
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s => format!("{} days ago", s / 86_400),
    }
}

async fn device_find_files(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let query = s(input, "query");
    if query.trim().is_empty() {
        return ToolOutput::err("No query given.");
    }
    let folder = match s(input, "folder") {
        "" => None,
        f => match resolve(ctx, f) {
            Ok(p) => Some(p),
            Err(e) => return ToolOutput::err(e),
        },
    };
    let mut cmd = Command::new("/usr/bin/mdfind");
    cmd.arg("-name").arg(query);
    if let Some(f) = &folder {
        cmd.arg("-onlyin").arg(f);
    }
    let mut hits: Vec<String> = match run_process(cmd, Duration::from_secs(20), ctx).await {
        Ok((_, out)) => out
            .lines()
            .filter(|l| l.starts_with('/'))
            .filter(|l| guard::check_path(Path::new(l)).is_ok())
            .filter(|l| !l.contains("/Library/") && !l.contains("/.Trash/"))
            .map(str::to_string)
            .take(60)
            .collect(),
        Err(_) => vec![],
    };
    if hits.is_empty() {
        let mut found = vec![];
        let roots = folder.map(|f| vec![f]).unwrap_or_else(|| ["Desktop", "Documents", "Downloads"].iter().map(|d| guard::home().join(d)).collect());
        for r in roots {
            walk(&r, query, 5, &mut found);
        }
        hits = found.iter().map(|p| p.display().to_string()).take(60).collect();
    }
    if hits.is_empty() { ToolOutput::ok(format!("No files named like \"{query}\".")) } else { ToolOutput::ok(hits.join("\n")) }
}

async fn device_read_file(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let path = match resolve(ctx, s(input, "path")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    if !path.is_file() {
        return ToolOutput::err(format!("{} isn't a file.", path.display()));
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "heic" | "gif" | "tiff" | "webp" | "bmp") {
        return match image_payload(ctx, &path).await {
            Ok(img) => ToolOutput { text: format!("{}:", path.display()), images: vec![img], is_error: false },
            Err(e) => ToolOutput::err(e),
        };
    }
    if matches!(ext.as_str(), "pdf" | "doc" | "docx" | "rtf" | "rtfd" | "pages" | "odt" | "webarchive" | "key" | "numbers") {
        return match doc_text(ctx, &path).await {
            Ok(t) => ToolOutput::ok(cap(format!("{}\n\n{t}", path.display()), READ_CAP)),
            Err(e) => copy_to_inbox(ctx, &path, &e),
        };
    }
    match read_text(&path, 1, 5000) {
        Ok(t) => ToolOutput::ok(t),
        Err(e) => copy_to_inbox(ctx, &path, &e),
    }
}

fn copy_to_inbox(ctx: &ToolCtx, path: &Path, why: &str) -> ToolOutput {
    let inbox = ctx.cwd.join("inbox");
    let _ = std::fs::create_dir_all(&inbox);
    let dest = guard::unique_destination(&inbox.join(path.file_name().unwrap_or_default()));
    match std::fs::copy(path, &dest) {
        Ok(_) => ToolOutput::ok(format!("{why} Copied it to {} so you can work on it with bash.", dest.display())),
        Err(e) => ToolOutput::err(format!("{why} Copying it failed too: {e}")),
    }
}

async fn device_save_file(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let source = match resolve(ctx, s(input, "source")) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    if !source.exists() {
        return ToolOutput::err(format!("{} doesn't exist.", source.display()));
    }
    let raw_dest = s(input, "destination");
    let mut dest = match resolve(ctx, if raw_dest.is_empty() { "~/Desktop/" } else { raw_dest }) {
        Ok(p) => p,
        Err(e) => return ToolOutput::err(e),
    };
    if dest.is_dir() || raw_dest.ends_with('/') {
        let _ = std::fs::create_dir_all(&dest);
        dest = dest.join(source.file_name().unwrap_or_default());
    }
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            return ToolOutput::err(format!("The folder {} doesn't exist.", parent.display()));
        }
    }
    // Never overwrite (enforced here, not in the prompt).
    let dest = guard::unique_destination(&dest);
    let r = if source.is_dir() {
        let mut cmd = Command::new("/bin/cp");
        cmd.arg("-R").arg(&source).arg(&dest);
        run_process(cmd, Duration::from_secs(120), ctx).await.and_then(|(c, o)| if c == 0 { Ok(()) } else { Err(o) })
    } else {
        std::fs::copy(&source, &dest).map(|_| ()).map_err(|e| e.to_string())
    };
    match r {
        Ok(()) => ToolOutput::ok(format!("Saved to {}", dest.display())),
        Err(e) => ToolOutput::err(format!("Couldn't save: {e}")),
    }
}

async fn device_open(ctx: &ToolCtx, input: &Value) -> ToolOutput {
    let target = s(input, "target").trim().to_string();
    if target.is_empty() {
        return ToolOutput::err("Nothing to open.");
    }
    let is_url = target.contains("://") || target.starts_with("mailto:");
    let mut cmd = Command::new("/usr/bin/open");
    if is_url {
        cmd.arg(&target);
    } else {
        let p = match resolve(ctx, &target) {
            Ok(p) => p,
            Err(e) => return ToolOutput::err(e),
        };
        if p.exists() {
            cmd.arg(&p);
        } else {
            cmd.arg("-a").arg(&target);
        }
    }
    match run_process(cmd, Duration::from_secs(20), ctx).await {
        Ok((0, _)) => ToolOutput::ok(format!("Opened {target}.")),
        Ok((_, out)) => ToolOutput::err(format!("Couldn't open {target}: {}", out.trim())),
        Err(e) => ToolOutput::err(e),
    }
}

fn note_op(ctx: &ToolCtx, f: impl FnOnce(&str) -> Result<String, String>) -> ToolOutput {
    let current = std::fs::read_to_string(&ctx.note_path).unwrap_or_default();
    match f(&current) {
        Ok(next) => {
            if let Some(dir) = ctx.note_path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(&ctx.note_path, &next) {
                return ToolOutput::err(e.to_string());
            }
            ctx.host.note_changed(ctx);
            ToolOutput::ok("Note updated.")
        }
        Err(e) => ToolOutput::err(e),
    }
}

fn lines_of(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        Value::String(s) => s.lines().map(str::to_string).collect(),
        _ => vec![],
    }
}

/// Run one tool. Unknown names go to the host.
pub async fn execute(ctx: &ToolCtx, name: &str, input: &Value) -> ToolOutput {
    if let Some(raw) = input.get("__invalid_json") {
        return ToolOutput::err(format!("The tool input wasn't valid JSON ({raw}). Send it again."));
    }
    match name {
        "bash" => bash(ctx, input).await,
        "read" => read(ctx, input).await,
        "write" => write(ctx, input).await,
        "edit" => edit(ctx, input).await,
        "grep" => grep(ctx, input).await,
        "find" => find(ctx, input).await,
        "view_image" => view_image(ctx, input).await,
        "fetch_url" => fetch_url(ctx, input).await,
        "web_search" => web_search(ctx, input).await,
        "device_list_folder" => device_list_folder(ctx, input).await,
        "device_find_files" => device_find_files(ctx, input).await,
        "device_read_file" => device_read_file(ctx, input).await,
        "device_save_file" => device_save_file(ctx, input).await,
        "device_open" => device_open(ctx, input).await,
        "note" => note_op(ctx, |_| Ok(format!("{}\n", s(input, "markdown").trim_end()))),
        "note_add" => note_op(ctx, |c| Ok(notes::add(c, &lines_of(input.get("lines").unwrap_or(&Value::Null))))),
        "note_answer" => note_op(ctx, |c| Ok(notes::answer(c, s(input, "line")))),
        "note_replace" => note_op(ctx, |c| notes::replace(c, s(input, "old"), s(input, "new"))),
        "note_remove" => note_op(ctx, |c| notes::remove(c, s(input, "line"))),
        "note_tick" => note_op(ctx, |c| {
            notes::tick(c, s(input, "label"), input.get("current").and_then(Value::as_bool).unwrap_or(false))
        }),
        other => match ctx.host.call(ctx, other, input).await {
            Some(out) => out,
            None => ToolOutput::err(format!("There's no tool called {other}.")),
        },
    }
}

#[allow(dead_code)]
fn _schema_ref() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CancellationToken, NoHost};
    use openclicky_core::prompt::Mode;
    use std::sync::Arc;

    fn ctx(dir: &Path) -> ToolCtx {
        ToolCtx {
            mode: Mode::Job,
            cwd: dir.to_path_buf(),
            note_path: dir.join("note.md"),
            job_id: Some("t".into()),
            host: Arc::new(NoHost),
            cancel: CancellationToken::new(),
            helper: None,
        }
    }

    #[tokio::test]
    async fn write_read_edit_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let c = ctx(d.path());
        assert!(!execute(&c, "write", &json!({"path":"output/a.txt","content":"hello\nworld\n"})).await.is_error);
        assert_eq!(execute(&c, "read", &json!({"path":"output/a.txt"})).await.text, "hello\nworld");
        let e = execute(&c, "edit", &json!({"path":"output/a.txt","edits":[{"old":"world","new":"there"}]})).await;
        assert!(!e.is_error, "{e:?}");
        assert!(std::fs::read_to_string(d.path().join("output/a.txt")).unwrap().contains("there"));
        assert!(execute(&c, "edit", &json!({"path":"output/a.txt","edits":[{"old":"zzz","new":"y"}]})).await.is_error);
    }

    #[tokio::test]
    async fn writes_outside_workspace_and_secrets_are_refused() {
        let d = tempfile::tempdir().unwrap();
        let c = ctx(d.path());
        let r = execute(&c, "write", &json!({"path":"~/Desktop/x.txt","content":"x"})).await;
        assert!(r.is_error && r.text.contains("device_save_file"));
        let r = execute(&c, "device_read_file", &json!({"path":"~/.ssh/id_rsa"})).await;
        assert!(r.is_error && r.text.contains("private"));
        let r = execute(&c, "bash", &json!({"command":"cat ~/.aws/credentials"})).await;
        assert!(r.is_error);
    }

    #[tokio::test]
    async fn bash_runs_in_cwd_and_reports_exit() {
        let d = tempfile::tempdir().unwrap();
        let c = ctx(d.path());
        let r = execute(&c, "bash", &json!({"command":"pwd && echo hi"})).await;
        assert!(!r.is_error);
        assert!(r.text.contains("hi"));
        let r = execute(&c, "bash", &json!({"command":"exit 3"})).await;
        assert!(r.is_error && r.text.starts_with("exit 3"));
        let r = execute(&c, "bash", &json!({"command":"sleep 5","timeout":1})).await;
        assert!(r.is_error && r.text.contains("Timed out"));
    }

    #[tokio::test]
    async fn save_file_never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let mac = tempfile::tempdir().unwrap();
        let c = ctx(d.path());
        std::fs::create_dir_all(d.path().join("output")).unwrap();
        std::fs::write(d.path().join("output/r.pdf"), "new").unwrap();
        std::fs::write(mac.path().join("r.pdf"), "old").unwrap();
        let r = execute(&c, "device_save_file", &json!({"source":"output/r.pdf","destination": format!("{}/", mac.path().display())})).await;
        assert!(r.text.ends_with("r 2.pdf"), "{r:?}");
        assert_eq!(std::fs::read_to_string(mac.path().join("r.pdf")).unwrap(), "old");
    }

    #[tokio::test]
    async fn note_tools_edit_note_md() {
        let d = tempfile::tempdir().unwrap();
        let c = ctx(d.path());
        execute(&c, "note", &json!({"markdown":"# Desk\n\n## Pick\n- [ ] Read reviews"})).await;
        execute(&c, "note_answer", &json!({"line":"Buy the Jarvis {big=\"$599\"}"})).await;
        execute(&c, "note_tick", &json!({"label":"Read"})).await;
        execute(&c, "note_add", &json!({"lines":["::link[Source]{url=https://x.y}"]})).await;
        let n = std::fs::read_to_string(d.path().join("note.md")).unwrap();
        assert_eq!(n, "# Desk\n\n## Pick\n> Buy the Jarvis {big=\"$599\"}\n- [x] Read reviews\n::link[Source]{url=https://x.y}\n");
    }

    #[test]
    fn glob() {
        assert!(glob_match("*.pdf", "Report.PDF"));
        assert!(glob_match("rep*", "report.txt"));
        assert!(!glob_match("*.pdf", "x.pdfx"));
        assert!(glob_match("port", "report.txt"));
    }
}
