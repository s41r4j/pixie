//! Google account: Gmail and Google Calendar straight from Google's APIs.
//!
//! Sign-in is OAuth for installed apps (loopback redirect + PKCE) with the user's own OAuth client
//! (Google Cloud → APIs & Services → Credentials → "Desktop app"), typed into Settings → Apps or set
//! as GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET. Tokens stay in `<app_data>/google.json` (owner-only).
//! Pixie reads mail, writes *drafts* (never sends) and reads/adds calendar events.

use crate::AppState;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use openclicky_llm::ToolDef;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const SCOPES: &str = "openid email https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose https://www.googleapis.com/auth/calendar.events";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Tokens {
    refresh_token: String,
    access_token: String,
    /// Unix seconds.
    expires_at: i64,
    email: String,
}

fn file(dir: &Path) -> PathBuf {
    dir.join("google.json")
}

fn load(dir: &Path) -> Option<Tokens> {
    std::fs::read_to_string(file(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).filter(|t: &Tokens| !t.refresh_token.is_empty())
}

fn save(dir: &Path, t: &Tokens) {
    let p = file(dir);
    if let Ok(s) = serde_json::to_string_pretty(t) {
        let _ = std::fs::write(&p, s);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

fn client(app: &AppHandle) -> Option<(String, String)> {
    let st = app.state::<AppState>();
    let s = st.settings.lock().unwrap().clone();
    let vars = crate::config::vars(&st.data_dir, &s);
    let id = Some(s.google_client_id.trim().to_string()).filter(|x| !x.is_empty()).or_else(|| vars.get("GOOGLE_CLIENT_ID").cloned())?;
    let secret = Some(s.google_client_secret.trim().to_string()).filter(|x| !x.is_empty()).or_else(|| vars.get("GOOGLE_CLIENT_SECRET").cloned()).unwrap_or_default();
    Some((id, secret))
}

pub fn signed_in(app: &AppHandle) -> bool {
    load(&app.state::<AppState>().data_dir).is_some()
}

pub fn status(app: &AppHandle) -> Value {
    let t = load(&app.state::<AppState>().data_dir);
    json!({ "configured": client(app).is_some(), "signed_in": t.is_some(), "email": t.map(|t| t.email).unwrap_or_default() })
}

pub fn sign_out(app: &AppHandle) {
    let dir = app.state::<AppState>().data_dir.clone();
    if let Some(t) = load(&dir) {
        // Best effort: revoke the grant at Google too.
        tauri::async_runtime::spawn(async move {
            let _ = reqwest::Client::new().post("https://oauth2.googleapis.com/revoke").form(&[("token", t.refresh_token)]).send().await;
        });
    }
    let _ = std::fs::remove_file(file(&dir));
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("/usr/bin/open").arg(url).status();
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).status();
    #[cfg(not(any(target_os = "macos", windows)))]
    let r = std::process::Command::new("xdg-open").arg(url).status();
    r.map(|_| ()).map_err(|e| e.to_string())
}

fn random_token(n: usize) -> String {
    let bytes: Vec<u8> = (0..n).map(|_| rand::random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Open Google's consent page and wait (up to 5 minutes) for the redirect back to Pixie.
pub async fn sign_in(app: &AppHandle) -> Result<String, String> {
    let (id, secret) = client(app).ok_or("Add your Google OAuth client ID first (Settings → Apps → Google).")?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = random_token(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token(16);
    let mut url = Url::parse(AUTH).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    open_browser(url.as_str())?;

    let code = tokio::time::timeout(Duration::from_secs(300), async {
        loop {
            let (mut sock, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let u = Url::parse(&format!("http://127.0.0.1{path}")).map_err(|e| e.to_string())?;
            let get = |k: &str| u.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string());
            if get("state").is_none() && get("error").is_none() {
                // favicon or a stray request
                let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
                continue;
            }
            let ok = get("state").as_deref() == Some(state.as_str()) && get("code").is_some();
            let page = if ok {
                "<h2>Pixie is connected to Google.</h2><p>You can close this tab.</p>"
            } else {
                "<h2>Pixie couldn't connect.</h2><p>Close this tab and try again from Settings.</p>"
            };
            let html = format!("<!doctype html><meta charset=utf-8><title>Pixie</title><body style=\"font:16px -apple-system,system-ui,sans-serif;max-width:32em;margin:18vh auto;text-align:center;color:#222\">{page}</body>");
            let _ = sock
                .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len()).as_bytes())
                .await;
            return if ok { Ok(get("code").unwrap()) } else { Err(get("error").unwrap_or_else(|| "sign-in was cancelled".into())) };
        }
    })
    .await
    .map_err(|_| "Sign-in timed out.".to_string())??;

    let mut form = vec![
        ("client_id", id.as_str()),
        ("code", code.as_str()),
        ("code_verifier", verifier.as_str()),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect.as_str()),
    ];
    if !secret.is_empty() {
        form.push(("client_secret", secret.as_str()));
    }
    let v: Value = reqwest::Client::new().post(TOKEN).form(&form).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let refresh = v["refresh_token"].as_str().ok_or_else(|| format!("Google didn't return a token: {}", v["error_description"].as_str().or(v["error"].as_str()).unwrap_or("unknown error")))?;
    let email = v["id_token"].as_str().and_then(jwt_email).unwrap_or_default();
    let t = Tokens {
        refresh_token: refresh.into(),
        access_token: v["access_token"].as_str().unwrap_or("").into(),
        expires_at: chrono::Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(0) - 60,
        email: email.clone(),
    };
    save(&app.state::<AppState>().data_dir, &t);
    Ok(email)
}

/// The email claim of an ID token (straight from Google's token endpoint over TLS).
fn jwt_email(jwt: &str) -> Option<String> {
    let body = jwt.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(body.trim_end_matches('=')).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()?["email"].as_str().map(str::to_string)
}

async fn access_token(app: &AppHandle) -> Result<String, String> {
    let dir = app.state::<AppState>().data_dir.clone();
    let mut t = load(&dir).ok_or("Google isn't connected; they can sign in under Settings → Apps.")?;
    if !t.access_token.is_empty() && t.expires_at > chrono::Utc::now().timestamp() {
        return Ok(t.access_token);
    }
    let (id, secret) = client(app).ok_or("The Google client ID is missing in Settings → Apps.")?;
    let mut form = vec![("client_id", id.as_str()), ("refresh_token", t.refresh_token.as_str()), ("grant_type", "refresh_token")];
    if !secret.is_empty() {
        form.push(("client_secret", secret.as_str()));
    }
    let v: Value = reqwest::Client::new().post(TOKEN).form(&form).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let at = v["access_token"].as_str().ok_or_else(|| format!("Google sign-in expired ({}); sign in again in Settings → Apps.", v["error"].as_str().unwrap_or("no token")))?;
    t.access_token = at.into();
    t.expires_at = chrono::Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(3600) - 60;
    save(&dir, &t);
    Ok(t.access_token)
}

async fn api(app: &AppHandle, method: reqwest::Method, url: &str, body: Option<Value>) -> Result<Value, String> {
    let token = access_token(app).await?;
    let mut req = reqwest::Client::new().request(method, url).bearer_auth(token).timeout(Duration::from_secs(30));
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(format!("Google said {status}: {}", v["error"]["message"].as_str().unwrap_or("")));
    }
    Ok(v)
}

// ---- tools -------------------------------------------------------------------------------

pub fn tools(app: &AppHandle) -> Vec<ToolDef> {
    if !signed_in(app) {
        return vec![];
    }
    vec![
        ToolDef {
            name: "gmail".into(),
            description: "The user's Gmail. action=search takes a Gmail query (from:, subject:, is:unread, newer_than:2d …; empty = latest inbox) and lists messages; action=read returns one message by id; action=draft saves a draft (to, subject, body, cc; reply_to=<message id> threads it as a reply) for them to review and send in Gmail. You never send mail.".into(),
            input_schema: json!({ "type": "object", "properties": {
                "action": { "type": "string", "enum": ["search", "read", "draft"] },
                "query": { "type": "string" }, "max": { "type": "integer" }, "id": { "type": "string" },
                "to": { "type": "string" }, "cc": { "type": "string" }, "subject": { "type": "string" },
                "body": { "type": "string" }, "reply_to": { "type": "string" }
            }, "required": ["action"] }),
        },
        ToolDef {
            name: "google_calendar".into(),
            description: "The user's primary Google Calendar. action=events lists events between from and to (ISO local times; default now → 7 days), optionally matching query. action=add creates an event (title, start, end, location, notes); only when they asked.".into(),
            input_schema: json!({ "type": "object", "properties": {
                "action": { "type": "string", "enum": ["events", "add"] },
                "from": { "type": "string" }, "to": { "type": "string" }, "query": { "type": "string" },
                "title": { "type": "string" }, "start": { "type": "string" }, "end": { "type": "string" },
                "location": { "type": "string" }, "notes": { "type": "string" }
            }, "required": ["action"] }),
        },
    ]
}

pub fn handles(name: &str) -> bool {
    matches!(name, "gmail" | "google_calendar")
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

pub async fn call(app: &AppHandle, name: &str, input: &Value) -> Result<String, String> {
    match (name, s(input, "action")) {
        ("gmail", "read") => gmail_read(app, s(input, "id")).await,
        ("gmail", "draft") => gmail_draft(app, input).await,
        ("gmail", _) => gmail_search(app, s(input, "query"), input.get("max").and_then(Value::as_u64).unwrap_or(10)).await,
        ("google_calendar", "add") => cal_add(app, input).await,
        ("google_calendar", _) => cal_events(app, input).await,
        _ => Err("unknown tool".into()),
    }
}

const GMAIL: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

fn header(msg: &Value, name: &str) -> String {
    msg["payload"]["headers"]
        .as_array()
        .and_then(|h| h.iter().find(|x| x["name"].as_str().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false)))
        .and_then(|x| x["value"].as_str())
        .unwrap_or("")
        .to_string()
}

async fn gmail_search(app: &AppHandle, query: &str, max: u64) -> Result<String, String> {
    let mut url = Url::parse(&format!("{GMAIL}/messages")).unwrap();
    url.query_pairs_mut().append_pair("maxResults", &max.clamp(1, 25).to_string());
    if query.trim().is_empty() {
        url.query_pairs_mut().append_pair("labelIds", "INBOX");
    } else {
        url.query_pairs_mut().append_pair("q", query);
    }
    let list = api(app, reqwest::Method::GET, url.as_str(), None).await?;
    let ids: Vec<String> = list["messages"].as_array().map(|a| a.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    if ids.is_empty() {
        return Ok("No messages.".into());
    }
    let mut out = vec!["id\tdate\tfrom\tsubject\tsnippet".to_string()];
    for id in ids {
        let m = api(app, reqwest::Method::GET, &format!("{GMAIL}/messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=Subject&metadataHeaders=Date"), None).await?;
        let unread = m["labelIds"].as_array().map(|l| l.iter().any(|x| x == "UNREAD")).unwrap_or(false);
        out.push(format!(
            "{id}\t{}\t{}\t{}{}\t{}",
            header(&m, "Date"),
            header(&m, "From"),
            if unread { "[unread] " } else { "" },
            header(&m, "Subject"),
            m["snippet"].as_str().unwrap_or("")
        ));
    }
    Ok(out.join("\n"))
}

/// Text of a message: text/plain parts, else HTML with the tags stripped.
fn body_text(part: &Value, out_plain: &mut String, out_html: &mut String) {
    let mime = part["mimeType"].as_str().unwrap_or("");
    if let Some(data) = part["body"]["data"].as_str() {
        let bytes = URL_SAFE.decode(data).or_else(|_| URL_SAFE_NO_PAD.decode(data.trim_end_matches('='))).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        if mime == "text/plain" {
            out_plain.push_str(&text);
        } else if mime == "text/html" {
            out_html.push_str(&text);
        }
    }
    if let Some(parts) = part["parts"].as_array() {
        for p in parts {
            body_text(p, out_plain, out_html);
        }
    }
}

fn strip_html(html: &str) -> String {
    let no_style = regex_lite_strip(html);
    no_style.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn regex_lite_strip(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&#39;", "'").replace("&quot;", "\"")
}

async fn gmail_read(app: &AppHandle, id: &str) -> Result<String, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("read needs a message id from search".into());
    }
    let m = api(app, reqwest::Method::GET, &format!("{GMAIL}/messages/{id}?format=full"), None).await?;
    let (mut plain, mut html) = (String::new(), String::new());
    body_text(&m["payload"], &mut plain, &mut html);
    let body = if plain.trim().is_empty() { strip_html(&html) } else { plain };
    let text = format!(
        "From: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
        header(&m, "From"),
        header(&m, "To"),
        header(&m, "Date"),
        header(&m, "Subject"),
        body.trim()
    );
    Ok(text.chars().take(12_000).collect())
}

async fn gmail_draft(app: &AppHandle, input: &Value) -> Result<String, String> {
    let to = s(input, "to");
    if to.trim().is_empty() && s(input, "reply_to").is_empty() {
        return Err("A draft needs a to address (or reply_to).".into());
    }
    let mut subject = s(input, "subject").to_string();
    let mut to = to.to_string();
    let mut extra = String::new();
    let mut thread = None;
    let reply = s(input, "reply_to");
    if !reply.is_empty() {
        let m = api(app, reqwest::Method::GET, &format!("{GMAIL}/messages/{reply}?format=metadata&metadataHeaders=Message-ID&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=Reply-To"), None).await?;
        let mid = header(&m, "Message-ID");
        if !mid.is_empty() {
            extra = format!("In-Reply-To: {mid}\r\nReferences: {mid}\r\n");
        }
        if subject.is_empty() {
            let orig = header(&m, "Subject");
            subject = if orig.to_lowercase().starts_with("re:") { orig } else { format!("Re: {orig}") };
        }
        if to.trim().is_empty() {
            let rt = header(&m, "Reply-To");
            to = if rt.is_empty() { header(&m, "From") } else { rt };
        }
        thread = m["threadId"].as_str().map(str::to_string);
    }
    let clean = |x: &str| x.replace(['\r', '\n'], " ");
    let cc = s(input, "cc");
    let mut raw = format!("To: {}\r\n", clean(&to));
    if !cc.is_empty() {
        raw.push_str(&format!("Cc: {}\r\n", clean(cc)));
    }
    // RFC 2047 for non-ASCII subjects.
    let subj = clean(&subject);
    let subj = if subj.is_ascii() { subj } else { format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(subj.as_bytes())) };
    raw.push_str(&format!("Subject: {subj}\r\n{extra}MIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}", s(input, "body").replace("\r\n", "\n").replace('\n', "\r\n")));
    let mut message = json!({ "raw": URL_SAFE.encode(raw.as_bytes()) });
    if let Some(t) = thread {
        message["threadId"] = json!(t);
    }
    let v = api(app, reqwest::Method::POST, &format!("{GMAIL}/drafts"), Some(json!({ "message": message }))).await?;
    Ok(format!("Saved a draft to {} (\"{subject}\", draft id {}). It's in Gmail → Drafts for them to check and send; don't say it was sent.", clean(&to), v["id"].as_str().unwrap_or("?")))
}

