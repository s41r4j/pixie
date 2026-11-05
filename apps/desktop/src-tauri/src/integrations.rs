//! Apps on this Mac: Calendar, Reminders and Contacts (EventKit / Contacts in the Swift helper)
//! and Mail (Mail.app's own scripting). Each is off until switched on in Settings → Apps, which
//! also asks macOS for the permission. Nothing here sends mail: `mail` only drafts, and the draft
//! opens in Mail for the user to send.

use crate::AppState;
use openclicky_llm::ToolDef;
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub const KINDS: [&str; 4] = ["calendar", "reminders", "contacts", "mail"];

fn def(name: &str, description: &str, props: Value, required: &[&str]) -> ToolDef {
    ToolDef { name: name.into(), description: description.into(), input_schema: json!({ "type": "object", "properties": props, "required": required }) }
}

pub fn enabled(app: &AppHandle, kind: &str) -> bool {
    cfg!(target_os = "macos") && app.state::<AppState>().settings.lock().map(|s| s.apps.get(kind).copied().unwrap_or(false)).unwrap_or(false)
}

/// The tools for the apps that are switched on.
pub fn tools(app: &AppHandle) -> Vec<ToolDef> {
    let mut out = vec![];
    if enabled(app, "calendar") {
        out.push(def(
            "calendar",
            "The user's calendars on this Mac (iCloud, Google, Exchange… whatever is in Calendar.app). action=events lists events between from and to (ISO dates/times in local time, default today → 7 days), optionally matching query. action=add creates an event (title, start, end or all_day, location, notes, calendar name); only add when they asked you to.",
            json!({
                "action": { "type": "string", "enum": ["events", "add"] },
                "from": { "type": "string" }, "to": { "type": "string" }, "query": { "type": "string" },
                "title": { "type": "string" }, "start": { "type": "string" }, "end": { "type": "string" },
                "all_day": { "type": "boolean" }, "location": { "type": "string" }, "notes": { "type": "string" },
                "calendar": { "type": "string" }
            }),
            &["action"],
        ));
    }
    if enabled(app, "reminders") {
        out.push(def(
            "reminders",
            "The user's Reminders. action=list (optionally list=<name>, done=true for recently completed), action=add (title, due as ISO local time, list, notes), action=done (id from list).",
            json!({
                "action": { "type": "string", "enum": ["list", "add", "done"] },
                "list": { "type": "string" }, "done": { "type": "boolean" }, "title": { "type": "string" },
                "due": { "type": "string" }, "notes": { "type": "string" }, "id": { "type": "string" }
            }),
            &["action"],
        ));
    }
    if enabled(app, "contacts") {
        out.push(def(
            "contacts",
            "Look someone up in the user's Contacts by name (or email): emails, phones, company, birthday, address.",
            json!({ "query": { "type": "string" } }),
            &["query"],
        ));
    }
    if enabled(app, "mail") {
        out.push(def(
            "mail",
            "Apple Mail on this Mac. action=recent lists the newest inbox messages (count, unread_only); action=search finds inbox messages whose subject or sender contains query; action=read returns one message by id; action=draft opens a new message in Mail with to, subject, body (and cc) filled in for the user to review and send. You never send mail.",
            json!({
                "action": { "type": "string", "enum": ["recent", "search", "read", "draft"] },
                "count": { "type": "integer" }, "unread_only": { "type": "boolean" }, "query": { "type": "string" },
                "id": { "type": "string" }, "to": { "type": "string" }, "cc": { "type": "string" },
                "subject": { "type": "string" }, "body": { "type": "string" }
            }),
            &["action"],
        ));
    }
    out
}

