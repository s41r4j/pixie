// Browser preview of every view with mocked IPC, for design work outside the app:
//   npm run dev -w apps/desktop, then open http://localhost:1420/?preview=panel&mode=answer
// Views: panel (modes: listening, thinking, answer, idle, toast, dictating), overlay, shelf,
// settings (tab=welcome|setup|ai|voice|sounds|connectors|about), note.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { Job, NoteDoc } from "./lib/bridge";

const params = new URLSearchParams(location.search);
export const previewView = params.get("preview");

const NOTE: NoteDoc = {
  title: "Desk pick",
  pages: [
    {
      name: "The pick",
      blocks: [
        { type: "answer", text: "Buy the Fully Jarvis: steadiest under $700.", attrs: { big: "$599", note: "on sale till Friday" } },
        {
          type: "container", name: "tiles", label: "", attrs: {}, lines: [],
          children: [
            { type: "object", name: "figure", label: "Price", attrs: { value: "$599", delta: "-8%", good: "down" } },
            { type: "object", name: "status", label: "Stock", attrs: { state: "ok", note: "ships Friday" } },
            { type: "object", name: "countdown", label: "Sale ends", attrs: { to: "2026-11-12 23:59" } },
            { type: "object", name: "spark", label: "Price trend", attrs: { values: "649, 649, 629, 599", unit: "" } },
          ],
        },
        { type: "object", name: "sticky", label: "Measure your desk depth first", attrs: { color: "yellow" } },
        {
          type: "list", ordered: false,
          items: [
            { text: "Checked 14 reviews", checked: true, marks: ["new"], detail: ["Wirecutter and two forums"] },
            { text: "Compare warranty terms", checked: false, marks: ["now"], detail: [] },
            { text: "Order before Friday", checked: false, marks: [], detail: [] },
          ],
        },
        { type: "object", name: "question", label: "Do you want the 48\" top?", attrs: { yes: "Get the 48 inch top", no: "Keep the 60 inch", yes_label: "48\"", no_label: "60\"" } },
        { type: "object", name: "ask", label: "Find it cheaper", attrs: { say: "Find an equally steady desk for less" } },
      ],
    },
    {
      name: "Compared",
      blocks: [
        { type: "table", header: ["Desk", "Price", "Wobble"], rows: [["Jarvis", "$599", "low"], ["Uplift V2", "$649", "lowest"], ["Flexispot E7", "$499", "medium"]] },
        { type: "container", name: "chart", label: "Reviewer scores", attrs: { unit: "/10" }, lines: ["Jarvis: 9.1", "Uplift V2: 9.3", "Flexispot E7: 8.2"], children: [] },
        { type: "object", name: "link", label: "Wirecutter: standing desks", attrs: { url: "https://www.nytimes.com/wirecutter/reviews/best-standing-desk/" } },
      ],
    },
  ],
};

const now = Date.now();
const JOBS: Job[] = [
  {
    id: "a", title: "Desk Pick", state: "done", instructions: "", cwd: "/tmp/a", created_at: now - 60_000, updated_at: now, steps: 9,
    plan: [{ label: "Read reviews", status: "done" }, { label: "Compare", status: "done" }, { label: "Write it up", status: "done" }],
    progress: "", question: null, error: null, unread: true,
    report: { body: "The Jarvis is the steadiest desk under $700, on sale for $599 until Friday.", done_title: "Desk Pick", next_actions: ["Find it cheaper", "Make a one-page PDF"], artifacts: ["/tmp/a/output/desks.pdf"], learned: [], choices: [], draft: null },
  },
  {
    id: "b", title: "Tea one-pager", state: "running", instructions: "", cwd: "/tmp/b", created_at: now - 30_000, updated_at: now, steps: 4,
    plan: [{ label: "Research", status: "done" }, { label: "Write", status: "in_progress" }, { label: "Make PDF", status: "pending" }, { label: "Check", status: "pending" }],
    progress: "Reading teaclass.com", question: null, error: null, unread: true, report: null,
  },
  {
    id: "c", title: "Inbox sweep", state: "waiting_on_user", instructions: "", cwd: "/tmp/c", created_at: now - 10_000, updated_at: now, steps: 3,
    plan: [{ label: "Read inbox", status: "done" }, { label: "Draft replies", status: "in_progress" }, { label: "Send", status: "pending" }],
    progress: "", question: { question: "Should I reply to the landlord as you or as the household?", choices: ["As me", "As the household"] }, error: null, unread: true, report: null, kind: "background",
  },
  {
    id: "d", title: "Rename slides", state: "running", instructions: "", cwd: "/tmp/d", created_at: now - 5_000, updated_at: now, steps: 2,
    plan: [{ label: "Open Keynote", status: "done" }, { label: "Rename each slide", status: "in_progress" }, { label: "Check", status: "pending" }],
    progress: "Clicking slide 3", question: null, error: null, unread: true, report: null, kind: "live",
  },
];