/// "2026-11-05T15:00" (local) or RFC 3339 → RFC 3339 with offset.
fn rfc3339(s: &str) -> Option<String> {
    use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
    let s = s.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.to_rfc3339());
    }
    for f in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(s, f) {
            return Local.from_local_datetime(&n).single().map(|d| d.to_rfc3339());
        }
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().and_then(|d| Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single()).map(|d| d.to_rfc3339())
}

const CAL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

async fn cal_events(app: &AppHandle, input: &Value) -> Result<String, String> {
    let now = chrono::Local::now();
    let from = rfc3339(s(input, "from")).unwrap_or_else(|| now.to_rfc3339());
    let to = rfc3339(s(input, "to")).unwrap_or_else(|| (now + chrono::Duration::days(7)).to_rfc3339());
    let mut url = Url::parse(CAL).unwrap();
    url.query_pairs_mut()
        .append_pair("timeMin", &from)
        .append_pair("timeMax", &to)
        .append_pair("singleEvents", "true")
        .append_pair("orderBy", "startTime")
        .append_pair("maxResults", "50");
    if !s(input, "query").is_empty() {
        url.query_pairs_mut().append_pair("q", s(input, "query"));
    }
    let v = api(app, reqwest::Method::GET, url.as_str(), None).await?;
    let items: Vec<Value> = v["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|e| {
                    json!({
                        "title": e["summary"], "start": e["start"]["dateTime"].as_str().or(e["start"]["date"].as_str()),
                        "end": e["end"]["dateTime"].as_str().or(e["end"]["date"].as_str()), "location": e["location"],
                        "attendees": e["attendees"].as_array().map(|x| x.iter().filter_map(|p| p["email"].as_str()).collect::<Vec<_>>()),
                        "meet": e["hangoutLink"], "notes": e["description"].as_str().map(|d| d.chars().take(300).collect::<String>()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({ "from": from, "to": to, "events": items }).to_string())
}

async fn cal_add(app: &AppHandle, input: &Value) -> Result<String, String> {
    let title = s(input, "title");
    let start = rfc3339(s(input, "start")).ok_or("start must be a date/time like 2026-11-05T15:00")?;
    let end = rfc3339(s(input, "end")).unwrap_or_else(|| {
        chrono::DateTime::parse_from_rfc3339(&start).map(|d| (d + chrono::Duration::hours(1)).to_rfc3339()).unwrap_or(start.clone())
    });
    if title.is_empty() {
        return Err("An event needs a title.".into());
    }
    let mut ev = json!({ "summary": title, "start": { "dateTime": start }, "end": { "dateTime": end } });
    if !s(input, "location").is_empty() {
        ev["location"] = json!(s(input, "location"));
    }
    if !s(input, "notes").is_empty() {
        ev["description"] = json!(s(input, "notes"));
    }
    let v = api(app, reqwest::Method::POST, CAL, Some(ev)).await?;
    Ok(format!("Added \"{title}\" on {start} to their Google Calendar. {}", v["htmlLink"].as_str().unwrap_or("")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_html() {
        assert!(rfc3339("2026-11-05T15:00").unwrap().starts_with("2026-11-05T15:00:00"));
        assert_eq!(rfc3339("2026-11-05T15:00:00Z").unwrap(), "2026-11-05T15:00:00+00:00");
        assert!(rfc3339("nope").is_none());
        assert_eq!(strip_html("<p>Hi&nbsp;<b>there</b></p>"), "Hi there");
        let jwt = format!("x.{}.y", URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c"}"#));
        assert_eq!(jwt_email(&jwt).as_deref(), Some("a@b.c"));
    }
}
