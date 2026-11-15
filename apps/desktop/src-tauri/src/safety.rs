//! Nothing that deletes, sends, buys or can't be undone happens on their screen without their OK.
//!
//! The model's own judgement isn't enough (it once wrote "hi" into the wrong chat), so this sits
//! in the code path of every click, press, menu choice, key and typed text: when the control
//! under the pointer, the key or the text is risky, Pixie asks them itself in a system dialog
//! ("Allow" / "Don't", Don't is the default). Don't, or no answer in a minute, and it doesn't
//! happen. The model can't answer the dialog.

use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Words that start a risky button or menu item ("Delete", "Move to Trash", "Send", "Pay now").
const RISKY: &[&str] = &[
    "delete", "remove", "erase", "move to trash", "move to bin", "empty trash", "empty bin", "destroy", "drop", "truncate", "wipe",
    "reset", "format", "uninstall", "discard", "don't save", "dont save", "do not save", "overwrite", "replace", "revoke", "deactivate",
    "close account", "terminate", "send", "post", "publish", "submit", "pay", "buy", "purchase", "place order", "order now", "checkout",
    "check out", "subscribe", "donate", "transfer", "withdraw", "confirm", "approve", "merge", "deploy", "force", "shut down", "restart",
    "log out", "block", "leave", "exit group", "clear chat", "clear history", "clear all", "factory", "permanently", "forever",
];

/// Typed text that would do damage once it runs (a terminal, a database console).
const RISKY_TEXT: &[&str] = &[
    "rm -rf", "rm -fr", "rm -r ", "rm -f ", "sudo rm", "rmdir ", "mkfs", "dd if=", "diskutil erase", "format c:", "del /", "rd /s",
    "remove-item", "drop table", "drop database", "drop schema", "drop collection", "dropdatabase", "truncate table", "delete from",
    "git push -f", "git push --force", "git reset --hard", "git clean -f", "git branch -d", "chmod -r", "chown -r", "killall", "kill -9",
    "shutdown", "reboot", "docker system prune", "docker rm", "kubectl delete", "terraform destroy", "npm publish", "cargo publish",
    "> /dev/", "flushall", "flushdb",
];

/// Apps where Return sends what's typed.
const MESSENGERS: &[&str] = &[
    "whatsapp", "messages", "telegram", "slack", "discord", "signal", "microsoft teams", "teams", "messenger", "mail", "outlook", "skype",
    "wechat", "line", "viber", "beeper", "element", "spark", "airmail",
];

/// An app name as people see it (WhatsApp reports "\u{200e}WhatsApp").
pub fn app_name(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '.').collect::<String>().trim().to_string()
}

fn norm(label: &str) -> String {
    let l = label.to_lowercase().replace(['’', '‘'], "'");
    let l = l.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').trim().to_string();
    l.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A short button or menu label that names a risky action. Long labels are content (a chat
/// preview that mentions "delete" isn't a Delete button).
pub fn risky_label(label: &str) -> bool {
    let l = norm(label);
    if l.is_empty() || l.chars().count() > 40 || l.split(' ').count() > 5 {
        return false;
    }
    let words: Vec<&str> = l.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).collect();
    let joined = format!(" {} ", words.join(" "));
    RISKY.iter().any(|p| joined.contains(&format!(" {p} ")))
}

pub fn risky_text(text: &str) -> Option<&'static str> {
    let t = format!("{} ", text.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" "));
    RISKY_TEXT.iter().copied().find(|p| t.contains(p))
}

fn is_messenger(app: &str) -> bool {
    let a = app_name(app).to_lowercase();
    MESSENGERS.iter().any(|m| a == *m)
}

/// A key combo that deletes or sends: cmd+delete (to the Trash), shift+cmd+delete (empty the
/// Trash), shift+delete (Windows: delete for good), cmd/ctrl+return (send in many apps), and
/// return in a messaging app.
pub fn risky_key(combo: &str, app: &str) -> Option<String> {
    let parts: Vec<String> = combo.to_lowercase().split(['+', '-', ' ']).filter(|p| !p.is_empty()).map(|p| p.to_string()).collect();
    let has = |names: &[&str]| parts.iter().any(|p| names.contains(&p.as_str()));
    let cmd = has(&["cmd", "command", "⌘", "meta", "super", "win"]);
    let ctrl = has(&["ctrl", "control", "⌃"]);
    let shift = has(&["shift", "⇧"]);
    let key = parts.last().cloned().unwrap_or_default();
    let delete = matches!(key.as_str(), "delete" | "backspace" | "del" | "forwarddelete" | "⌫");
    let enter = matches!(key.as_str(), "return" | "enter" | "⏎");
    if delete && (cmd || shift) {
        return Some(format!("press {combo}, which can delete things"));
    }
    if enter && (cmd || ctrl) {
        return Some(format!("press {combo}, which sends in many apps"));
    }
    if enter && is_messenger(app) {
        return Some(format!("send the message in {}", app_name(app)));
    }
    None
}

