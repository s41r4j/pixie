//! `note.md` DSL → AST (BUILD-SPEC §7). The note is the UI: Markdown plus paper objects.
//!
//! ```text
//! # Title
//! ## Page name
//! > The answer in one line. {big="$599" note="steadiest under $700"}
//! :::tiles                         (or a bare `:::`)
//! ::figure[Revenue]{value="$1,402" delta="+6%"}
//! ::countdown[Midterms]{to="2026-10-21 09:00"}
//! :::
//! ::ask[Find it cheaper]{say="Find an equally steady desk for less"}
//! - [ ] a box to tick {new}
//!   a reason on its own indented line
//! > [!WARNING] a callout
//! | a | table |
//! ```
//!
//! The parser never fails: anything it doesn't recognise renders as a paragraph.

use serde::Serialize;
use std::collections::BTreeMap;

pub type Attrs = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct NoteDoc {
    pub title: String,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Page {
    pub name: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListItem {
    pub text: String,
    /// `Some` for `- [ ]` / `- [x]`.
    pub checked: Option<bool>,
    /// Trailing marks such as `{new}`, `{pick}`, `{now}`.
    pub marks: Vec<String>,
    /// Indented lines under the item.
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// `> text {big=… note=…}`: the answer line (first page) or any quote with attrs.
    Answer { text: String, attrs: Attrs },
    Quote { text: String },
    Callout { kind: String, text: String },
    Heading { level: u8, text: String },
    /// One-line object: `::name[label]{attrs}`.
    Object { name: String, label: String, attrs: Attrs },
    /// `:::name[label]{attrs}` … `:::`. `lines` is the raw body; `children` the parsed body.
    Container { name: String, label: String, attrs: Attrs, lines: Vec<String>, children: Vec<Block> },
    List { ordered: bool, items: Vec<ListItem> },
    Table { header: Vec<String>, rows: Vec<Vec<String>> },
    Code { lang: String, text: String },
    Paragraph { text: String },
    Rule,
}

/// Parse `name[label]{attrs}` (after the leading colons).
fn parse_directive(s: &str) -> Option<(String, String, Attrs)> {
    let name_end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(s.len());
    let name = &s[..name_end];
    if name.is_empty() {
        return None;
    }
    let mut rest = &s[name_end..];
    let mut label = String::new();
    if rest.starts_with('[') {
        let close = matching(rest, '[', ']')?;
        label = rest[1..close].to_string();
        rest = &rest[close + 1..];
    }
    let mut attrs = Attrs::new();
    let rest = rest.trim_start();
    if rest.starts_with('{') {
        let close = matching(rest, '{', '}').unwrap_or(rest.len() - 1);
        attrs = parse_attrs(&rest[1..close]);
    }
    Some((name.to_string(), label, attrs))
}

/// Index of the bracket closing the one at position 0 (quotes respected).
fn matching(s: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut quoted = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            c if c == open && !quoted => depth += 1,
            c if c == close && !quoted => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// `a=1 b="two words" flag` → {a: 1, b: two words, flag: ""}.
pub fn parse_attrs(s: &str) -> Attrs {
    let mut out = Attrs::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if key.is_empty() {
            i += 1;
            continue;
        }
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            let value: String = if i < chars.len() && chars[i] == '"' {
                i += 1;
                let vs = i;
                while i < chars.len() && chars[i] != '"' {
                    i += 1;
                }
                let v = chars[vs..i].iter().collect();
                i += 1;
                v
            } else {
                let vs = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                chars[vs..i].iter().collect()
            };
            out.insert(key, value);
        } else {
            out.insert(key, String::new());
        }
    }
    out
}

/// Split a trailing `{…}` attribute group off a line, if there is one.
fn split_trailing_attrs(line: &str) -> (String, Option<Attrs>) {
    let t = line.trim_end();
    if t.ends_with('}') {
        if let Some(open) = t.rfind('{') {
            let inner = &t[open + 1..t.len() - 1];
            // Only treat it as attrs when it looks like attrs (no nested braces).
            if !inner.contains('{') {
                return (t[..open].trim_end().to_string(), Some(parse_attrs(inner)));
            }
        }
    }
    (t.to_string(), None)
}

/// Trailing `{new}` style marks on a list item.
fn split_marks(text: &str) -> (String, Vec<String>) {
    let (rest, attrs) = split_trailing_attrs(text);
    match attrs {
        Some(a) if !a.is_empty() && a.values().all(|v| v.is_empty()) => (rest, a.into_keys().collect()),
        _ => (text.trim_end().to_string(), vec![]),
    }
}

fn list_marker(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
        return Some((false, r));
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        return Some((true, &t[digits + 2..]));
    }
    None
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

fn is_table_rule(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Parse a run of lines (no `#`/`##` handling) into blocks.
pub fn parse_blocks(lines: &[&str]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let flush = |para: &mut Vec<String>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Paragraph { text: para.join(" ") });
            para.clear();
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut out);
            i += 1;
            continue;
        }
        // Fenced code.
        if let Some(lang) = t.strip_prefix("```") {
            flush(&mut para, &mut out);
            let mut body = vec![];
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                body.push(lines[i]);
                i += 1;
            }
            out.push(Block::Code { lang: lang.trim().to_string(), text: body.join("\n") });
            i += 1;
            continue;
        }
        // Containers: `:::name…` or bare `:::` (a tile strip).
        if let Some(rest) = t.strip_prefix(":::") {
            flush(&mut para, &mut out);
            let (name, label, attrs) = if rest.trim().is_empty() {
                ("tiles".to_string(), String::new(), Attrs::new())
            } else {
                parse_directive(rest.trim()).unwrap_or(("tiles".into(), String::new(), Attrs::new()))
            };
            let mut body = vec![];
            i += 1;
            while i < lines.len() && lines[i].trim() != ":::" {
                body.push(lines[i]);
                i += 1;
            }
            i += 1; // closing :::
            let children = parse_blocks(&body);
            out.push(Block::Container {
                name,
                label,
                attrs,
                lines: body.iter().map(|s| s.to_string()).collect(),
                children,
            });
            continue;
        }
        if let Some(rest) = t.strip_prefix("::") {
            if let Some((name, label, attrs)) = parse_directive(rest) {
                flush(&mut para, &mut out);
                out.push(Block::Object { name, label, attrs });
                i += 1;
                continue;
            }
        }
        if let Some(level) = (3..=6).find(|n| t.starts_with(&"#".repeat(*n)) && t[*n..].starts_with(' ')) {
            flush(&mut para, &mut out);
            out.push(Block::Heading { level: level as u8, text: t[level..].trim().to_string() });
            i += 1;
            continue;
        }
        if t == "---" || t == "***" {
            flush(&mut para, &mut out);
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            flush(&mut para, &mut out);
            let q = q.trim();
            if let Some(rest) = q.strip_prefix("[!") {
                if let Some(end) = rest.find(']') {
                    let kind = rest[..end].to_lowercase();
                    let mut text = rest[end + 1..].trim().to_string();
                    i += 1;
                    while i < lines.len() && lines[i].trim().starts_with('>') {
                        text.push(' ');
                        text.push_str(lines[i].trim()[1..].trim());
                        i += 1;
                    }
                    out.push(Block::Callout { kind, text: text.trim().to_string() });
                    continue;
                }
            }
            let (text, attrs) = split_trailing_attrs(q);
            out.push(match attrs {
                Some(attrs) => Block::Answer { text, attrs },
                None => Block::Quote { text: q.to_string() },
            });
            i += 1;
            continue;
        }
        if t.starts_with('|') {
            flush(&mut para, &mut out);
            let header = table_cells(t);
            let mut rows = vec![];
            i += 1;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                if !is_table_rule(lines[i]) {
                    rows.push(table_cells(lines[i]));
                }
                i += 1;
            }
            out.push(Block::Table { header, rows });
            continue;
        }
        if let Some((ordered, _)) = list_marker(line) {
            flush(&mut para, &mut out);
            let mut items: Vec<ListItem> = vec![];
            while i < lines.len() {
                let l = lines[i];
                if l.trim().is_empty() {
                    // A blank line ends the list unless the next line continues it.
                    if i + 1 < lines.len() && list_marker(lines[i + 1]).is_some() && !lines[i + 1].starts_with(' ') {
                        i += 1;
                        continue;
                    }
                    break;
                }
                let indented = l.starts_with("  ") || l.starts_with('\t');
                match list_marker(l) {
                    Some((_, body)) if !indented || items.is_empty() => {
                        let (checked, body) = if let Some(r) = body.strip_prefix("[ ] ") {
                            (Some(false), r)
                        } else if let Some(r) = body.strip_prefix("[x] ").or_else(|| body.strip_prefix("[X] ")) {
                            (Some(true), r)
                        } else {
                            (None, body)
                        };
                        let (text, marks) = split_marks(body);
                        items.push(ListItem { text, checked, marks, detail: vec![] });
                    }
                    _ if indented => {
                        if let Some(last) = items.last_mut() {
                            last.detail.push(l.trim().trim_start_matches("- ").to_string());
                        }
                    }
                    _ => break,
                }
                i += 1;
            }
            out.push(Block::List { ordered, items });
            continue;
        }
        para.push(t.to_string());
        i += 1;
    }
    flush(&mut para, &mut out);
    out
}

