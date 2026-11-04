//! Playbook loader: `skills/<name>/SKILL.md` (or `skills/<name>.md`) with a front-matter
//! `name` and `description`, from the bundled `skills/` and `<app_data>/skills/`.

use openclicky_core::prompt::Skill;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

fn front_matter(src: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut desc = None;
    let mut lines = src.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    for l in lines {
        let l = l.trim();
        if l == "---" {
            break;
        }
        if let Some(v) = l.strip_prefix("name:") {
            name = Some(v.trim().trim_matches('"').to_string());
        }
        if let Some(v) = l.strip_prefix("description:") {
            desc = Some(v.trim().trim_matches('"').to_string());
        }
    }
    (name, desc)
}

pub fn scan(dir: &Path) -> Vec<Skill> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out = vec![];
    for e in rd.flatten() {
        let p = e.path();
        let file = if p.is_dir() { p.join("SKILL.md") } else { p.clone() };
        if file.extension().map(|x| x != "md").unwrap_or(true) || file.file_name().map(|n| n == "README.md").unwrap_or(false) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&file) else { continue };
        let (name, desc) = front_matter(&src);
        let name = name.unwrap_or_else(|| p.file_stem().unwrap_or_default().to_string_lossy().to_string());
        let Some(description) = desc else { continue };
        out.push(Skill { name, description, path: file.display().to_string() });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn load(app: &AppHandle) -> Vec<Skill> {
    let mut dirs: Vec<PathBuf> = vec![];
    if let Ok(r) = app.path().resource_dir() {
        dirs.push(r.join("skills"));
    }
    dirs.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../skills")));
    if let Ok(d) = app.path().app_data_dir() {
        dirs.push(d.join("skills"));
    }
    let mut all: Vec<Skill> = vec![];
    for d in dirs {
        for s in scan(&d) {
            if !all.iter().any(|x| x.name == s.name) {
                all.push(s);
            }
        }
    }
    log::info!("skills: {}", all.len());
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_front_matter() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("pdf")).unwrap();
        std::fs::write(d.path().join("pdf/SKILL.md"), "---\nname: pdf\ndescription: \"Make PDFs\"\n---\nbody").unwrap();
        std::fs::write(d.path().join("no-desc.md"), "# x").unwrap();
        let s = scan(d.path());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].description, "Make PDFs");
    }
}