/// What would happen, for the dialog ("press “Delete” in Finder").
pub fn press_text(label: &str, app: &str) -> String {
    let app = app_name(app);
    if app.is_empty() {
        format!("press “{}”", label.trim())
    } else {
        format!("press “{}” in {app}", label.trim())
    }
}

/// Ask them. True only for an explicit Allow.
pub async fn ask(app: &AppHandle, what: &str, pid: Option<i64>) -> bool {
    let text = format!("Pixie wants to {what}.\n\nThis may not be undoable. Allow it?");
    log::warn!("safety: asking before: {what}");
    let t = text.clone();
    let allowed = tauri::async_runtime::spawn_blocking(move || dialog(&t)).await.unwrap_or(false);
    log::info!("safety: {}: {what}", if allowed { "allowed" } else { "not allowed" });
    if allowed {
        // The dialog took focus: give it back to the app Pixie is working in.
        if let (true, Some(p)) = (cfg!(target_os = "macos"), pid) {
            let _ = app.state::<crate::AppState>().helper.call("activate", serde_json::json!({ "pid": p }), Duration::from_secs(3)).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }
    allowed
}

/// The message for the model when they said no.
pub fn declined(what: &str) -> String {
    format!("They didn't allow it (or didn't answer within a minute), so I didn't {what}. Don't try another way to do it; tell them it's stopped and ask what they'd like.")
}

#[cfg(target_os = "macos")]
fn dialog(text: &str) -> bool {
    // The text goes in as an argument, so nothing in it is read as AppleScript.
    let out = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display dialog (item 1 of argv) with title \"Pixie: check this first\" buttons {\"Don't\", \"Allow\"} default button \"Don't\" cancel button \"Don't\" with icon caution giving up after 60",
            "-e",
            "end run",
            text,
        ])
        .output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            o.status.success() && s.contains("button returned:Allow") && !s.contains("gave up:true")
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn dialog(text: &str) -> bool {
    use std::os::windows::process::CommandExt;
    let script = "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show($env:PIXIE_ASK, 'Pixie: check this first', 'YesNo', 'Warning', 'No')";
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("PIXIE_ASK", text)
        .creation_flags(0x0800_0000)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "Yes")
        .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn dialog(text: &str) -> bool {
    let zenity = std::process::Command::new("zenity")
        .args(["--question", "--title=Pixie: check this first", "--default-cancel", "--timeout=60", "--ok-label=Allow", "--cancel-label=Don't"])
        .arg(format!("--text={text}"))
        .status();
    if let Ok(s) = zenity {
        return s.success();
    }
    // No way to ask: don't do it.
    std::process::Command::new("kdialog").args(["--title", "Pixie: check this first", "--warningyesno", text]).status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risky_buttons_are_caught() {
        for l in ["Delete", "Delete Chat", "Move to Trash", "Empty Trash…", "Send", "Send message", "Pay now", "Place order", "Yes, delete it", "Don’t Save", "Replace", "Permanently delete", "Clear chat", "DROP"] {
            assert!(risky_label(l), "{l}");
        }
        for l in ["Search", "Cancel", "M0M", "Open", "Chats", "Archive", "Reply", "Sender settings", "Share", "Posts"] {
            assert!(!risky_label(l), "{l}");
        }
        // A chat preview that only mentions a risky word isn't a button.
        assert!(!risky_label("Priya: can you delete the old photos from the shared album before Friday?"));
    }

    #[test]
    fn risky_commands_and_keys() {
        assert_eq!(risky_text("rm -rf ~/Projects"), Some("rm -rf"));
        assert_eq!(risky_text("DROP   DATABASE prod;"), Some("drop database"));
        assert_eq!(risky_text("psql -c 'delete from users'"), Some("delete from"));
        assert_eq!(risky_text("git push --force origin main"), Some("git push --force"));
        assert_eq!(risky_text("hi mom"), None);
        assert_eq!(risky_text("ls -la"), None);
        assert!(risky_key("cmd+delete", "Finder").is_some());
        assert!(risky_key("shift+cmd+backspace", "Finder").is_some());
        assert!(risky_key("cmd+return", "Slack").is_some());
        assert!(risky_key("return", "\u{200e}WhatsApp").is_some());
        assert!(risky_key("return", "TextEdit").is_none());
        assert!(risky_key("delete", "TextEdit").is_none());
        assert!(risky_key("cmd+a", "WhatsApp").is_none());
    }
}