/// Parse a whole note: `# Title`, `## Page` sections, blocks.
pub fn parse(src: &str) -> NoteDoc {
    let mut doc = NoteDoc::default();
    let mut current = Page::default();
    let mut buf: Vec<&str> = vec![];
    let mut in_fence = false;
    let mut in_container = false;
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && t.starts_with(":::") {
            in_container = if t == ":::" { !in_container } else { true };
        }
        if !in_fence && !in_container {
            if let Some(title) = t.strip_prefix("# ") {
                if doc.title.is_empty() {
                    doc.title = title.trim().to_string();
                    continue;
                }
            }
            if let Some(name) = t.strip_prefix("## ") {
                current.blocks = parse_blocks(&buf);
                buf.clear();
                if !current.name.is_empty() || !current.blocks.is_empty() {
                    doc.pages.push(std::mem::take(&mut current));
                }
                current.name = name.trim().to_string();
                continue;
            }
        }
        buf.push(line);
    }
    current.blocks = parse_blocks(&buf);
    if !current.name.is_empty() || !current.blocks.is_empty() {
        doc.pages.push(current);
    }
    doc
}

impl NoteDoc {
    /// The answer line, if the first page has one.
    pub fn answer(&self) -> Option<(&str, &Attrs)> {
        self.pages.first()?.blocks.iter().find_map(|b| match b {
            Block::Answer { text, attrs } => Some((text.as_str(), attrs)),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# Desk pick

## The pick
> Buy the Fully Jarvis: steadiest under $700. {big="$599" note="on sale" icon=cart}
:::tiles
::figure[Price]{value="$599" delta="-8%"}
::status[Stock]{state=ok note="ships Friday"}
::countdown[Sale ends]{to="2026-11-12 23:59"}
:::
::sticky[Measure your desk depth first]{color=yellow}
- [x] Checked 14 reviews {new}
  Wirecutter and two forums
- [ ] Order before Friday {now}
- plain item

> [!WARNING] The 48" top is back-ordered
> until December.

| Desk | Price |
|---|---|
| Jarvis | $599 |
| Uplift | $649 |

## Sources
::link[Wirecutter]{url=https://example.com/desks}
:::pipeline[Funnel]
Visited: 4,050
Paid: 11
:::
Some closing words
over two lines.
"#;

    #[test]
    fn parses_title_pages_and_answer() {
        let d = parse(SAMPLE);
        assert_eq!(d.title, "Desk pick");
        assert_eq!(d.pages.len(), 2);
        assert_eq!(d.pages[0].name, "The pick");
        let (text, attrs) = d.answer().unwrap();
        assert_eq!(text, "Buy the Fully Jarvis: steadiest under $700.");
        assert_eq!(attrs["big"], "$599");
        assert_eq!(attrs["icon"], "cart");
    }

    #[test]
    fn parses_tiles_container_with_objects() {
        let d = parse(SAMPLE);
        let Block::Container { name, children, .. } = &d.pages[0].blocks[1] else { panic!("{:?}", d.pages[0].blocks[1]) };
        assert_eq!(name, "tiles");
        assert_eq!(children.len(), 3);
        assert!(matches!(&children[2], Block::Object { name, attrs, .. } if name == "countdown" && attrs["to"] == "2026-11-12 23:59"));
    }

    #[test]
    fn parses_lists_marks_detail_callout_table() {
        let d = parse(SAMPLE);
        let blocks = &d.pages[0].blocks;
        let Block::List { items, .. } = &blocks[3] else { panic!("{:?}", blocks[3]) };
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].checked, Some(true));
        assert_eq!(items[0].marks, vec!["new"]);
        assert_eq!(items[0].detail, vec!["Wirecutter and two forums"]);
        assert_eq!(items[1].marks, vec!["now"]);
        assert_eq!(items[2].checked, None);
        assert!(matches!(&blocks[4], Block::Callout { kind, text } if kind == "warning" && text.ends_with("until December.")));
        let Block::Table { header, rows } = &blocks[5] else { panic!() };
        assert_eq!(header, &vec!["Desk".to_string(), "Price".to_string()]);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn container_body_keeps_raw_lines_and_paragraphs_join() {
        let d = parse(SAMPLE);
        let blocks = &d.pages[1].blocks;
        assert!(matches!(&blocks[0], Block::Object { name, attrs, .. } if name == "link" && attrs["url"] == "https://example.com/desks"));
        let Block::Container { name, label, lines, .. } = &blocks[1] else { panic!() };
        assert_eq!((name.as_str(), label.as_str()), ("pipeline", "Funnel"));
        assert_eq!(lines, &vec!["Visited: 4,050".to_string(), "Paid: 11".to_string()]);
        assert_eq!(blocks[2], Block::Paragraph { text: "Some closing words over two lines.".into() });
    }

    #[test]
    fn bare_container_is_tiles_and_attrs_parse() {
        let d = parse(":::\n::figure[A]{value=1}\n::figure[B]{value=2}\n:::\n");
        assert!(matches!(&d.pages[0].blocks[0], Block::Container { name, children, .. } if name == "tiles" && children.len() == 2));
        let a = parse_attrs(r#"options=a,b,c other input="Paste the link" x"#);
        assert_eq!(a["options"], "a,b,c");
        assert_eq!(a["input"], "Paste the link");
        assert_eq!(a["other"], "");
        assert_eq!(a["x"], "");
    }

    #[test]
    fn never_fails_on_junk() {
        let d = parse("::\n:::\n{{{\n> \n- \n|");
        assert!(d.pages.len() <= 1);
        let d = parse("");
        assert!(d.pages.is_empty());
    }

    #[test]
    fn ask_with_label_containing_brackets_in_quotes() {
        let d = parse(r#"::ask[Pick one]{say="Find [cheaper] desks" options=a,b}"#);
        let Block::Object { label, attrs, .. } = &d.pages[0].blocks[0] else { panic!() };
        assert_eq!(label, "Pick one");
        assert_eq!(attrs["say"], "Find [cheaper] desks");
    }
}
