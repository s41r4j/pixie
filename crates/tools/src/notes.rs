//! `note_*` operations as pure string edits (so they're easy to test), applied to note.md.

pub fn add(src: &str, lines: &[String]) -> String {
    let mut out = src.trim_end_matches('\n').to_string();
    for l in lines {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(l);
    }
    out.push('\n');
    out
}

/// Set the answer line (`> …`) at the top of the first page.
pub fn answer(src: &str, line: &str) -> String {
    let line = line.trim();
    let line = if line.starts_with('>') { line.to_string() } else { format!("> {line}") };
    let lines: Vec<&str> = src.lines().collect();
    // First page = up to the second `## ` heading.
    let mut pages_seen = 0;
    let mut first_page_end = lines.len();
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("## ") {
            pages_seen += 1;
            if pages_seen == 2 {
                first_page_end = i;
                break;
            }
        }
    }
    let existing = lines[..first_page_end]
        .iter()
        .position(|l| l.trim_start().starts_with('>') && !l.trim_start()[1..].trim_start().starts_with("[!"));
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    match existing {
        Some(i) => out[i] = line,
        None => {
            let at = lines
                .iter()
                .position(|l| l.starts_with("## "))
                .or_else(|| lines.iter().position(|l| l.starts_with("# ")))
                .map(|i| i + 1)
                .unwrap_or(0);
            out.insert(at, line);
        }
    }
    out.join("\n") + "\n"
}

pub fn replace(src: &str, old: &str, new: &str) -> Result<String, String> {
    match src.matches(old).count() {
        0 => Err(format!("\"{old}\" isn't in the note.")),
        _ => Ok(src.replacen(old, new, 1)),
    }
}

pub fn remove(src: &str, line: &str) -> Result<String, String> {
    let target = line.trim();
    let lines: Vec<&str> = src.lines().collect();
    let idx = lines
        .iter()
        .position(|l| l.trim() == target)
        .or_else(|| lines.iter().position(|l| !target.is_empty() && l.contains(target)))
        .ok_or_else(|| format!("No line \"{target}\" in the note."))?;
    let mut out: Vec<&str> = lines.clone();
    out.remove(idx);
    // Drop indented detail lines that belonged to a removed list item.
    if lines[idx].trim_start().starts_with("- ") {
        while idx < out.len() && (out[idx].starts_with("  ") && !out[idx].trim_start().starts_with("- ")) {
            out.remove(idx);
        }
    }
    Ok(out.join("\n") + "\n")
}

fn strip_marks(text: &str) -> &str {
    let t = text.trim_end();
    if t.ends_with('}') {
        if let Some(open) = t.rfind('{') {
            return t[..open].trim_end();
        }
    }
    t
}

/// Tick `- [ ] label…` (or mark it `{now}` with `current`).
pub fn tick(src: &str, label: &str, current: bool) -> Result<String, String> {
    let want = label.trim().to_lowercase();
    let mut lines: Vec<String> = src.lines().map(str::to_string).collect();
    let is_box = |l: &str| {
        let t = l.trim_start();
        t.starts_with("- [ ] ") || t.starts_with("- [x] ") || t.starts_with("- [X] ")
    };
    let idx = lines
        .iter()
        .position(|l| is_box(l) && strip_marks(&l.trim_start()[6..]).to_lowercase().starts_with(&want))
        .or_else(|| lines.iter().position(|l| is_box(l) && l.to_lowercase().contains(&want)))
        .ok_or_else(|| {
            let open: Vec<String> = lines
                .iter()
                .filter(|l| l.trim_start().starts_with("- [ ] "))
                .map(|l| l.trim_start()[6..].to_string())
                .collect();
            format!("No box \"{label}\". Open boxes: {}", if open.is_empty() { "none".into() } else { open.join("; ") })
        })?;
    // Only one item is "now".
    for l in lines.iter_mut() {
        if l.trim_end().ends_with("{now}") {
            *l = l.trim_end().trim_end_matches("{now}").trim_end().to_string();
        }
    }
    let line = &lines[idx];
    let indent = &line[..line.len() - line.trim_start().len()];
    let body = line.trim_start()[6..].to_string();
    lines[idx] = if current {
        format!("{indent}- [ ] {} {{now}}", strip_marks(&body))
    } else {
        format!("{indent}- [x] {body}")
    };
    Ok(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "# Desk pick\n\n## The pick\n- [ ] Read reviews\n- [ ] Compare prices\n  six desks\n\n## Sources\n> a quote\n";

    #[test]
    fn answer_inserts_then_replaces() {
        let a = answer(NOTE, "Buy the Jarvis. {big=\"$599\"}");
        assert!(a.contains("## The pick\n> Buy the Jarvis. {big=\"$599\"}\n- [ ] Read"));
        let b = answer(&a, "> Buy the Uplift.");
        assert!(b.contains("> Buy the Uplift.") && !b.contains("Jarvis"));
        assert!(b.contains("> a quote"), "later pages are untouched");
        assert_eq!(answer("", "Hi"), "> Hi\n");
    }

    #[test]
    fn tick_and_current() {
        let t = tick(NOTE, "read", false).unwrap();
        assert!(t.contains("- [x] Read reviews"));
        let c = tick(&t, "Compare", true).unwrap();
        assert!(c.contains("- [ ] Compare prices {now}"));
        let d = tick(&c, "Compare prices", false).unwrap();
        assert!(d.contains("- [x] Compare prices") && !d.contains("{now}"));
        assert!(tick(NOTE, "nothing", false).unwrap_err().contains("Open boxes: Read reviews; Compare prices"));
    }

    #[test]
    fn add_replace_remove() {
        let a = add("# T", &["- one".into(), "- two".into()]);
        assert_eq!(a, "# T\n- one\n- two\n");
        assert_eq!(replace(&a, "two", "2").unwrap(), "# T\n- one\n- 2\n");
        assert!(replace(&a, "zzz", "y").is_err());
        let r = remove(NOTE, "- [ ] Compare prices").unwrap();
        assert!(!r.contains("Compare") && !r.contains("six desks"));
        assert!(remove(NOTE, "nope").is_err());
    }
}
