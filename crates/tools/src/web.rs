//! `fetch_url` and `web_search` (no API key: DuckDuckGo's HTML page, Bing as a fallback).

use regex::Regex;
use std::sync::OnceLock;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(25))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()
            .expect("http client")
    })
}

fn re(pat: &str) -> Regex {
    Regex::new(pat).expect("valid regex")
}

pub fn decode_entities(s: &str) -> String {
    static NUM: OnceLock<Regex> = OnceLock::new();
    let num = NUM.get_or_init(|| re(r"&#(x?[0-9a-fA-F]+);"));
    let s = num.replace_all(s, |c: &regex::Captures| {
        let v = &c[1];
        let n = if let Some(h) = v.strip_prefix('x').or_else(|| v.strip_prefix('X')) {
            u32::from_str_radix(h, 16).ok()
        } else {
            v.parse().ok()
        };
        n.and_then(char::from_u32).map(String::from).unwrap_or_default()
    });
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&rsquo;", "’")
        .replace("&lsquo;", "‘")
        .replace("&ldquo;", "“")
        .replace("&rdquo;", "”")
        .replace("&amp;", "&")
}

/// Readable text from HTML: drop scripts/styles/nav chrome, keep block breaks.
pub fn html_to_text(html: &str) -> String {
    static RES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RES.get_or_init(|| {
        vec![
            (re(r"(?is)<(script|style|noscript|svg|template|iframe)\b.*?</(script|style|noscript|svg|template|iframe)>"), " "),
            (re(r"(?s)<!--.*?-->"), " "),
            (re(r"(?is)<(nav|footer|aside)\b.*?</(nav|footer|aside)>"), "\n"),
            (re(r"(?i)<br\s*/?>"), "\n"),
            (re(r"(?i)<li\b[^>]*>"), "\n- "),
            (re(r"(?i)<h[1-6]\b[^>]*>"), "\n\n## "),
            (re(r"(?i)</(p|div|section|article|h[1-6]|tr|table|ul|ol|blockquote|header|main|figure)>"), "\n"),
            (re(r"(?i)</t[dh]>"), " | "),
            (re(r"(?s)<[^>]+>"), ""),
            (re(r"[ \t\u{a0}]+"), " "),
            (re(r"\n[ \t]+"), "\n"),
            (re(r"\n{3,}"), "\n\n"),
        ]
    });
    let mut s = html.to_string();
    for (r, with) in rules.iter() {
        s = r.replace_all(&s, *with).to_string();
    }
    decode_entities(&s).trim().to_string()
}

fn title_of(html: &str) -> String {
    static T: OnceLock<Regex> = OnceLock::new();
    T.get_or_init(|| re(r"(?is)<title[^>]*>(.*?)</title>"))
        .captures(html)
        .map(|c| decode_entities(c[1].trim()))
        .unwrap_or_default()
}

pub struct Fetched {
    pub text: String,
    pub bytes: Option<Vec<u8>>,
    pub content_type: String,
}

pub async fn fetch(url: &str) -> Result<Fetched, String> {
    let url = if url.starts_with("http://") || url.starts_with("https://") { url.to_string() } else { format!("https://{url}") };
    let resp = client().get(&url).send().await.map_err(|e| format!("Couldn't load {url}: {e}"))?;
    let status = resp.status();
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    let final_url = resp.url().to_string();
    let body = resp.bytes().await.map_err(|e| format!("Couldn't read {url}: {e}"))?;
    if !status.is_success() {
        return Err(format!("{url} answered {status}"));
    }
    if ct.contains("pdf") || ct.starts_with("image/") || ct.contains("octet-stream") || ct.contains("zip") {
        return Ok(Fetched { text: String::new(), bytes: Some(body.to_vec()), content_type: ct });
    }
    let raw = String::from_utf8_lossy(&body).to_string();
    let text = if ct.contains("html") || raw.trim_start().starts_with('<') {
        let t = title_of(&raw);
        let body = html_to_text(&raw);
        if t.is_empty() { format!("URL: {final_url}\n\n{body}") } else { format!("# {t}\nURL: {final_url}\n\n{body}") }
    } else {
        raw
    };
    Ok(Fetched { text, bytes: None, content_type: ct })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

fn ddg_url(href: &str) -> String {
    let full = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    if let Ok(u) = url::Url::parse(&full) {
        if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "uddg") {
            return v.to_string();
        }
    }
    full
}

