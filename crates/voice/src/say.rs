//! Text → what the natural voice should say.
//!
//! Kokoro (through sherpa-onnx) reads words from a lexicon and silently skips any word it doesn't
//! have: "opened", "GitHub", every number. So Pixie does that step itself: [`normalize`] writes
//! numbers, times, prices and symbols out as words, and [`Lexicon::phonemes`] turns the result
//! into Kokoro's phoneme symbols, with fallbacks for words the lexicon lacks (endings like -ed
//! and -s on a known word, two known words run together, acronyms spelled out, and finally
//! known pieces of the word). The voice server then speaks the phonemes directly.

use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------------------------
// Numbers and symbols → words
// ---------------------------------------------------------------------------------------------

const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen",
    "sixteen", "seventeen", "eighteen", "nineteen",
];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

/// 0 ≤ n < 10^15, in words ("one thousand two hundred thirty four").
pub fn number_words(n: u64) -> String {
    fn under_1000(n: u64, out: &mut Vec<String>) {
        let (h, r) = (n / 100, n % 100);
        if h > 0 {
            out.push(ONES[h as usize].into());
            out.push("hundred".into());
        }
        if r >= 20 {
            out.push(TENS[(r / 10) as usize].into());
            if r % 10 > 0 {
                out.push(ONES[(r % 10) as usize].into());
            }
        } else if r > 0 || h == 0 {
            out.push(ONES[r as usize].into());
        }
    }
    if n == 0 {
        return "zero".into();
    }
    let mut out = Vec::new();
    let scales = [(1_000_000_000_000, "trillion"), (1_000_000_000, "billion"), (1_000_000, "million"), (1_000, "thousand")];
    let mut rest = n;
    for (size, name) in scales {
        if rest >= size {
            under_1000(rest / size, &mut out);
            out.push(name.into());
            rest %= size;
        }
    }
    if rest > 0 {
        under_1000(rest, &mut out);
    }
    out.join(" ")
}

fn ordinal_words(n: u64) -> String {
    let w = number_words(n);
    let (head, last) = match w.rsplit_once(' ') {
        Some((h, l)) => (format!("{h} "), l.to_string()),
        None => (String::new(), w.clone()),
    };
    let last = match last.as_str() {
        "one" => "first".into(),
        "two" => "second".into(),
        "three" => "third".into(),
        "five" => "fifth".into(),
        "eight" => "eighth".into(),
        "nine" => "ninth".into(),
        "twelve" => "twelfth".into(),
        l if l.ends_with('y') => format!("{}ieth", &l[..l.len() - 1]),
        l => format!("{l}th"),
    };
    format!("{head}{last}")
}

/// A year the way people say it: 1999 → "nineteen ninety nine", 2026 → "twenty twenty six",
/// 2005 → "two thousand five".
fn year_words(n: u64) -> String {
    let (hi, lo) = (n / 100, n % 100);
    if (2000..2010).contains(&n) {
        return number_words(n);
    }
    if lo == 0 {
        return format!("{} hundred", number_words(hi));
    }
    if lo < 10 {
        return format!("{} oh {}", number_words(hi), number_words(lo));
    }
    format!("{} {}", number_words(hi), number_words(lo))
}

fn digits_words(s: &str) -> String {
    s.chars().filter_map(|c| c.to_digit(10)).map(|d| ONES[d as usize]).collect::<Vec<_>>().join(" ")
}

/// "1,234.50" → words; `None` when it isn't a plain number.
fn decimal_words(s: &str) -> Option<String> {
    let clean: String = s.chars().filter(|&c| c != ',').collect();
    let (int, frac) = match clean.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (clean.as_str(), None),
    };
    if int.is_empty() && frac.is_none() || !int.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut out = if int.is_empty() { "zero".to_string() } else if int.len() > 15 { digits_words(int) } else { number_words(int.parse().ok()?) };
    if let Some(f) = frac {
        if f.is_empty() || !f.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        out.push_str(" point ");
        out.push_str(&digits_words(f));
    }
    Some(out)
}

