// Typed bridge to the Rust side. Media is addressed by slot name only (resources/manifest.json).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const EVENT_CHANNEL = "openclicky://event";

export type PanelMode =
  | "hidden"
  | "listening"
  | "dictating"
  | "thinking"
  | "answer"
  | "idle"
  | "typing"
  | "toast"
  | "peek";

export interface CoreEvent<P = any> {
  kind: string;
  job_id: string | null;
  payload: P;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ScreenInfo {
  frame: Rect;
  top_inset: number;
  notch_width: number | null;
  scale: number;
}

export type PermState = "granted" | "denied" | "restricted" | "not_determined" | "not_granted" | "unchecked";

export interface PermissionInfo {
  id: string;
  label: string;
  why: string;
  required: boolean;
  state: PermState;
  restart_hint: boolean;
}

export interface ResourcesReport {
  root: string;
  pack: "env" | "local" | "bundled" | "repo";
  sounds: number;
  missing: { kind: string; slot: string; file: string }[];
}

/** `resources/theme.json`: groups of name → CSS value. */
export type Theme = { version: number } & Record<string, Record<string, string> | number>;

// ---- note AST (crates/core/src/note.rs) ----
export type Attrs = Record<string, string>;
export interface ListItem {
  text: string;
  checked: boolean | null;
  marks: string[];
  detail: string[];
}
export type Block =
  | { type: "answer"; text: string; attrs: Attrs }
  | { type: "quote"; text: string }
  | { type: "callout"; kind: string; text: string }
  | { type: "heading"; level: number; text: string }
  | { type: "object"; name: string; label: string; attrs: Attrs }
  | { type: "container"; name: string; label: string; attrs: Attrs; lines: string[]; children: Block[] }
  | { type: "list"; ordered: boolean; items: ListItem[] }
  | { type: "table"; header: string[]; rows: string[][] }
  | { type: "code"; lang: string; text: string }
  | { type: "paragraph"; text: string }
  | { type: "rule" };
export interface NoteDoc {
  title: string;
  pages: { name: string; blocks: Block[] }[];
}

// ---- jobs ----
export type JobState = "offered" | "waiting_on_user" | "running" | "done" | "failed";
export interface PlanStep {
  label: string;
  status: "pending" | "in_progress" | "done";
}
export interface Report {
  body: string;
  done_title: string | null;
  next_actions: string[];
  artifacts: string[];
  learned: string[];
  choices: string[];
  draft: string | null;
}
export interface ChatEntry {
  q: string;
  a: string;
  at: number;
}

export interface ChatSummary {
  id: string;
  title: string;
  started: number;
  updated: number;
  count: number;
  preview: string;
}

export interface Job {
  id: string;
  title: string;
  state: JobState;
  instructions: string;
  cwd: string;
  created_at: number;
  updated_at: number;
  plan: PlanStep[];
  progress: string;
  question: { question: string; choices: string[] } | null;
  report: Report | null;
  error: string | null;
  unread: boolean;
  steps: number;
  /** "background" works on its own; "live" works on the screen while explaining. */
  kind?: string;
}

export interface ModelChoice {
  fast: string;
  deep: string;
}
export interface Price {
  input: number;
  output: number;
}
export interface CodingPrefs {
  agent: string;
  models: Record<string, string>;
  codex_sandbox: string;
  claude_permission: string;
  dir: string;
}
export interface UsageModel {
  model: string;
  provider: string;
  requests: number;
  input: number;
  output: number;
  voice: number;
  job: number;
  cost: number | null;
  price: Price | null;
}
export interface UsageSummary {
  days: number;
  requests: number;
  input: number;
  output: number;
  cost: number;
  unpriced: boolean;
  models: UsageModel[];
  by_day: { day: string; tokens: number; cost: number }[];
  prices_at?: number;
  all_time?: { requests: number; tokens: number; cost: number; since: number | null };
  providers?: string[];
}
export interface MemoryNote {
  id: string;
  text: string;
  kind: string;
  source: string;
  created: number;
  updated: number;
  uses: number;
  last_used: number;
}

export interface Settings {
  deepseek_key: string;
  anthropic_key: string;
  openai_key: string;
  models: Record<string, ModelChoice>;
  prices: Record<string, Price>;
  coding: CodingPrefs;
  provider: string;
  voice_id: string;
  speech_rate: number;
  voice_engine: string;
  kokoro_voice: number;
  voice_speed: number;
  speak_answers: boolean;
  share_screen: boolean;
  master_volume: number;
  muted: boolean;
  onboarded: boolean;
  buddy: boolean;
  buddy_mode: "always" | "idle" | "off";
  fallback: string;
  shortcut_talk: string;
  shortcut_dictate: string;
  sounds: Record<string, { volume: number; muted: boolean; custom: string }>;
  can_click: boolean;
  user_name: string;
  locale: string;
  apps: Record<string, boolean>;
  auto_update: boolean;
  experimental: Record<string, boolean>;
  /** Long-term memory: "auto" (also learns from chats), "manual" or "off". */
  memory: "auto" | "manual" | "off";
  /** Hour (0–23) when today's chat ends and a new one starts. */
  chat_reset_hour: number;
  /** Personality: pix (default), inos, xtrem, eon. */
  persona: "pix" | "inos" | "xtrem" | "eon";
  /** Xtrem may swear a little. */
  spicy: boolean;
  /** How readily Pixie draws on the canvas. */
  canvas_use: "asked" | "helps" | "often";
  google_client_id: string;
  google_client_secret: string;
}
export interface SettingsView {
  settings: Settings;
  provider: string | null;
  key_hint: string;
  active?: string | null;
  effective?: Record<string, ModelChoice>;
}
export type PermWord = "granted" | "denied" | "restricted" | "not_determined" | "write_only" | "ask" | "unknown";
export interface AppsStatus {
  supported: boolean;
  on: Record<string, boolean>;
  calendar: PermWord;
  reminders: PermWord;
  contacts: PermWord;
  mail: PermWord;
  google: { configured: boolean; signed_in: boolean; email: string };
}
export interface ConnectorCard {
  id: string;
  name: string;
  blurb?: string;
  help?: string;
  help_url?: string;
  fields?: { key: string; label: string; placeholder: string }[];
  folder?: boolean;
  command?: string;
  added: boolean;
  on: boolean;
  tools: number | null;
  error: string | null;
  env: Record<string, string> | null;
  args: string[] | null;
  /** Page section: work, notes, search, files, custom. */
  group?: string;
  /** token, browser (sign in in the browser), none, folder, custom. */
  auth?: string;
  /** A community server rather than the service's own. */
  community?: boolean;
  tool_list?: { name: string; description: string }[] | null;
}
export interface Gallery {
  presets: ConnectorCard[];
  custom: ConnectorCard[];
  node: boolean;
  path: string;
}
export interface Voice {
  id: string;
  name: string;
  language: string;
  quality: "premium" | "enhanced" | "default";
}

export type SlotKind = "sound" | "icon" | "video" | "font";

export interface UpdateStatus {
  current: string;
  phase: "idle" | "checking" | "downloading" | "ready" | "up_to_date" | "error";
  version: string | null;
  notes: string | null;
  error: string | null;
  done: number;
  total: number;
  checked_at: number | null;
  auto: boolean;
  /** False in dev builds. */
  supported: boolean;
}

/** Which desktop this is ("mac" in the browser preview too). */
export const OS: "mac" | "windows" | "linux" = /Windows/.test(navigator.userAgent) ? "windows" : /Linux|X11/.test(navigator.userAgent) && !/Mac/.test(navigator.userAgent) ? "linux" : "mac";
export const IS_MAC = OS === "mac";
/** "this Mac" / "this PC" in copy. */
export const MACHINE = IS_MAC ? "Mac" : "PC";

/** URL for a resource slot, served by the Rust `slot://` protocol. */
export function slotUrl(kind: SlotKind, slot: string): string {
  if (document.documentElement.classList.contains("preview") || new URLSearchParams(location.search).has("preview")) return `/__slot/${kind}/${slot}`;
  const windowsLike = /Windows|Android/.test(navigator.userAgent);
  return windowsLike ? `http://slot.localhost/${kind}/${slot}` : `slot://localhost/${kind}/${slot}`;
}

export const api = {
  panelMode: () => invoke<PanelMode>("panel_mode"),
  panelScreen: () => invoke<ScreenInfo>("panel_screen"),
  hidePanel: () => invoke<void>("hide_panel"),
  hitRects: (window: string, rects: Rect[]) => invoke<void>("hit_rects", { window, rects }),
  panelHover: (hovered: boolean) => invoke<void>("panel_hover", { hovered }),
  panelType: () => invoke<void>("panel_type"),
  panelPeek: (on: boolean) => invoke<void>("panel_peek", { on }),
  talkStart: () => invoke<void>("talk_start"),
  chatLog: () => invoke<ChatEntry[]>("chat_log"),
  chatClear: () => invoke<void>("chat_clear"),
  chatsList: () => invoke<ChatSummary[]>("chats_list"),
  chatGet: (id: string) => invoke<{ id: string; title: string; started: number; updated: number; entries: ChatEntry[] } | null>("chat_get", { id }),
  chatDelete: (id: string) => invoke<void>("chat_delete", { id }),
  openChats: (id?: string) => invoke<void>("open_chats", { id }),
  talkStop: () => invoke<void>("talk_stop"),
  voiceStatus: () => invoke<{ supported?: boolean; installed: boolean; ready: boolean; installing: boolean; voices: { id: number; name: string; description: string }[]; done: number; total: number }>("voice_status"),
  voiceInstall: () => invoke<void>("voice_install"),
  ask: (text: string) => invoke<void>("ask", { text }),
  sessionNote: () => invoke<NoteDoc | null>("session_note"),
  playSound: (slot: string) => invoke<void>("play_sound", { slot }),
  stopBed: () => invoke<void>("stop_bed"),
  showSettings: (tab?: string) => invoke<void>("show_settings", { tab }),
  resourcesReport: () => invoke<ResourcesReport>("resources_report"),
  theme: () => invoke<Theme>("theme"),
  jobs: () => invoke<Job[]>("jobs_list"),
  job: (id: string) => invoke<Job | null>("job_get", { id }),
  jobNote: (id: string) => invoke<NoteDoc | null>("job_note", { id }),
  jobStart: (title: string, instructions: string) => invoke<string>("job_start", { title, instructions }),
  jobAnswer: (id: string, text: string) => invoke<void>("job_answer", { id, text }),
  jobCancel: (id: string) => invoke<void>("job_cancel", { id }),
  jobDismiss: (id: string) => invoke<void>("job_dismiss", { id }),
  jobDelete: (id: string) => invoke<void>("job_delete", { id }),
  openNote: (id: string) => invoke<void>("open_note", { id }),
  appsStatus: () => invoke<AppsStatus>("apps_status"),
  appSet: (kind: string, on: boolean) => invoke<string>("app_set", { kind, on }),
  googleSignIn: () => invoke<string>("google_sign_in"),
  googleSignOut: () => invoke<void>("google_sign_out"),
  connectorsGallery: () => invoke<Gallery>("connectors_gallery"),
  connectorSave: (id: string, values: Record<string, string>, on: boolean) => invoke<Gallery>("connector_save", { id, values, on }),
  connectorRemove: (id: string) => invoke<Gallery>("connector_remove", { id }),
  connectorRetry: (id: string) => invoke<Gallery>("connector_retry", { id }),
  connectorAddCustom: (name: string, command: string, args: string[], env: Record<string, string>) => invoke<Gallery>("connector_add_custom", { name, command, args, env }),
  openTarget: (target: string, reveal = false) => invoke<void>("open_target", { target, reveal }),
  updateStatus: () => invoke<UpdateStatus>("update_status"),
  updateCheck: () => invoke<UpdateStatus>("update_check"),
  updateRestart: () => invoke<void>("update_restart"),
  canvasOpen: (id?: string) => invoke<string>("canvas_open", { id: id ?? null }),
  memoryList: () => invoke<MemoryNote[]>("memory_list"),
  memoryAdd: (text: string, kind: string) => invoke<MemoryNote>("memory_add", { text, kind }),
  memoryUpdate: (id: string, text: string, kind?: string) => invoke<MemoryNote>("memory_update", { id, text, kind: kind ?? null }),
  memoryDelete: (id: string) => invoke<void>("memory_delete", { id }),
  memoryClear: () => invoke<void>("memory_clear"),
  settings: () => invoke<SettingsView>("settings_get"),
  setSettings: (patch: Partial<Settings>) => invoke<SettingsView>("settings_set", { patch }),
  providerTest: () => invoke<string>("provider_test"),
  voices: () => invoke<{ voices: Voice[]; default: string }>("voices"),
  voiceTest: (text?: string) => invoke<void>("voice_test", { text }),
  onboardingDone: () => invoke<void>("onboarding_done"),
  status: () =>
    invoke<{ helper: boolean; provider: string | null; browser: boolean; connectors: string[]; data_dir: string; version: string }>(
      "status",
    ),
  permissions: () => invoke<PermissionInfo[]>("permissions_status"),
  requestPermission: (id: string) => invoke<PermissionInfo>("permission_request", { id }),
  openPermissionSettings: (id: string) => invoke<void>("permission_open_settings", { id }),
  relaunch: () => invoke<void>("relaunch"),
  modelsAvailable: (provider: string) =>
    invoke<{ models: { id: string; name: string; vision: boolean | null; created: number | null; price: { input: number; output: number } | null; context: number | null }[]; prices_at: number }>("models_available", { provider }),
  pricesRefresh: () => invoke<{ models: number; fetched_at: number }>("prices_refresh"),
  usageSummary: (days: number, provider = "", kind = "") => invoke<UsageSummary>("usage_summary", { days, provider, kind }),
  soundCustom: (slot: string, name: string, data: string) => invoke<SettingsView>("sound_custom", { slot, name, data }),
  micTestStart: () => invoke<void>("mic_test_start"),
  micTestStop: () => invoke<string>("mic_test_stop"),
  codersStatus: () =>
    invoke<{ agents: { id: string; label: string; installed: boolean; path: string | null; version: string | null }[]; t3code: boolean }>("coders_status"),
  guideShow: (id: string) => invoke<void>("guide_show", { id }),
  guideHide: () => invoke<void>("guide_hide"),
  resetPermission: (id: string) => invoke<PermissionInfo>("permission_reset", { id }),
};

export function onEvent(cb: (e: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>(EVENT_CHANNEL, (e) => cb(e.payload));
}
