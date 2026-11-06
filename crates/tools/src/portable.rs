//! The bits of the tool layer that differ by platform. The Mac uses its own programs (bash,
//! grep, sips, open); Windows and Linux get the nearest equivalent or a plain Rust version.

use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Is a program on PATH? (`name` without extension; Windows also tries .exe/.cmd.)
pub fn which(name: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p).find_map(|d| exts.iter().map(|e| d.join(format!("{name}{e}"))).find(|f| f.is_file()))
    })
}

/// The shell the `bash` tool runs: bash on Mac/Linux; on Windows Git Bash when it's installed,
/// otherwise PowerShell.
pub fn shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let git_bash = which("bash").filter(|p| !p.to_string_lossy().to_lowercase().contains("system32")).or_else(|| {
            ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"]
                .iter()
                .filter_map(std::env::var_os)
                .map(|d| PathBuf::from(d).join("Git").join("bin").join("bash.exe"))
                .find(|p| p.is_file())
        });
        if let Some(bash) = git_bash {
            let mut c = Command::new(bash);
            c.arg("-c").arg(command);
            return c;
        }
        let mut c = Command::new("powershell.exe");
        c.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", command]);
        no_window(&mut c);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("/bin/bash");
        c.arg("-c").arg(command);
        c
    }
}

/// Which shell `bash` really is here, for the tool description.
pub fn shell_name() -> &'static str {
    if cfg!(windows) {
        "bash (Git Bash) or PowerShell when Git isn't installed"
    } else {
        "bash"
    }
}

#[cfg(windows)]
pub fn no_window(c: &mut Command) {
    // CREATE_NO_WINDOW: no console flashing up for every command.
    c.creation_flags(0x0800_0000);
}

/// PowerShell single-quoted literal (also doubles the curly quotes PowerShell treats as quotes).
pub fn ps_quote(s: &str) -> String {
    let mut out = String::from("'");
    for ch in s.chars() {
        if matches!(ch, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(ch);
        }
        out.push(ch);
    }
    out.push('\'');
    out
}

/// Open a file, folder, URL or (by name) an app with the system's default handler.
pub fn open(target: &str, app_name: bool) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("/usr/bin/open");
        if app_name {
            c.arg("-a");
        }
        c.arg(target);
        c
    }
    #[cfg(windows)]
    {
        let _ = app_name;
        let mut c = Command::new("powershell.exe");
        c.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &format!("Start-Process -FilePath {}", ps_quote(target))]);
        no_window(&mut c);
        c
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if app_name {
            // gtk-launch takes a .desktop id; fall back to running it by name.
            let mut c = Command::new("sh");
            c.arg("-c").arg("gtk-launch \"$1\" 2>/dev/null || (\"$1\" >/dev/null 2>&1 &)").arg("sh").arg(target);
            return c;
        }
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    }
}

/// Recursive copy (directories) without shelling out.
pub fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

/// `grep -rnEI -m 50` in Rust, for systems without grep. Lines are `path:line:text`.
pub fn grep(pattern: &str, root: &Path, glob: &str) -> Result<String, String> {
    let re = regex::Regex::new(pattern).map_err(|e| format!("Bad pattern: {e}"))?;
    let mut out = vec![];
    fn walk(dir: &Path, re: &regex::Regex, glob: &str, out: &mut Vec<String>, depth: usize) {
        if depth > 30 || out.len() >= 2000 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                if name != "node_modules" && name != ".git" {
                    walk(&p, re, glob, out, depth + 1);
                }
                continue;
            }
            if !glob.is_empty() && !crate::exec::glob_match(glob, &name) {
                continue;
            }
            file(&p, re, out);
        }
    }
    fn file(p: &Path, re: &regex::Regex, out: &mut Vec<String>) {
        let Ok(bytes) = std::fs::read(p) else { return };
        if bytes.len() > 20_000_000 || bytes.iter().take(4096).any(|b| *b == 0) {
            return;
        }
        let text = String::from_utf8_lossy(&bytes);
        let mut n = 0;
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                out.push(format!("{}:{}:{}", p.display(), i + 1, line.chars().take(400).collect::<String>()));
                n += 1;
                if n >= 50 {
                    break;
                }
            }
        }
    }
    if root.is_file() {
        file(root, &re, &mut out);
    } else {
        walk(root, &re, glob, &mut out, 0);
    }
    Ok(out.join("\n"))
}

/// Re-encode any common image as a JPEG no larger than `max` px on its long side.
#[cfg(not(target_os = "macos"))]
pub fn to_jpeg(src: &Path, out: &Path, max: u32) -> Result<(), String> {
    let img = image::open(src).map_err(|e| format!("Couldn't read {}: {e}", src.display()))?;
    let img = if img.width().max(img.height()) > max { img.resize(max, max, image::imageops::FilterType::Triangle) } else { img };
    img.to_rgb8().save_with_format(out, image::ImageFormat::Jpeg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_powershell() {
        assert_eq!(ps_quote("it's"), "'it''s'");
        assert_eq!(ps_quote("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
    }

    #[test]
    fn rust_grep_finds_lines() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.txt"), "one\ntwo needle\nthree").unwrap();
        std::fs::write(d.path().join("b.md"), "needle").unwrap();
        let out = grep("needle", d.path(), "*.txt").unwrap();
        assert!(out.ends_with("a.txt:2:two needle"), "{out}");
        assert!(!out.contains("b.md"));
    }
}