pub fn parse_ddg(html: &str) -> Vec<Hit> {
    static A: OnceLock<Regex> = OnceLock::new();
    static S: OnceLock<Regex> = OnceLock::new();
    let a = A.get_or_init(|| re(r#"(?s)<a[^>]*class="result__a"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#));
    let s = S.get_or_init(|| re(r#"(?s)class="result__snippet"[^>]*>(.*?)</(a|div|td)>"#));
    let snippets: Vec<String> = s.captures_iter(html).map(|c| html_to_text(&c[1])).collect();
    a.captures_iter(html)
        .enumerate()
        .map(|(i, c)| Hit {
            title: html_to_text(&c[2]),
            url: ddg_url(&decode_entities(&c[1])),
            snippet: snippets.get(i).cloned().unwrap_or_default(),
        })
        .filter(|h| !h.url.contains("duckduckgo.com/y.js"))
        .collect()
}

pub fn parse_bing(html: &str) -> Vec<Hit> {
    static B: OnceLock<Regex> = OnceLock::new();
    let b = B.get_or_init(|| re(r#"(?s)<li class="b_algo".*?<h2[^>]*>\s*<a[^>]*href="([^"]+)"[^>]*>(.*?)</a>.*?(?:<p[^>]*>(.*?)</p>|</li>)"#));
    b.captures_iter(html)
        .map(|c| Hit {
            title: html_to_text(&c[2]),
            url: decode_entities(&c[1]),
            snippet: c.get(3).map(|m| html_to_text(m.as_str())).unwrap_or_default(),
        })
        .collect()
}

pub async fn search(query: &str, count: usize) -> Result<Vec<Hit>, String> {
    let ddg = client()
        .post("https://html.duckduckgo.com/html/")
        .form(&[("q", query), ("kl", "")])
        .send()
        .await;
    if let Ok(r) = ddg {
        if r.status().is_success() {
            let hits = parse_ddg(&r.text().await.unwrap_or_default());
            if !hits.is_empty() {
                return Ok(hits.into_iter().take(count).collect());
            }
        }
    }
    let url = format!("https://www.bing.com/search?q={}&setlang=en", url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>());
    let r = client().get(url).send().await.map_err(|e| format!("Search failed: {e}"))?;
    let hits = parse_bing(&r.text().await.unwrap_or_default());
    if hits.is_empty() {
        return Err("The search came back empty.".into());
    }
    Ok(hits.into_iter().take(count).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_text_is_readable() {
        let t = html_to_text("<html><head><style>x{}</style></head><body><h1>Hi&amp;bye</h1><p>One<br>Two</p><script>evil()</script><ul><li>a</li><li>b</li></ul></body></html>");
        assert!(t.contains("## Hi&bye"));
        assert!(t.contains("One\nTwo"));
        assert!(t.contains("- a") && t.contains("- b"));
        assert!(!t.contains("evil"));
    }

    #[test]
    fn ddg_results_parse() {
        let html = r#"<div class="result"><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdesks&amp;rut=abc">Best <b>desks</b></a>
        <a class="result__snippet" href="x">Steady &amp; cheap</a></div>"#;
        let hits = parse_ddg(html);
        assert_eq!(hits, vec![Hit { title: "Best desks".into(), url: "https://example.com/desks".into(), snippet: "Steady & cheap".into() }]);
    }

    #[test]
    fn entities() {
        assert_eq!(decode_entities("&#8217;&#x41;&lt;"), "’A<");
    }
}
