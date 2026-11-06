//! Tool schemas (BUILD-SPEC §5). Names are part of the prompt contract: change them together.

use openclicky_core::prompt::Mode;
use openclicky_llm::ToolDef;
use serde_json::{json, Value};

fn def(name: &str, description: &str, schema: Value) -> ToolDef {
    ToolDef { name: name.into(), description: description.into(), input_schema: schema }
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

pub fn bash() -> ToolDef {
    let what = format!("Run a shell command ({}) in the job's working folder. Default timeout 300 s; output is cut to 50 KB. Install missing libraries into tmp/.", crate::portable::shell_name());
    def("bash", &what,
        obj(json!({ "command": {"type":"string"}, "timeout": {"type":"integer","description":"seconds"} }), &["command"]))
}
pub fn read() -> ToolDef {
    def("read", "Read a text file from the working folder (or an absolute path). offset is the first line (1-based), limit the number of lines.",
        obj(json!({ "path": {"type":"string"}, "offset": {"type":"integer"}, "limit": {"type":"integer"} }), &["path"]))
}
pub fn write() -> ToolDef {
    def("write", "Create or replace a file inside the working folder (use output/ for deliverables, tmp/ for scratch). To put a file on their Mac use device_save_file.",
        obj(json!({ "path": {"type":"string"}, "content": {"type":"string"} }), &["path", "content"]))
}
pub fn edit() -> ToolDef {
    def("edit", "Edit a file in the working folder by exact replacement. Each old must appear exactly once.",
        obj(json!({ "path": {"type":"string"}, "edits": {"type":"array","items": obj(json!({"old":{"type":"string"},"new":{"type":"string"}}), &["old","new"])} }), &["path", "edits"]))
}
pub fn grep() -> ToolDef {
    def("grep", "Search file contents with an extended regex. path defaults to the working folder; glob filters file names (e.g. *.md).",
        obj(json!({ "pattern": {"type":"string"}, "path": {"type":"string"}, "glob": {"type":"string"} }), &["pattern"]))
}
pub fn find() -> ToolDef {
    def("find", "Find files in the working folder whose name contains the query (or matches a glob like *.pdf).",
        obj(json!({ "query": {"type":"string"} }), &["query"]))
}
pub fn view_image() -> ToolDef {
    def("view_image", "Look at an image file (png, jpg, heic, pdf first page…).", obj(json!({ "path": {"type":"string"} }), &["path"]))
}
pub fn fetch_url() -> ToolDef {
    def("fetch_url", "Fetch a web page and return its readable text. Use offset/max_chars to page through long pages.",
        obj(json!({ "url": {"type":"string"}, "offset": {"type":"integer"}, "max_chars": {"type":"integer"} }), &["url"]))
}
pub fn web_search() -> ToolDef {
    def("web_search", "Search the web. Returns titles, links and snippets; fetch_url the ones worth reading.",
        obj(json!({ "query": {"type":"string"}, "count": {"type":"integer"}, "purpose": {"type":"string"} }), &["query"]))
}
pub fn device_list_folder() -> ToolDef {
    def("device_list_folder", "List a folder on their Mac (e.g. ~/Downloads).", obj(json!({ "path": {"type":"string"} }), &["path"]))
}
pub fn device_find_files() -> ToolDef {
    def("device_find_files", "Find files on their Mac by name (Spotlight). folder narrows the search.",
        obj(json!({ "query": {"type":"string"}, "folder": {"type":"string"} }), &["query"]))
}
pub fn device_read_file() -> ToolDef {
    def("device_read_file", "Read a file from their Mac: text, PDF, Word, RTF and pages come back as text; images come back to look at; anything else is copied into inbox/.",
        obj(json!({ "path": {"type":"string"} }), &["path"]))
}
pub fn device_save_file() -> ToolDef {
    def("device_save_file", "Copy a file from the working folder onto their Mac. Never overwrites: a number is added if the name is taken. Returns the saved path.",
        obj(json!({ "source": {"type":"string"}, "destination": {"type":"string","description":"folder or full path, e.g. ~/Desktop/"} }), &["source", "destination"]))
}
pub fn device_open() -> ToolDef {
    def("device_open", "Open a file, folder, URL or app on their Mac.", obj(json!({ "target": {"type":"string"} }), &["target"]))
}
pub fn update_plan() -> ToolDef {
    def("update_plan", "Set the job's plan: 3 to 5 short steps, exactly one in_progress. This is their progress bar.",
        obj(json!({ "plan": {"type":"array","minItems":1,"maxItems":5,"items": obj(json!({"label":{"type":"string"},"status":{"type":"string","enum":["pending","in_progress","done"]}}), &["label","status"])} }), &["plan"]))
}
pub fn progress() -> ToolDef {
    def("progress", "One short status line on the job card (\"Comparing 6 desks\").", obj(json!({ "note": {"type":"string"} }), &["note"]))
}
pub fn ask_user() -> ToolDef {
    def("ask_user", "Ask them something only they can answer and wait for the reply. Bundle everything into one question. choices are tap-to-answer options.",
        obj(json!({ "question": {"type":"string"}, "choices": {"type":"array","items":{"type":"string"}} }), &["question"]))
}
pub fn note() -> ToolDef {
    def("note", "Replace the whole note with this markdown.", obj(json!({ "markdown": {"type":"string"} }), &["markdown"]))
}
pub fn note_add() -> ToolDef {
    def("note_add", "Append lines to the end of the note (they appear as you write).",
        obj(json!({ "lines": {"anyOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}]} }), &["lines"]))
}
pub fn note_answer() -> ToolDef {
    def("note_answer", "Set the answer line at the top of the first page, e.g. 'Buy the Jarvis: steadiest under $700. {big=\"$599\"}'.",
        obj(json!({ "line": {"type":"string"} }), &["line"]))
}
pub fn note_replace() -> ToolDef {
    def("note_replace", "Replace exact text in the note.", obj(json!({ "old": {"type":"string"}, "new": {"type":"string"} }), &["old", "new"]))
}
pub fn note_remove() -> ToolDef {
    def("note_remove", "Remove a line from the note.", obj(json!({ "line": {"type":"string"}, "why": {"type":"string"} }), &["line"]))
}
pub fn note_tick() -> ToolDef {
    def("note_tick", "Tick the '- [ ]' box whose text starts with label; current=true marks it as the one you're on now instead.",
        obj(json!({ "label": {"type":"string"}, "current": {"type":"boolean"} }), &["label"]))
}
pub fn say() -> ToolDef {
    def("say", "Say one short line out loud to them.", obj(json!({ "text": {"type":"string"} }), &["text"]))
}
pub fn draw() -> ToolDef {
    let op = obj(json!({
        "kind": {"type":"string","enum":["circle","underline","arrow","box","check","cross","text","line"]},
        "target": {"type":"string","description":"on screen: element id (e12) or exact on-screen text to circle/box/underline; its exact frame is used"},
        "x": {"type":"number"}, "y": {"type":"number"}, "w": {"type":"number"}, "h": {"type":"number"},
        "to_x": {"type":"number","description":"arrow/line end x"}, "to_y": {"type":"number"},
        "text": {"type":"string","description":"handwritten text (kind text), or a label inside a box/circle"},
        "color": {"type":"string","enum":["ink","blue","red","green","orange","yellow","pink"]}
    }), &["kind"]);
    def("draw", "Draw by hand. On their screen (default): circle, box or underline something that's on screen (x,y,w,h in screen points, use the frame from the lists), an arrow from (x,y) to (to_x,to_y), a check or cross, or short text; marks fade after the answer. This marks what's on their screen; to explain something with a diagram, use the canvas tool instead.",
        obj(json!({ "ops": {"type":"array","items": op} }), &["ops"]))
}
pub fn highlight() -> ToolDef {
    def("highlight", "Point at something on screen: the pointer flies there and rings it. Best: give target, the element id from the controls list (e.g. e12) or the exact text of a line from the text list; its exact frame is used. Otherwise give x, y, w, h in screen points copied from those lists (never estimate from the image when the item is listed). label is a 1-4 word tag shown beside it.",
        obj(json!({ "target": {"type":"string","description":"element id like e12, or exact on-screen text"}, "x": {"type":"number"}, "y": {"type":"number"}, "w": {"type":"number"}, "h": {"type":"number"}, "label": {"type":"string"} }), &[]))
}
pub fn mac_step() -> ToolDef {
    def("mac_step", "Do one step in their apps. Your buddy pointer flies there first so they see it. action: click / double_click / right_click (target = a label, an element id like e12, or give x and y in screen points), drag (x,y → to_x,to_y), press (a button by label/id through Accessibility), menu (a path like \"File > Export…\"), type (target = the text; it goes into the field with keyboard focus. Give x and y of the field to click into it first: always do that unless you just clicked it, and always when the window has several panes or inputs such as split terminals, chat side panels or editor + console; the result says which field got it), key (a shortcut like cmd+s or return), focus (a field by label), open (an app, file or URL), scroll (up/down), find (locate without acting). Each click returns what the screen looks like after; check it before the next step.",
        obj(json!({
            "action": {"type":"string","enum":["click","double_click","right_click","drag","press","menu","type","key","focus","open","scroll","find"]},
            "target": {"type":"string"},
            "x": {"type":"number"}, "y": {"type":"number"},
            "to_x": {"type":"number"}, "to_y": {"type":"number"}
        }), &["action"]))
}
pub fn clipboard() -> ToolDef {
    def("clipboard", "Their clipboard. action: copy (put text on it so they can paste it), read (what they last copied).",
        obj(json!({ "action": {"type":"string","enum":["copy","read"]}, "text": {"type":"string"} }), &["action"]))
}
pub fn look() -> ToolDef {
    def("look", "See their screen now: a screenshot plus the visible controls and text with exact frames (screen points) to use with mac_step, highlight and draw.",
        obj(json!({}), &[]))
}
pub fn code() -> ToolDef {
    // Not in §5's table: delegation to the user's local coding agents. TODO: fold into the spec.
    def("code", "Hand a coding task to a local coding agent on their Mac (Codex CLI, Claude Code or opencode, whichever is installed; see the environment). It edits files in dir (a repo on their Mac, or output/ by default) and returns its summary. Write the task as a complete brief: goal, files, constraints, how to check. Use it for real code changes, apps, scripts and fixes; review its summary and check the result.",
        obj(json!({ "task": {"type":"string"}, "dir": {"type":"string"}, "agent": {"type":"string","enum":["auto","codex","claude","opencode"]}, "timeout_minutes": {"type":"integer"} }), &["task"]))
}

pub fn start_job() -> ToolDef {
    // Not in §5's list: the voice loop needs a way to hand work to a background job. TODO: fold into
    // the spec's tool table.
    def("start_job", "Hand longer work to an agent. kind=background (default): works on its own in the background (research, documents, files, code) with a plan and a note. kind=live: does it on their real screen while explaining each step out loud (clicking, typing and copying in their apps; use it when they want you to do something in their apps that takes several steps, or to show them how). Both show in the notch's Agents tab. Write the brief as a complete instruction with every detail they gave.",
        obj(json!({ "title": {"type":"string","description":"2-4 words"}, "instructions": {"type":"string"}, "kind": {"type":"string","enum":["background","live"]} }), &["title", "instructions"]))
}
pub fn browser() -> ToolDef {
    def("browser", "Use their Chrome through the Pixie extension. operation: tabs, open {url}, snapshot {tabId?} (numbered elements + text), click {ref}, type {ref, text, submit?}, scroll {direction}, navigate {url}, back, screenshot, batch {steps:[...]} (up to 20 operations). Re-snapshot after actions that change the page.",
        json!({ "type":"object", "properties": { "operation": {"type":"string"}, "tabId": {"type":"integer"} }, "required": ["operation"], "additionalProperties": true }))
}

/// Tools offered per mode. Host-provided extras (connectors) are appended by the caller.
pub fn for_mode(mode: Mode, browser_connected: bool) -> Vec<ToolDef> {
    let mut v = match mode {
        Mode::Voice | Mode::Takeover => vec![
            highlight(), draw(), mac_step(), clipboard(), note(), note_answer(), start_job(), web_search(), fetch_url(),
            device_open(), device_find_files(), device_list_folder(), device_read_file(),
        ],
        Mode::Live => vec![
            look(), mac_step(), highlight(), draw(), say(), clipboard(), update_plan(), progress(), ask_user(),
            web_search(), fetch_url(), device_open(), device_find_files(), device_read_file(), note(), note_answer(),
        ],
        Mode::Tour => vec![say(), highlight()],
        Mode::Interview => vec![note_answer()],
        Mode::Job => vec![
            bash(), read(), write(), edit(), grep(), find(), view_image(), fetch_url(), web_search(),
            device_list_folder(), device_find_files(), device_read_file(), device_save_file(), device_open(),
            update_plan(), progress(), ask_user(), code(), clipboard(),
            note(), note_add(), note_answer(), note_replace(), note_remove(), note_tick(), say(),
        ],
    };
    if browser_connected && matches!(mode, Mode::Voice | Mode::Takeover | Mode::Job | Mode::Live) {
        v.push(browser());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_tool_names_exist() {
        let all: Vec<String> = [for_mode(Mode::Job, true), for_mode(Mode::Voice, true)]
            .concat()
            .into_iter()
            .map(|t| t.name)
            .collect();
        for name in [
            "bash", "read", "write", "edit", "grep", "find", "view_image", "fetch_url", "web_search",
            "device_list_folder", "device_find_files", "device_read_file", "device_save_file", "device_open",
            "update_plan", "progress", "ask_user", "note", "note_add", "note_answer", "note_replace",
            "note_remove", "note_tick", "say", "draw", "highlight", "mac_step", "browser",
        ] {
            assert!(all.iter().any(|n| n == name), "missing tool {name}");
        }
    }

    #[test]
    fn schemas_are_objects_with_unique_names() {
        let job = for_mode(Mode::Job, true);
        let mut names: Vec<_> = job.iter().map(|t| t.name.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), job.len());
        assert!(job.iter().all(|t| t.input_schema["type"] == "object"));
    }
}