fn money_words(amount: &str, unit: (&str, &str), cents: (&str, &str)) -> Option<String> {
    let clean: String = amount.chars().filter(|&c| c != ',').collect();
    let (int, frac) = clean.split_once('.').unwrap_or((clean.as_str(), ""));
    let n: u64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let mut out = format!("{} {}", number_words(n), if n == 1 { unit.0 } else { unit.1 });
    if frac.len() == 2 && frac.chars().all(|c| c.is_ascii_digit()) {
        let c: u64 = frac.parse().ok()?;
        if c > 0 {
            out = format!("{out} {} {}", number_words(c), if c == 1 { cents.0 } else { cents.1 });
        }
    } else if !frac.is_empty() {
        return decimal_words(amount).map(|w| format!("{w} {}", unit.1));
    }
    Some(out)
}

fn time_words(h: u64, m: u64, ampm: Option<&str>) -> String {
    let hour = number_words(h);
    let mins = match m {
        0 if ampm.is_none() => "o'clock".to_string(),
        0 => String::new(),
        1..=9 => format!("oh {}", number_words(m)),
        _ => number_words(m),
    };
    let mut s = if mins.is_empty() { hour } else { format!("{hour} {mins}") };
    if let Some(p) = ampm {
        s.push_str(if p.starts_with('a') { " A M" } else { " P M" });
    }
    s
}

const UNITS: &[(&str, &str)] = &[
    ("km", "kilometers"),
    ("kg", "kilograms"),
    ("cm", "centimeters"),
    ("mm", "millimeters"),
    ("ms", "milliseconds"),
    ("mph", "miles per hour"),
    ("kb", "kilobytes"),
    ("mb", "megabytes"),
    ("gb", "gigabytes"),
    ("tb", "terabytes"),
    ("ghz", "gigahertz"),
    ("mhz", "megahertz"),
    ("fps", "frames per second"),
    ("hrs", "hours"),
    ("hr", "hours"),
    ("mins", "minutes"),
    ("min", "minutes"),
    ("secs", "seconds"),
    ("sec", "seconds"),
];

fn unit(s: &str) -> Option<&'static str> {
    let l = s.to_ascii_lowercase();
    UNITS.iter().find(|(k, _)| *k == l).map(|(_, v)| *v)
}

/// One whitespace-free token → words, or `None` to leave it as it is.
fn token_words(tok: &str) -> Option<String> {
    // Split off leading and trailing punctuation so "($12.50)," keeps its brackets and comma.
    let start = tok.find(|c: char| c.is_alphanumeric() || "$€£#+-".contains(c))?;
    let end = tok.rfind(|c: char| c.is_alphanumeric() || c == '%' || c == '°')? + 1;
    if start >= end {
        return None;
    }
    let (pre, core, post) = (&tok[..start], &tok[start..end], &tok[end..]);
    let words = core_words(core)?;
    Some(format!("{pre}{words}{post}"))
}

