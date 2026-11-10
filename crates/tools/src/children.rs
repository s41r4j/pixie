//! Processes started for agents (shell commands, coding agents) end with Pixie.
//!
//! Each runs in its own process group, listed here and in a small file. Quit ends them all
//! ([`kill_all`]); if Pixie is force-quit or crashes, the helper notices at once (its input
//! closes) and ends the groups in the file. Stopping or timing out a command ends its whole group,
//! not only the shell at the top (a `npm run dev` under it kept running before).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tokio::process::Command;

static LIST: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static FILE: OnceLock<PathBuf> = OnceLock::new();

/// Where the list is mirrored for the helper (one process group id per line).
pub fn set_file(path: PathBuf) {
    let _ = std::fs::write(&path, "");
    let _ = FILE.set(path);
}

/// The file the list is kept in, for the helper.
pub fn file() -> Option<&'static PathBuf> {
    FILE.get()
}

/// Before spawning: its own process group, so everything under it can be ended together.
pub fn prepare(cmd: &mut Command) {
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(not(unix))]
    let _ = cmd;
}

/// Keeps a spawned process listed; dropped before [`Guard::done`] (stopped, timed out), it ends
/// the whole group.
pub struct Guard(Option<u32>);

impl Guard {
    pub fn done(mut self) {
        self.0 = None;
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            kill_group(pid);
        }
    }
}

/// List a spawned process (its pid is its group id after [`prepare`]).
pub fn add(pid: Option<u32>) -> Guard {
    let Some(pid) = pid else { return Guard(None) };
    let mut list = LIST.lock().unwrap();
    // Groups that have ended drop off, so the list stays short.
    list.retain(|p| alive(*p));
    list.push(pid);
    save(&list);
    Guard(Some(pid))
}

/// Pixie is quitting: end every group still running.
pub fn kill_all() {
    let mut list = LIST.lock().unwrap();
    for p in list.drain(..) {
        kill_group(p);
    }
    save(&list);
}

fn save(list: &[u32]) {
    if let Some(f) = FILE.get() {
        let body: String = list.iter().map(|p| format!("{p}\n")).collect();
        let _ = std::fs::write(f, body);
    }
}

#[cfg(unix)]
fn alive(pgid: u32) -> bool {
    // Signal 0 checks the group exists without touching it.
    unsafe { libc::kill(-(pgid as i32), 0) == 0 }
}

#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    true
}

#[cfg(unix)]
fn kill_group(pgid: u32) {
    if pgid > 1 {
        unsafe {
            libc::kill(-(pgid as i32), libc::SIGTERM);
        }
    }
}

#[cfg(windows)]
fn kill_group(pid: u32) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(0x0800_0000).status();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stopping_ends_the_whole_group() {
        // A shell that starts a grandchild and waits on it.
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg("sleep 30 & wait");
        prepare(&mut cmd);
        let mut child = cmd.spawn().unwrap();
        let pid = child.id().unwrap();
        let guard = add(Some(pid));
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(alive(pid));
        drop(guard);
        let _ = child.wait().await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(!alive(pid), "the sleep under the shell ended too");
    }
}
