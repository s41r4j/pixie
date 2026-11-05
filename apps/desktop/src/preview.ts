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
    progress: "", question: { question: "Should I reply to the landlord as you or as the household?", choices: ["As me", "As the household"] }, error: null, unread: true, report: null,
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
        case "job_get": return JOBS[0];
        case "job_note": return NOTE;
        case "session_note": return null;
        case "settings_get": return { settings: { deepseek_key: "sk-1••••abcd", anthropic_key: "", provider: "", voice_id: "", speech_rate: 0.52, voice_engine: "kokoro", kokoro_voice: 3, voice_speed: 1, speak_answers: true, share_screen: true, master_volume: 1, muted: false, onboarded: params.get("tab") !== "welcome", user_name: "", locale: "" }, provider: "DeepSeek · deepseek-flash", key_hint: "sk-1••••abcd" };
        case "permissions_status": return [
          { id: "microphone", label: "Microphone", why: "Hear you while you hold ⌃⌥.", required: true, state: "granted", restart_hint: false },
          { id: "screen", label: "Screen Recording", why: "See what you're looking at when you ask about it.", required: true, state: "not_granted", restart_hint: true },
          { id: "accessibility", label: "Accessibility", why: "Point at buttons and type into fields for you.", required: true, state: "not_granted", restart_hint: false },
          { id: "speech", label: "Speech Recognition", why: "Turn your voice into text, on this Mac.", required: true, state: "granted", restart_hint: false },
          { id: "files", label: "Desktop, Documents & Downloads", why: "Find and save files where you keep them.", required: false, state: "unchecked", restart_hint: false },
        ];
        case "voices": return { voices: [{ id: "ava", name: "Ava", language: "en-US", quality: "premium" }, { id: "sam", name: "Samantha", language: "en-US", quality: "default" }], default: "ava" };
        case "status": return { helper: true, provider: "DeepSeek · deepseek-flash", browser: false, connectors: [], data_dir: "/Users/me/Library/Application Support/com.openclicky.app", version: "0.2.0" };
        case "voice_status": return { installed: true, ready: true, installing: false, voices: [{ id: 3, name: "Heart", description: "warm, American (default)" }, { id: 2, name: "Bella", description: "bright, American" }, { id: 16, name: "Michael", description: "relaxed, American" }, { id: 21, name: "Emma", description: "clear, British" }] };
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
    ev("overlay.screen", { screen: { frame: { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight }, top_inset: 34, notch_width: 185, scale: 2 }, origin: { x: 0, y: 0, w: window.innerWidth, h: 900 } }, 50);
    ev("overlay.highlight", { rect: { x: 520, y: 260, w: 120, h: 34 }, label: "Share" }, 300);
    ev("overlay.draw", { ops: [{ kind: "circle", x: 200, y: 420, w: 180, h: 50, color: "coral" }, { kind: "underline", x: 520, y: 500, w: 220, h: 20, color: "sky" }, { kind: "arrow", x: 820, y: 600, to_x: 660, to_y: 300, color: "butter" }, { kind: "text", x: 840, y: 640, text: "this one!", color: "coral" }] }, 900);
  }
  if (previewView === "settings" && params.get("tab")) ev("settings.tab", { tab: params.get("tab") }, 300);
  if (previewView === "note") location.hash = "job=a";
  document.documentElement.classList.add("preview");
  if (params.get("static")) document.documentElement.classList.add("static");
}
