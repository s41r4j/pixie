//! Hard rules enforced in code, not in the prompt (BUILD-SPEC §5):
//! - device paths under `.ssh`, `.gnupg`, `Library/Keychains`, `.aws`, `.config/gh`, or any
//!   `.env*` file are refused before the read;
//! - saving to the Mac never overwrites: ` 2`, ` 3`, … go before the extension.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct Denied(pub String);

impl std::fmt::Display for Denied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is private (keys, credentials or secrets), so I won't open it.", self.0)
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// `~` expansion only; no canonicalisation.
pub fn expand(path: &str) -> PathBuf {
    let p = path.trim();
    if p == "~" {
        return home();
    }
    if let Some(rest) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
        return home().join(rest);
    }
    PathBuf::from(p)
}

/// Lexically resolve `.` and `..` (works for paths that don't exist yet).
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn denied_components(path: &Path) -> bool {
    let parts: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    for (i, p) in parts.iter().enumerate() {
        let lower = p.to_lowercase();
        if lower == ".ssh" || lower == ".gnupg" || lower == ".aws" || lower.starts_with(".env") {
            return true;
        }
        let next = parts.get(i + 1).map(|s| s.to_lowercase());
        if lower == "library" && next.as_deref() == Some("keychains") {
            return true;
        }
        if lower == ".config" && next.as_deref() == Some("gh") {
            return true;
        }
        // Windows: saved credentials, DPAPI keys and the credential vault.
        if lower == "microsoft" && matches!(next.as_deref(), Some("credentials" | "protect" | "vault")) {
            return true;
        }
    }
    false
}

/// Check a path from the model. Both the literal path and its symlink-resolved form must pass.
pub fn check_path(path: &Path) -> Result<PathBuf, Denied> {
    let abs = normalize(path);
    if denied_components(&abs) {
        return Err(Denied(path.display().to_string()));
    }
    if let Ok(real) = std::fs::canonicalize(&abs) {
        if denied_components(&real) {
            return Err(Denied(path.display().to_string()));
        }
    }
    Ok(abs)
}

/// Best effort for `bash`: refuse commands that name a denied location.
pub fn check_command(cmd: &str) -> Result<(), Denied> {
    for token in cmd.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | ';' | '|' | '&' | '(' | ')' | '<' | '>' | '=')) {
        if token.is_empty() {
            continue;
        }
        if denied_components(&expand(token)) || token.contains("Library/Keychains") {
            return Err(Denied(token.to_string()));
        }
    }
    Ok(())
}

/// First free path: `name.ext`, `name 2.ext`, `name 3.ext`, …
pub fn unique_destination(dest: &Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| parent.join(format!("{stem} {n}{ext}")))
        .find(|p| !p.exists())
        .expect("some free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_blocks_secrets() {
        for p in [
            "~/.ssh/id_ed25519",
            "/Users/me/.gnupg/pubring.kbx",
            "~/Library/Keychains/login.keychain-db",
            "~/.aws/credentials",
            "~/.config/gh/hosts.yml",
            "/Users/me/code/app/.env",
            "/Users/me/code/app/.env.local",
            "/Users/me/Documents/../.ssh/config",
            "~/library/keychains",
        ] {
            assert!(check_path(&expand(p)).is_err(), "{p} should be denied");
        }
    }

    #[test]
    fn denylist_allows_normal_files() {
        for p in ["~/Documents/report.pdf", "~/Desktop/notes.txt", "/tmp/environment.txt", "~/.config/nvim/init.lua", "~/my.envelope"] {
            assert!(check_path(&expand(p)).is_ok(), "{p} should be allowed");
        }
    }

    #[test]
    fn symlink_into_secret_is_denied() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join(".ssh");
        std::fs::create_dir(&secret).unwrap();
        std::fs::write(secret.join("key"), "x").unwrap();
        let link = dir.path().join("innocent");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        assert!(check_path(&link.join("key")).is_err());
    }

    #[test]
    fn bash_commands_naming_secrets_are_refused() {
        assert!(check_command("cat ~/.ssh/id_rsa").is_err());
        assert!(check_command("cp .env /tmp/x").is_err());
        assert!(check_command("security dump-keychain ~/Library/Keychains/login.keychain-db").is_err());
        assert!(check_command("ls -la output && python3 make.py > out.txt").is_ok());
    }

    #[test]
    fn never_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("Report.pdf");
        assert_eq!(unique_destination(&a), a);
        std::fs::write(&a, "1").unwrap();
        assert_eq!(unique_destination(&a), dir.path().join("Report 2.pdf"));
        std::fs::write(dir.path().join("Report 2.pdf"), "2").unwrap();
        assert_eq!(unique_destination(&a), dir.path().join("Report 3.pdf"));
        let b = dir.path().join("Makefile");
        std::fs::write(&b, "").unwrap();
        assert_eq!(unique_destination(&b), dir.path().join("Makefile 2"));
    }
}