fn core_words(core: &str) -> Option<String> {
    let lower = core.to_ascii_lowercase();
    match lower.as_str() {
        "e.g" => return Some("for example".into()),
        "i.e" => return Some("that is".into()),
        "etc" => return Some("et cetera".into()),
        "vs" => return Some("versus".into()),
        _ => {}
    }
    if !core.chars().any(|c| c.is_ascii_digit()) {
        return web_words(core);
    }
    // Money.
    for (sym, unit, cents) in [('$', ("dollar", "dollars"), ("cent", "cents")), ('€', ("euro", "euros"), ("cent", "cents")), ('£', ("pound", "pounds"), ("penny", "pence"))] {
        if let Some(rest) = core.strip_prefix(sym) {
            let (num, suffix) = split_number(rest);
            let scale = match suffix.to_ascii_lowercase().as_str() {
                "k" => Some("thousand"),
                "m" => Some("million"),
                "b" | "bn" => Some("billion"),
                "" => None,
                _ => return None,
            };
            return match scale {
                Some(s) => decimal_words(num).map(|w| format!("{w} {s} {}", unit.1)),
                None => money_words(num, unit, cents),
            };
        }
    }
    if let Some(rest) = core.strip_prefix('#') {
        return decimal_words(rest).map(|w| format!("number {w}"));
    }
    if let Some(rest) = core.strip_prefix('-').filter(|r| r.starts_with(|c: char| c.is_ascii_digit())) {
        return core_words(rest).map(|w| format!("minus {w}"));
    }
    if let Some(rest) = core.strip_prefix('+').filter(|r| r.starts_with(|c: char| c.is_ascii_digit())) {
        return core_words(rest).map(|w| format!("plus {w}"));
    }
    // Times: 3:45, 12:05pm.
    if let Some((h, m)) = core.split_once(':') {
        let (mins, ampm) = split_number(m);
        let ampm = match ampm.to_ascii_lowercase().as_str() {
            "" => None,
            "am" | "a.m" => Some("am"),
            "pm" | "p.m" => Some("pm"),
            _ => return None,
        };
        let (h, m): (u64, u64) = (h.parse().ok()?, mins.parse().ok()?);
        return (h < 24 && m < 60 && mins.len() == 2).then(|| time_words(h, m, ampm));
    }
    // Ranges and ratios: 3-5, 1920x1080, 1/2, 24/7.
    for (sep, word) in [('-', "to"), ('x', "by"), ('/', "out of")] {
        if let Some((a, b)) = core.split_once(sep) {
            if !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
                if let (Some(x), Some(y)) = (core_words(a), core_words(b)) {
                    let word = if sep == '/' && b == "7" { "seven" } else { word };
                    if sep == '/' && b == "7" {
                        return Some(format!("{x} {word}"));
                    }
                    return Some(format!("{x} {word} {y}"));
                }
            }
        }
    }
    let (num, suffix) = split_number(core);
    if num.is_empty() {
        // Letters then digits: "mp3", "H2O", "v2".
        return mixed_words(core);
    }
    let suffix_l = suffix.to_ascii_lowercase();
    let n_words = || decimal_words(num);
    match suffix_l.as_str() {
        "" => {
            let plain = !num.contains(',') && !num.contains('.');
            if plain && num.len() == 4 {
                let n: u64 = num.parse().ok()?;
                if (1100..2100).contains(&n) {
                    return Some(year_words(n));
                }
            }
            if plain && num.len() > 1 && num.starts_with('0') {
                return Some(digits_words(num));
            }
            n_words()
        }
        "%" => n_words().map(|w| format!("{w} percent")),
        "°" | "°c" => n_words().map(|w| if suffix_l == "°" { format!("{w} degrees") } else { format!("{w} degrees Celsius") }),
        "°f" => n_words().map(|w| format!("{w} degrees Fahrenheit")),
        "st" | "nd" | "rd" | "th" => num.parse().ok().map(ordinal_words),
        "s" if num.len() == 4 => {
            // The 1990s.
            let n: u64 = num.parse().ok()?;
            let w = year_words(n);
            Some(if let Some(stem) = w.strip_suffix('y') { format!("{stem}ies") } else { format!("{w}s") })
        }
        "k" => n_words().map(|w| format!("{w} thousand")),
        "am" | "pm" => {
            let h: u64 = num.parse().ok()?;
            (h <= 12).then(|| time_words(h, 0, Some(&suffix_l)))
        }
        "x" => n_words().map(|w| format!("{w} times")),
        _ => match unit(suffix) {
            Some(u) => n_words().map(|w| format!("{w} {u}")),
            None => mixed_words(core),
        },
    }
}

/// Leading digits (with , and .) and whatever follows.
fn split_number(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.')).unwrap_or(s.len());
    let num = s[..end].trim_end_matches(['.', ',']);
    (num, &s[num.len()..])
}