pub fn handles(name: &str) -> bool {
    matches!(name, "calendar" | "reminders" | "contacts" | "mail")
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

pub async fn call(app: &AppHandle, name: &str, input: &Value) -> Result<String, String> {
    if !enabled(app, name) {
        return Err(format!("{name} is switched off; they can turn it on in Settings → Apps."));
    }
    let helper = app.state::<AppState>().helper.clone();
    let t = Duration::from_secs(20);
    let v = match (name, s(input, "action")) {
        ("calendar", "add") => helper.call("calendar_add", input.clone(), t).await?,
        ("calendar", _) => helper.call("calendar_events", input.clone(), t).await?,
        ("reminders", "add") => helper.call("reminder_add", input.clone(), t).await?,
        ("reminders", "done") => helper.call("reminder_done", input.clone(), t).await?,
        ("reminders", _) => helper.call("reminders_list", input.clone(), t).await?,
        ("contacts", _) => helper.call("contacts_find", input.clone(), t).await?,
        ("mail", action) => return mail(action, input).await,
        _ => return Err("unknown app".into()),
    };
    Ok(v.to_string())
}

/// Permission states for Settings → Apps.
pub async fn status(app: &AppHandle) -> Value {
    let st = app.state::<AppState>();
    let pim = st.helper.call("pim_status", json!({}), Duration::from_secs(5)).await.unwrap_or(json!({}));
    let on = st.settings.lock().map(|s| s.apps.clone()).unwrap_or_default();
    json!({
        "supported": cfg!(target_os = "macos"),
        "on": on,
        "calendar": pim["calendar"], "reminders": pim["reminders"], "contacts": pim["contacts"],
        "mail": "ask",
    })
}

/// Switch an app on (asking macOS for access first) or off.
pub async fn set(app: &AppHandle, kind: &str, on: bool) -> Result<String, String> {
    if !KINDS.contains(&kind) {
        return Err("unknown app".into());
    }
    let st = app.state::<AppState>();
    let mut result = "off".to_string();
    if on {
        result = if kind == "mail" {
            // The first Apple Event to Mail shows macOS's "Pixie wants to control Mail" prompt.
            osascript(&["tell application \"Mail\" to return count of accounts"], &[]).await.map(|_| "granted".to_string()).map_err(|e| {
                if e.contains("-1743") { "Pixie isn't allowed to control Mail. Turn it on in System Settings → Privacy & Security → Automation.".to_string() } else { e }
            })?
        } else {
            let v = st.helper.call("pim_request", json!({ "kind": kind }), Duration::from_secs(120)).await?;
            v["status"].as_str().unwrap_or("denied").to_string()
        };
        if result != "granted" {
            let what = match kind {
                "calendar" => "Calendars",
                "reminders" => "Reminders",
                _ => "Contacts",
            };
            return Err(format!("macOS says {result}. Allow Pixie in System Settings → Privacy & Security → {what}."));
        }
    }
    {
        let mut s = st.settings.lock().unwrap();
        s.apps.insert(kind.to_string(), on);
        s.save(&st.data_dir);
    }
    Ok(result)
}

/// Run an AppleScript with arguments passed as `argv` (never spliced into the source).
async fn osascript(lines: &[&str], args: &[&str]) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new("/usr/bin/osascript");
    for l in lines {
        cmd.arg("-e").arg(l);
    }
    cmd.args(args);
    let out = tokio::time::timeout(Duration::from_secs(30), cmd.output())
        .await
        .map_err(|_| "Mail took too long to answer.".to_string())?
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

const LIST_FN: &[&str] = &[
    "on fmt(m)",
    "  tell application \"Mail\"",
    "    return (id of m as string) & tab & ((date received of m) as string) & tab & (sender of m) & tab & (subject of m) & tab & (read status of m as string)",
    "  end tell",
    "end fmt",
];

async fn mail(action: &str, input: &Value) -> Result<String, String> {
    let count = input.get("count").and_then(Value::as_u64).unwrap_or(10).clamp(1, 40).to_string();
    let lines: Vec<&str>;
    let mut args: Vec<String> = vec![];
    match action {
        "recent" => {
            let unread = input.get("unread_only").and_then(Value::as_bool).unwrap_or(false);
            lines = [
                LIST_FN,
                &[
                    "on run argv",
                    "  set n to (item 1 of argv) as integer",
                    "  set out to {}",
                    "  tell application \"Mail\"",
                    if unread { "    set msgs to (messages of inbox whose read status is false)" } else { "    set msgs to messages of inbox" },
                    "    if (count of msgs) < n then set n to (count of msgs)",
                    "    repeat with i from 1 to n",
                    "      set end of out to my fmt(item i of msgs)",
                    "    end repeat",
                    "  end tell",
                    "  set AppleScript's text item delimiters to linefeed",
                    "  return out as string",
                    "end run",
                ],
            ]
            .concat();
            args.push(count);
        }
        "search" => {
            let q = s(input, "query");
            if q.is_empty() {
                return Err("search needs a query".into());
            }
            lines = [
                LIST_FN,
                &[
                    "on run argv",
                    "  set q to item 1 of argv",
                    "  set n to (item 2 of argv) as integer",
                    "  set out to {}",
                    "  tell application \"Mail\"",
                    "    set msgs to (messages of inbox whose subject contains q or sender contains q)",
                    "    if (count of msgs) < n then set n to (count of msgs)",
                    "    repeat with i from 1 to n",
                    "      set end of out to my fmt(item i of msgs)",
                    "    end repeat",
                    "  end tell",
                    "  set AppleScript's text item delimiters to linefeed",
                    "  return out as string",
                    "end run",
                ],
            ]
            .concat();
            args.push(q.to_string());
            args.push(count);
        }
        "read" => {
            let id = s(input, "id");
            if id.parse::<u64>().is_err() {
                return Err("read needs a message id from recent or search".into());
            }
            lines = vec![
                "on run argv",
                "  set AppleScript's text item delimiters to \", \"",
                "  tell application \"Mail\"",
                "    set m to first message of inbox whose id is ((item 1 of argv) as integer)",
                "    return \"From: \" & (sender of m) & linefeed & \"To: \" & (address of to recipients of m as string) & linefeed & \"Date: \" & ((date received of m) as string) & linefeed & \"Subject: \" & (subject of m) & linefeed & linefeed & (content of m)",
                "  end tell",
                "end run",
            ];
            args.push(id.to_string());
        }
        "draft" => {
            lines = vec![
                "on run argv",
                "  tell application \"Mail\"",
                "    set m to make new outgoing message with properties {subject:(item 2 of argv), content:(item 3 of argv), visible:true}",
                "    tell m",
                "      repeat with a in paragraphs of (item 1 of argv)",
                "        if (length of a) > 0 then make new to recipient at end of to recipients with properties {address:a}",
                "      end repeat",
                "      repeat with a in paragraphs of (item 4 of argv)",
                "        if (length of a) > 0 then make new cc recipient at end of cc recipients with properties {address:a}",
                "      end repeat",
                "    end tell",
                "    activate",
                "  end tell",
                "  return \"ok\"",
                "end run",
            ];
            let split = |k: &str| s(input, k).split([',', ';']).map(str::trim).filter(|x| !x.is_empty()).collect::<Vec<_>>().join("\n");
            args.extend([split("to"), s(input, "subject").to_string(), s(input, "body").to_string(), split("cc")]);
        }
        _ => return Err("action must be recent, search, read or draft".into()),
    }
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = osascript(&lines, &argv).await?;
    Ok(match action {
        "draft" => "The draft is open in Mail for them to check and send. Tell them it's ready; don't say it was sent.".into(),
        "read" => out.chars().take(12_000).collect(),
        _ if out.trim().is_empty() => "No messages.".into(),
        _ => format!("id\treceived\tfrom\tsubject\tread\n{out}"),
    })
}
