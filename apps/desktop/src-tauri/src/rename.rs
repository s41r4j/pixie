//! The move from the old app ID (`com.openclicky.app`, up to 0.8.0) to `pixie.s41r4j.in`.
//!
//! On the first launch with the new ID, the old data folder (settings, keys, memory, chats, jobs,
//! canvases, the voice) is moved to the new name. Both sit in the same parent folder, so it's a
//! rename, not a copy. A link is left at the old path, so paths saved in older files and an older
//! version put back by a rollback still find everything. The old browser bridge registration is
//! removed when the new one is written.

use std::path::{Path, PathBuf};

pub const OLD_ID: &str = "com.openclicky.app";
pub const OLD_HOST_NAME: &str = "com.openclicky.browser";

/// What happened to the old folder.
#[derive(Debug, PartialEq)]
pub enum Moved {
    /// Nothing to move (a new install, or moved before).
    Nothing,
    /// Moved and linked.
    Done,
    /// Both folders have data; the old one was left alone.
    BothThere(PathBuf),
    Failed(String),
}

/// Move `<parent>/com.openclicky.app` to `new` (the current data folder) and link the old path.
pub fn move_data(new: &Path) -> Moved {
    let Some(parent) = new.parent() else { return Moved::Nothing };
    let old = parent.join(OLD_ID);
    let Ok(meta) = std::fs::symlink_metadata(&old) else { return Moved::Nothing };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Moved::Nothing;
    }
    if new.exists() {
        // An empty new folder (made before this ran) can go; one with data stays as it is.
        let empty = std::fs::read_dir(new).map(|mut r| r.next().is_none()).unwrap_or(false);
        if !empty || std::fs::remove_dir(new).is_err() {
            return Moved::BothThere(old);
        }
    }
    if let Err(e) = std::fs::rename(&old, new) {
        return Moved::Failed(e.to_string());
    }
    link(new, &old);
    Moved::Done
}

#[cfg(unix)]
fn link(new: &Path, old: &Path) {
    let _ = std::os::unix::fs::symlink(new, old);
}

#[cfg(windows)]
fn link(new: &Path, old: &Path) {
    // A directory junction needs no admin rights (a symlink does).
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("cmd").arg("/C").arg("mklink").arg("/J").arg(old).arg(new).creation_flags(0x0800_0000).status();
}

#[cfg(not(any(unix, windows)))]
fn link(_new: &Path, _old: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_folder_moves_once_and_stays_reachable() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join(OLD_ID);
        let new = d.path().join("pixie.s41r4j.in");
        std::fs::create_dir_all(old.join("jobs")).unwrap();
        std::fs::write(old.join("settings.json"), "{}").unwrap();
        // A new install: nothing to do.
        assert_eq!(move_data(&d.path().join("x").join("pixie.s41r4j.in")), Moved::Nothing);
        // An empty new folder made earlier doesn't stop it.
        std::fs::create_dir_all(&new).unwrap();
        assert_eq!(move_data(&new), Moved::Done);
        assert!(new.join("settings.json").exists() && new.join("jobs").is_dir());
        // The old path still leads there.
        assert!(old.join("settings.json").exists());
        assert_eq!(move_data(&new), Moved::Nothing);
    }

    #[test]
    fn data_in_both_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join(OLD_ID);
        let new = d.path().join("pixie.s41r4j.in");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("settings.json"), "{}").unwrap();
        assert!(matches!(move_data(&new), Moved::BothThere(_)));
        assert!(old.is_dir() && new.join("settings.json").exists());
    }
}