/// "mp3" → "mp three", "H2O" → "H two O".
fn mixed_words(s: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut digits = false;
    for c in s.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                parts.push(if digits { decimal_words(&cur).unwrap_or_default() } else { std::mem::take(&mut cur) });
                cur.clear();
            }
            continue;
        }
        let d = c.is_ascii_digit();
        if !cur.is_empty() && d != digits {
            parts.push(if digits { decimal_words(&cur).unwrap_or_default() } else { cur.clone() });
            cur.clear();
        }
        digits = d;
        cur.push(c);
    }
    if !cur.is_empty() {
        parts.push(if digits { decimal_words(&cur).unwrap_or_default() } else { cur });
    }
    Some(parts.join(" "))
}

/// "github.com" → "github dot com", "me@x.io" → "me at x dot io".
fn web_words(s: &str) -> Option<String> {
    let looks_web = s.contains('@') || s.split('.').count() > 1 && s.rsplit('.').next().is_some_and(|tld| ["com", "org", "net", "io", "ai", "dev", "app", "co", "uk", "in", "edu", "gov"].contains(&tld.to_ascii_lowercase().as_str()));
    if !looks_web {
        return None;
    }
    let s = s.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
    Some(s.replace('@', " at ").replace('.', " dot ").replace('/', " slash "))
}

/// Write numbers, prices, times, percentages, units and common symbols out as words, so the
/// voice says them instead of skipping them.
pub fn normalize(text: &str) -> String {
    let text = text
        .replace(" & ", " and ")
        .replace(" + ", " plus ")
        .replace(" = ", " equals ")
        .replace(['\u{2019}', '\u{2018}'], "'")
        .replace('\u{2014}', ", ")
        .replace('\u{2013}', "-");
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        let tok = tok.replace("e.g.", "e.g").replace("i.e.", "i.e");
        match token_words(&tok) {
            Some(w) => out.push(w),
            None => out.push(tok.to_string()),
        }
    }
    out.join(" ").replace(['&'], " and ").replace('@', " at ").replace('%', " percent")
}

// ---------------------------------------------------------------------------------------------
// Words → Kokoro phonemes
// ---------------------------------------------------------------------------------------------

/// Kokoro's US English lexicon, packed: one buffer, sorted entries.
pub struct Lexicon {
    buf: String,
    /// (word start, word len, phonemes len): phonemes follow the word in `buf`.
    entries: Vec<(u32, u16, u16)>,
    /// A few spellings Pixie says often that the lexicon lacks or gets wrong.
    extra: HashMap<&'static str, &'static str>,
}

const LETTERS: [&str; 26] = [
    "ˈA", "bˈi", "sˈi", "dˈi", "ˈi", "ˈɛf", "ʤˈi", "ˈAʧ", "ˈI", "ʤˈA", "kˈA", "ˈɛl", "ˈɛm", "ˈɛn", "ˈO", "pˈi", "kjˈu", "ˈɑɹ", "ˈɛs", "tˈi", "jˈu", "vˈi",
    "dˈʌbᵊljˌu", "ˈɛks", "wˈI", "zˈi",
];

/// A rough sound per letter, the very last resort.
fn letter_sound(c: char) -> &'static str {
    match c {
        'a' => "æ",
        'b' => "b",
        'c' => "k",
        'd' => "d",
        'e' => "ɛ",
        'f' => "f",
        'g' => "ɡ",
        'h' => "h",
        'i' => "ɪ",
        'j' => "ʤ",
        'k' => "k",
        'l' => "l",
        'm' => "m",
        'n' => "n",
        'o' => "ɑ",
        'p' => "p",
        'q' => "k",
        'r' => "ɹ",
        's' => "s",
        't' => "t",
        'u' => "ʌ",
        'v' => "v",
        'w' => "w",
        'x' => "ks",
        'y' => "i",
        'z' => "z",
        _ => "",
    }
}

const VOICELESS: &[char] = &['p', 't', 'k', 'f', 'θ', 's', 'ʃ', 'ʧ'];
const SIBILANT: &[char] = &['s', 'z', 'ʃ', 'ʒ', 'ʧ', 'ʤ'];

fn last_sound(ph: &str) -> char {
    ph.chars().rev().find(|c| !matches!(c, 'ˈ' | 'ˌ' | 'ː')).unwrap_or('ə')
}