export function installPreview() {
  if (!previewView) return;
  mockWindows(previewView);
  const mode = params.get("mode") ?? "answer";
  mockIPC(
    (cmd) => {
      switch (cmd) {
        case "panel_mode": return mode;
        case "panel_screen": return { frame: { x: 0, y: 0, w: 1710, h: 1107 }, top_inset: 34, notch_width: 185, scale: 2 };
        case "theme": return null;
        case "jobs_list": return JOBS;
        case "chats_list": return [
          { id: "c3", title: "What's a good name for this deck?", started: now - 300_000, updated: now - 30_000, count: 3, preview: "Try “Tea, Briefly”." },
          { id: "c2", title: "How do I export this as a PDF?", started: now - 86_400_000, updated: now - 86_000_000, count: 2, preview: "File, then Export To, then PDF." },
          { id: "c1", title: "Summarise this page", started: now - 3 * 86_400_000, updated: now - 3 * 86_000_000, count: 4, preview: "It's a review of standing desks." },
        ];
        case "chat_get": return { id: "c3", title: "What's a good name for this deck?", started: now - 300_000, updated: now - 30_000, entries: [
          { q: "Where's the share button in Keynote?", a: "Top right of the toolbar, the square with the arrow. I circled it.", at: now - 300_000 },
          { q: "Can you copy the slide titles for me?", a: "Done, all nine titles are on your clipboard.", at: now - 120_000 },
          { q: "What's a good name for this deck?", a: "Try “Tea, Briefly”. Short and it says what it is.", at: now - 30_000 },
        ] };
        case "chat_log": return [
          { q: "Where's the share button in Keynote?", a: "Top right of the toolbar, the square with the arrow. I circled it.", at: now - 300_000 },
          { q: "Can you copy the slide titles for me?", a: "Done, all nine titles are on your clipboard.", at: now - 120_000 },
          { q: "What's a good name for this deck?", a: "Try “Tea, Briefly”. Short and it says what it is.", at: now - 30_000 },
        ];
        case "job_get": return JOBS[0];
        case "job_note": return NOTE;
        case "session_note": return null;
        case "settings_get": return { settings: { deepseek_key: "sk-1••••abcd", anthropic_key: "sk-a••••wxyz", provider: "claude", voice_id: "", speech_rate: 0.52, voice_engine: "kokoro", kokoro_voice: 3, voice_speed: 1, openai_key: "", models: { claude: { fast: "claude-haiku-5-5", deep: "claude-opus-5-5" } }, prices: {}, coding: { agent: "codex", models: { codex: "" }, codex_sandbox: "workspace-write", claude_permission: "acceptEdits", dir: "" }, buddy: true, buddy_mode: "idle", can_click: true, fallback: "deepseek:deepseek-flash", shortcut_talk: "ctrl+option", shortcut_dictate: "fn+ctrl", sounds: { "agent-done": { volume: 0.6, muted: false, custom: "/Users/me/Library/Application Support/com.openclicky.app/custom-sounds/agent-done.wav" }, "whoosh": { volume: 1, muted: true, custom: "" } }, speak_answers: true, share_screen: true, master_volume: 1, muted: false, onboarded: params.get("tab") !== "welcome", user_name: "", locale: "", apps: { calendar: true, reminders: true }, google_client_id: "1234-abc.apps.googleusercontent.com", google_client_secret: "GOCS••••xyz" }, provider: "Claude · claude-haiku-5-5", key_hint: "sk-a••••wxyz", active: "claude", effective: { claude: { fast: "claude-haiku-5-5", deep: "claude-opus-5-5" }, openai: { fast: "gpt-5-mini", deep: "gpt-5" }, deepseek: { fast: "deepseek-flash", deep: "deepseek-flash" } } };
        case "permissions_status": if (previewView === "guide") return []; return [
          { id: "microphone", label: "Microphone", why: "Hear you while you hold ⌃⌥.", required: true, state: "granted", restart_hint: false },
          { id: "screen", label: "Screen Recording", why: "See what you're looking at when you ask about it.", required: true, state: "not_granted", restart_hint: true },
          { id: "accessibility", label: "Accessibility", why: "Point at buttons and type into fields for you.", required: true, state: "not_granted", restart_hint: false },
          { id: "speech", label: "Speech Recognition", why: "Turn your voice into text, on this Mac.", required: true, state: "granted", restart_hint: false },
          { id: "files", label: "Desktop, Documents & Downloads", why: "Find and save files where you keep them.", required: false, state: "unchecked", restart_hint: false },
        ];
        case "voices": return { voices: [{ id: "ava", name: "Ava", language: "en-US", quality: "premium" }, { id: "sam", name: "Samantha", language: "en-US", quality: "default" }], default: "ava" };
        case "status": return { helper: true, provider: "DeepSeek · deepseek-flash", browser: false, connectors: [], data_dir: "/Users/me/Library/Application Support/com.openclicky.app", version: "0.2.0" };
        case "models_available": return { prices_at: now - 3_600_000, models: [
          { id: "claude-opus-5-5", name: "Claude Opus 5.5", vision: true, created: now - 9e8, price: { input: 4, output: 20 }, context: 1000000 },
          { id: "claude-sonnet-5-5", name: "Claude Sonnet 5.5", vision: true, created: now - 9.5e8, price: { input: 2, output: 10 }, context: 1000000 },
          { id: "claude-haiku-5-5", name: "Claude Haiku 5.5", vision: true, created: now - 1e9, price: { input: 0.1, output: 0.5 }, context: 1000000 },
          { id: "claude-fable-5-1", name: "Claude Fable 5.1", vision: true, created: now - 2e9, price: { input: 10, output: 50 }, context: 1000000 },
          { id: "claude-opus-5", name: "Claude Opus 5", vision: true, created: now - 5e9, price: { input: 5, output: 25 }, context: 1000000 },
        ] };
        case "prices_refresh": return { models: 120, fetched_at: now };
        case "usage_summary": return { days: 30, requests: 128, input: 1840000, output: 96000, cost: 2.41, unpriced: true, models: [
          { model: "claude-opus-5-5", provider: "claude", requests: 40, input: 520000, output: 16000, voice: 30, job: 10, cost: 2.4, price: { input: 4, output: 20 } },
          { model: "deepseek-flash", provider: "deepseek", requests: 88, input: 1320000, output: 80000, voice: 80, job: 8, cost: 0.1, price: { input: 0.045, output: 0.5 } } ],
          prices_at: now - 3_600_000, providers: ["claude", "deepseek"], all_time: { requests: 412, tokens: 6_420_000, cost: 7.83, since: now - 12 * 86_400_000 },
          by_day: [{ day: new Date().toISOString().slice(0, 10), tokens: 198600, cost: 0.04 }, { day: new Date(Date.now() - 3 * 864e5).toISOString().slice(0, 10), tokens: 52000, cost: 0.01 }] };
        case "coders_status": return { agents: [
          { id: "codex", label: "Codex CLI", installed: true, path: "/opt/homebrew/bin/codex", version: "codex-cli 0.166.0" },
          { id: "claude", label: "Claude Code", installed: true, path: "/opt/homebrew/bin/claude", version: "2.1.318 (Claude Code)" },
          { id: "opencode", label: "opencode", installed: true, path: "/opt/homebrew/bin/opencode", version: "v2.2.5" } ], t3code: true };
        case "voice_status": return { installed: params.get("voice") !== "dl", ready: true, installing: params.get("voice") === "dl", done: 128_000_000, total: 372_000_000, voices: [{ id: 3, name: "Heart", description: "warm, American (default)" }, { id: 2, name: "Bella", description: "bright, American" }, { id: 16, name: "Michael", description: "relaxed, American" }, { id: 21, name: "Emma", description: "clear, British" }] };
        case "apps_status": return { supported: true, on: { calendar: true, reminders: true, contacts: false, mail: false }, calendar: "granted", reminders: "granted", contacts: "not_determined", mail: "ask", google: { configured: true, signed_in: params.get("google") === "on", email: "you@gmail.com" } };
        case "connectors_gallery": return { node: true, path: "/Users/me/Library/Application Support/com.openclicky.app/mcp.json", custom: [], presets: [
          { id: "github", name: "GitHub", blurb: "Issues, pull requests, code search and files in your repos.", help: "Create a fine-grained token.", help_url: "https://github.com", fields: [{ key: "GITHUB_PERSONAL_ACCESS_TOKEN", label: "Personal access token", placeholder: "github_pat_…" }], folder: false, added: true, on: true, tools: 26, error: null, env: { GITHUB_PERSONAL_ACCESS_TOKEN: "gith••••x9Q" }, args: [] },
          { id: "notion", name: "Notion", blurb: "Search, read and add pages in the workspaces you share with it.", fields: [{ key: "NOTION_TOKEN", label: "Integration secret", placeholder: "ntn_…" }], folder: false, added: false, on: false, tools: null, error: null, env: null, args: null },
          { id: "slack", name: "Slack", blurb: "Read channels and threads, post when you ask.", fields: [{ key: "SLACK_BOT_TOKEN", label: "Bot token", placeholder: "xoxb-…" }], folder: false, added: false, on: false, tools: null, error: null, env: null, args: null },
          { id: "linear", name: "Linear", blurb: "Find, create and update issues and projects.", fields: [], folder: false, added: true, on: true, tools: null, error: null, env: {}, args: [] },
          { id: "atlassian", name: "Jira & Confluence", blurb: "Search and update Jira issues and Confluence pages.", fields: [], folder: false, added: false, on: false, tools: null, error: null, env: null, args: null },
          { id: "brave", name: "Brave Search", blurb: "Private web and news search with fresh results.", fields: [{ key: "BRAVE_API_KEY", label: "API key", placeholder: "BSA…" }], folder: false, added: true, on: false, tools: null, error: null, env: { BRAVE_API_KEY: "BSA1••••abc" }, args: [] },
          { id: "files", name: "A folder", blurb: "Read and write files in one folder you choose.", fields: [], folder: true, added: false, on: false, tools: null, error: null, env: null, args: null },
        ] };
        case "resources_report": return { root: "", pack: "local", sounds: 37, missing: [] };
        default: return null;
      }
    },
    { shouldMockEvents: true },
  );

  // Script a scene per view.
  const ev = (kind: string, payload: unknown, delay: number, job_id: string | null = null) =>
    setTimeout(() => emit("openclicky://event", { kind, job_id, payload }), delay);
  if (previewView === "panel") {
    if (mode === "listening" || mode === "dictating") {
      ev("mic.partial", { text: "where's the share button in this" }, 200);
      let t = 0;
      const tick = () => {
        emit("openclicky://event", { kind: "mic.level", job_id: null, payload: { level: 0.3 + 0.6 * Math.abs(Math.sin(t++ / 3)) } });
      };
      setInterval(tick, 60);
    }
    if (mode === "thinking" || mode === "answer") {
      ev("turn.start", { turn: 0, text: "Where's the share button in Keynote?" }, 100);
      ev("turn.tool", { turn: 0, name: "highlight", input: { label: "Share" } }, 300);
    }
    if (mode === "answer") {
      ev("turn.tool_done", { turn: 0, name: "highlight", ok: true, summary: "" }, 500);
      ev("speech.segment", { id: "s1", text: "It's the square with the arrow, top right of the toolbar.", turn: 0 }, 600);
      ev("speech.segment", { id: "s2", text: "I've circled it for you.", turn: 0 }, 700);
      ev("speech.word", { id: "s1", at: 10, len: 6 }, 900);
      if (params.get("note")) ev("session.note", { doc: { title: "", pages: [{ name: "", blocks: [{ type: "answer", text: "Share → Export To → PDF…", attrs: { big: "⌘⇧E" } }] }] } }, 800);
    }
    if (mode === "toast") ev("toast", { text: "Desk Pick is ready", kind: "done", job: "a" }, 100);
  }
  if (previewView === "overlay") {
    ev("overlay.screen", { screen: { frame: { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight }, top_inset: 34, notch_width: 185, scale: 2 }, window: { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight }, origin: { x: 0, y: 0, w: window.innerWidth, h: 900 } }, 50);
    ev("buddy.cursor", { x: 300, y: 200 }, 100);
    ev("speech.start", { id: "b1" }, 200);
    ev("overlay.highlight", { rect: { x: 560, y: 120, w: 150, h: 36 } }, 300);
    ev("overlay.draw", { ops: [{ kind: "circle", x: 140, y: 330, w: 200, h: 48 }, { kind: "underline", x: 520, y: 420, w: 260, h: 20 }, { kind: "arrow", x: 860, y: 480, to_x: 720, to_y: 180 }] }, 500);
  }
  if (previewView === "board") {
    ev("board.show", { title: "How a request reaches the server", ops: [
      { kind: "box", x: 60, y: 240, w: 140, h: 60, text: "Your web browser on the laptop" },
      { kind: "arrow", x: 250, y: 280, to_x: 400, to_y: 280, text: "HTTPS" },
      { kind: "box", x: 410, y: 240, w: 180, h: 80, text: "Load balancer", color: "blue" },
      { kind: "arrow", x: 600, y: 280, to_x: 750, to_y: 200 },
      { kind: "arrow", x: 600, y: 280, to_x: 750, to_y: 360 },
      { kind: "box", x: 760, y: 150, w: 180, h: 80, text: "Server A" },
      { kind: "box", x: 760, y: 320, w: 180, h: 80, text: "Server B" },
      { kind: "text", x: 380, y: 300, text: "the load balancer picks whichever server is less busy right now", color: "red" },
      { kind: "underline", x: 410, y: 300, w: 180, h: 20, color: "red" },
    ] }, 200);
  }
  if (previewView === "guide") ev("guide.show", { perm: "accessibility", app: "/Applications/Pixie.app", icon: "" }, 150);
  if (previewView === "settings" && params.get("tab")) ev("settings.tab", { tab: params.get("tab") }, 300);
  if (previewView === "note") location.hash = params.get("side") === "chats" ? "job=chats:c3" : "job=a";
  document.documentElement.classList.add("preview");
  if (params.get("static")) document.documentElement.classList.add("static");
}