/// Primary stress only on the first part of a joined word.
fn demote(ph: &str) -> String {
    ph.replace('ˈ', "ˌ")
}

impl Lexicon {
    /// Load `lexicon-us-en.txt` ("word p h o n e s" per line).
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut rows: Vec<(&str, String)> = Vec::with_capacity(180_000);
        for line in text.lines() {
            let mut it = line.split(' ');
            let Some(word) = it.next().filter(|w| !w.is_empty()) else { continue };
            let ph: String = it.collect();
            if !ph.is_empty() && word.len() < u16::MAX as usize && ph.len() < u16::MAX as usize {
                rows.push((word, ph));
            }
        }
        Ok(Self::from_rows(rows))
    }

    fn from_rows(mut rows: Vec<(&str, String)>) -> Self {
        rows.sort_by(|a, b| a.0.cmp(b.0));
        rows.dedup_by(|a, b| a.0 == b.0);
        let mut buf = String::with_capacity(rows.iter().map(|r| r.0.len() + r.1.len()).sum());
        let mut entries = Vec::with_capacity(rows.len());
        for (w, p) in rows {
            entries.push((buf.len() as u32, w.len() as u16, p.len() as u16));
            buf.push_str(w);
            buf.push_str(&p);
        }
        let extra = HashMap::from([("a", "ɐ"), ("github", "ɡˈɪthˌʌb"), ("pixie", "pˈɪksi"), ("excalidraw", "ɛkskˈælɪdɹˌɔ"), ("opencode", "ˈOpᵊnkˌOd"), ("deepseek", "dˈipsˌik"), ("macos", "mˌækˌOˈɛs"), ("ios", "ˌIˌOˈɛs")]);
        Self { buf, entries, extra }
    }

    #[cfg(test)]
    fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self::from_rows(pairs.iter().map(|(w, p)| (*w, p.to_string())).collect())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn get(&self, word: &str) -> Option<&str> {
        if let Some(p) = self.extra.get(word) {
            return Some(p);
        }
        let i = self
            .entries
            .binary_search_by(|&(at, wl, _)| self.buf[at as usize..at as usize + wl as usize].cmp(word))
            .ok()?;
        let (at, wl, pl) = self.entries[i];
        let s = at as usize + wl as usize;
        Some(&self.buf[s..s + pl as usize])
    }

    /// One word (lowercase letters and apostrophes) → phonemes; never empty for a real word.
    fn word(&self, original: &str) -> String {
        let w = original.to_lowercase();
        if let Some(p) = self.get(&w) {
            return p.to_string();
        }
        if w.contains('-') {
            // "wi-fi" is in the lexicon; "re-opened" reads as "reopened"; else part by part.
            let joined = w.replace('-', "");
            if let Some(p) = self.get(&joined).map(str::to_string).or_else(|| self.inflected(&joined)) {
                return p;
            }
            return original.split('-').filter(|x| !x.is_empty()).map(|x| self.word(x)).collect::<Vec<_>>().join(" ");
        }
        // An acronym ("API", "URL"), unless the lexicon knows it as a word.
        let caps = original.chars().all(|c| c.is_ascii_uppercase());
        if caps && (2..=5).contains(&original.len()) {
            return self.spell(&w);
        }
        if let Some(p) = self.inflected(&w) {
            return p;
        }
        if let Some(p) = self.compound(&w) {
            return p;
        }
        self.pieces(&w)
    }

    fn spell(&self, w: &str) -> String {
        let letters: Vec<&str> = w.chars().filter(|c| c.is_ascii_lowercase()).map(|c| LETTERS[(c as u8 - b'a') as usize]).collect();
        let n = letters.len();
        letters.iter().enumerate().map(|(i, l)| if i + 1 == n { l.to_string() } else { demote(l) }).collect()
    }

    /// Known word + an ending: opened, stopped, uses, boxes, tries, making, faster, quickly…
    fn inflected(&self, w: &str) -> Option<String> {
        let try_stems = |stem: &str| -> Option<String> {
            if stem.len() < 2 {
                return None;
            }
            let b = stem.as_bytes();
            let doubled = b.len() >= 3 && b[b.len() - 1] == b[b.len() - 2];
            self.get(stem)
                .map(str::to_string)
                .or_else(|| self.get(&format!("{stem}e")).map(str::to_string))
                .or_else(|| doubled.then(|| self.get(&stem[..stem.len() - 1]).map(str::to_string)).flatten())
                .or_else(|| stem.strip_suffix('i').and_then(|s| self.get(&format!("{s}y"))).map(str::to_string))
        };
        if let Some(s) = w.strip_suffix("'s").or_else(|| w.strip_suffix("s'")) {
            let base = self.get(s).map(str::to_string).or_else(|| self.compound(s))?;
            return Some(with_s(&base));
        }
        if let Some(stem) = w.strip_suffix("ed") {
            if let Some(base) = try_stems(stem) {
                let last = last_sound(&base);
                let end = if last == 't' || last == 'd' { "ɪd" } else if VOICELESS.contains(&last) { "t" } else { "d" };
                return Some(format!("{base}{end}"));
            }
        }
        if let Some(stem) = w.strip_suffix("ing") {
            if let Some(base) = try_stems(stem) {
                return Some(format!("{base}ɪŋ"));
            }
        }
        if let Some(stem) = w.strip_suffix("es").filter(|s| s.ends_with(['s', 'x', 'z', 'h']) || s.ends_with('i')) {
            if let Some(base) = try_stems(stem) {
                return Some(with_s(&base));
            }
        }
        if let Some(stem) = w.strip_suffix('s') {
            if let Some(base) = self.get(stem).map(str::to_string).or_else(|| self.compound(stem)) {
                return Some(with_s(&base));
            }
        }
        for (suffix, ph) in [("ers", "əɹz"), ("er", "əɹ"), ("est", "əst"), ("ly", "li"), ("ness", "nəs"), ("ment", "mənt"), ("ments", "mənts"), ("ful", "fəl"), ("less", "ləs"), ("able", "əbᵊl"), ("ish", "ɪʃ")] {
            if let Some(stem) = w.strip_suffix(suffix) {
                if let Some(base) = try_stems(stem) {
                    return Some(format!("{base}{ph}"));
                }
            }
        }
        for (prefix, ph) in [("re", "ɹˌi"), ("un", "ˌʌn"), ("pre", "pɹˌi"), ("non", "nˌɑn"), ("mis", "mˌɪs"), ("over", "ˌOvəɹ"), ("auto", "ˌɔɾO")] {
            if let Some(rest) = w.strip_prefix(prefix).filter(|r| r.len() >= 3) {
                if let Some(base) = self.get(rest).map(str::to_string).or_else(|| self.inflected(rest)) {
                    return Some(format!("{ph}{base}"));
                }
            }
        }
        None
    }

    /// Two known words run together: github → git + hub, screenshots → screen + shots.
    fn compound(&self, w: &str) -> Option<String> {
        if w.len() < 6 {
            return None;
        }
        let mut best: Option<(usize, String)> = None;
        for i in 3..=w.len().saturating_sub(3) {
            if !w.is_char_boundary(i) {
                continue;
            }
            let (a, b) = (&w[..i], &w[i..]);
            if let (Some(pa), Some(pb)) = (self.get(a), self.get(b).map(str::to_string).or_else(|| self.inflected(b))) {
                // Prefer the most even split (fewer accidental short words).
                let score = a.len().min(b.len());
                if best.as_ref().is_none_or(|(s, _)| score > *s) {
                    best = Some((score, format!("{pa}{}", demote(&pb))));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Last resort: cover the word with the fewest known pieces (3+ letters), filling gaps with
    /// rough letter sounds. "excalidraw" → "ex·cal·i·draw"-like, never silence.
    fn pieces(&self, w: &str) -> String {
        let chars: Vec<char> = w.chars().collect();
        let n = chars.len();
        // cost[i] = best cost to cover chars[..i]; from[i] = (start, phonemes).
        let mut cost = vec![u32::MAX; n + 1];
        let mut from: Vec<(usize, String)> = vec![(0, String::new()); n + 1];
        cost[0] = 0;
        for i in 0..n {
            if cost[i] == u32::MAX {
                continue;
            }
            let ls = letter_sound(chars[i]);
            if cost[i] + 3 < cost[i + 1] {
                cost[i + 1] = cost[i] + 3;
                from[i + 1] = (i, ls.to_string());
            }
            for j in (i + 3)..=n.min(i + 12) {
                let piece: String = chars[i..j].iter().collect();
                if let Some(p) = self.get(&piece) {
                    let c = cost[i] + 2;
                    if c < cost[j] {
                        cost[j] = c;
                        from[j] = (i, p.to_string());
                    }
                }
            }
        }
        let mut parts = Vec::new();
        let mut at = n;
        while at > 0 {
            let (s, p) = from[at].clone();
            parts.push(p);
            at = s;
        }
        parts.reverse();
        let mut out = String::new();
        let mut stressed = false;
        for p in parts {
            if stressed {
                out.push_str(&demote(&p));
            } else {
                stressed = p.contains('ˈ');
                out.push_str(&p);
            }
        }
        if !out.contains('ˈ') {
            // Give the word a stress so it isn't swallowed.
            if let Some(i) = out.char_indices().find(|(_, c)| "aeiouæɑɛɪʌəɔAIOWYɜ".contains(*c)).map(|(i, _)| i) {
                out.insert(i, 'ˈ');
            }
        }
        out
    }

    /// Normalized text → Kokoro phonemes: words separated by spaces, Kokoro's punctuation kept.
    pub fn phonemes(&self, text: &str) -> String {
        let mut out = String::new();
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut String| {
            if word.is_empty() {
                return;
            }
            let w = word.trim_matches('\'');
            if !w.is_empty() {
                if !out.is_empty() && !out.ends_with(' ') && !out.ends_with(['(', '"', '“']) {
                    out.push(' ');
                }
                if w.chars().any(|c| c.is_ascii_digit()) {
                    let words = normalize(w);
                    let parts: Vec<String> = words.split_whitespace().map(|x| self.word(x)).collect();
                    out.push_str(&parts.join(" "));
                } else {
                    out.push_str(&self.word(w));
                }
            }
            word.clear();
        };
        let chars: Vec<char> = text.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            let joins = c == '-' && !word.is_empty() && chars.get(i + 1).is_some_and(|n| n.is_alphabetic());
            if c.is_alphanumeric() || c == '\'' || joins {
                word.push(c);
                continue;
            }
            flush(&mut word, &mut out);
            match c {
                ',' | '.' | '!' | '?' | ';' | ':' | '…' => {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    if !out.is_empty() && !out.ends_with([',', '.', '!', '?', ';', ':', '…']) {
                        out.push(c);
                    }
                }
                '(' | ')' | '"' | '“' | '”' => {
                    // Brackets and quotes read as a light pause.
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    if !out.is_empty() && !out.ends_with([',', '.', '!', '?', ';', ':', '…']) {
                        out.push(',');
                    }
                }
                _ => {
                    if !out.is_empty() && !out.ends_with(' ') {
                        out.push(' ');
                    }
                }
            }
        }
        flush(&mut word, &mut out);
        let t = out.trim().trim_start_matches([',', '.', ';', ':']).trim();
        let mut t = t.to_string();
        if !t.is_empty() && !t.ends_with(['.', '!', '?', '…']) {
            t = t.trim_end_matches([',', ';', ':']).to_string();
            t.push('.');
        }
        t
    }
}

fn with_s(base: &str) -> String {
    let last = last_sound(base);
    let end = if SIBILANT.contains(&last) { "ɪz" } else if VOICELESS.contains(&last) { "s" } else { "z" };
    format!("{base}{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_symbols_become_words() {
        assert_eq!(normalize("It costs $12.50 now."), "It costs twelve dollars fifty cents now.");
        assert_eq!(normalize("at 3:45, or 9:05pm"), "at three forty five, or nine oh five P M");
        assert_eq!(normalize("50% of 1,234"), "fifty percent of one thousand two hundred thirty four");
        assert_eq!(normalize("In 2026 and 1999"), "In twenty twenty six and nineteen ninety nine");
        assert_eq!(normalize("the 21st try"), "the twenty first try");
        assert_eq!(normalize("3.5 km away"), "three point five km away");
        assert_eq!(normalize("3.5km away"), "three point five kilometers away");
        assert_eq!(normalize("Pages 3-5"), "Pages three to five");
        assert_eq!(normalize("Go to github.com"), "Go to github dot com");
        assert_eq!(normalize("Tom & Jerry"), "Tom and Jerry");
        assert_eq!(normalize("(about $5)"), "(about five dollars)");
        assert_eq!(normalize("e.g. this"), "for example this");
        assert_eq!(normalize("an mp3 file"), "an mp three file");
        assert_eq!(normalize("10:00"), "ten o'clock");
        assert_eq!(normalize("$1.2M raised"), "one point two million dollars raised");
    }

    fn lex() -> Lexicon {
        Lexicon::from_pairs(&[
            ("open", "ˈOpᵊn"),
            ("stop", "stˈɑp"),
            ("box", "bˈɑks"),
            ("make", "mˈAk"),
            ("git", "ɡˈɪt"),
            ("hub", "hˈʌb"),
            ("screen", "skɹˈin"),
            ("shot", "ʃˈɑt"),
            ("want", "wˈɑnt"),
            ("hello", "həlˈO"),
            ("world", "wˈɜɹld"),
            ("three", "θɹˈi"),
            ("draw", "dɹˈɔ"),
        ])
    }

    #[test]
    fn missing_words_still_get_sounds() {
        let l = lex();
        assert_eq!(l.word("opened"), "ˈOpᵊnd");
        assert_eq!(l.word("stopped"), "stˈɑpt");
        assert_eq!(l.word("wanted"), "wˈɑntɪd");
        assert_eq!(l.word("boxes"), "bˈɑksɪz");
        assert_eq!(l.word("making"), "mˈAkɪŋ");
        assert_eq!(l.word("opens"), "ˈOpᵊnz");
        assert_eq!(l.word("screenshots"), "skɹˈinʃˌɑts");
        assert_eq!(l.word("API"), "ˌApˌiˈI");
        assert!(!l.word("zorblax").is_empty());
        assert!(l.word("redraw").ends_with("dɹˈɔ"));
    }

    #[test]
    fn sentences_keep_words_and_pauses() {
        let l = lex();
        assert_eq!(l.phonemes("Hello, world!"), "həlˈO, wˈɜɹld!");
        assert_eq!(l.phonemes("Opened 3 boxes"), "ˈOpᵊnd θɹˈi bˈɑksɪz.");
        assert_eq!(l.phonemes("Hello (world)"), "həlˈO, wˈɜɹld.");
        assert_eq!(l.phonemes("re-opened git-hub"), "ɹˌiˈOpᵊnd ɡˈɪthˌʌb.");
    }
}

#[cfg(test)]
mod real {
    /// `LEXICON=…/lexicon-us-en.txt cargo test -p pixie-voice real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_lexicon_samples() {
        let Ok(path) = std::env::var("LEXICON") else { return };
        let t = std::time::Instant::now();
        let l = super::Lexicon::load(std::path::Path::new(&path)).unwrap();
        println!("loaded {} words in {:?}", l.len(), t.elapsed());
        for s in [
            "Pixie opened Excalidraw at 3:45, it costs $12.50 and GitHub says OK.",
            "I clicked the Wi-Fi toggle in System Settings and it's off now.",
            "Your API key wanted a refresh, so I re-opened the screenshots folder.",
            "The 2026 roadmap lists 3 things: opencode, Codex and Claude Code.",
        ] {
            let n = super::normalize(s);
            println!("{s}\n  {n}\n  {}", l.phonemes(&n));
        }
    }
}
