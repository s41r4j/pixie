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
  can_click: boolean;
  user_name: string;
  locale: string;
}
export interface SettingsView {
  settings: Settings;
  provider: string | null;
  key_hint: string;
}
export interface Voice {
  id: string;
  name: string;
  language: string;
  quality: "premium" | "enhanced" | "default";
}

export type SlotKind = "sound" | "icon" | "video" | "font";

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
  talkStop: () => invoke<void>("talk_stop"),
  voiceStatus: () => invoke<{ installed: boolean; ready: boolean; installing: boolean; voices: { id: number; name: string; description: string }[] }>("voice_status"),
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
  openTarget: (target: string, reveal = false) => invoke<void>("open_target", { target, reveal }),
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
  boardClose: () => invoke<void>("board_close"),
  modelsAvailable: (provider: string) => invoke<{ models: { id: string; name: string; vision: boolean | null }[] }>("models_available", { provider }),
  usageSummary: (days: number) => invoke<UsageSummary>("usage_summary", { days }),
  codersStatus: () =>
    invoke<{ agents: { id: string; label: string; installed: boolean; path: string | null; version: string | null }[]; t3code: boolean }>("coders_status"),
  guideShow: (id: string) => invoke<void>("guide_show", { id }),
  guideHide: () => invoke<void>("guide_hide"),
  resetPermission: (id: string) => invoke<PermissionInfo>("permission_reset", { id }),
};

export function onEvent(cb: (e: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>(EVENT_CHANNEL, (e) => cb(e.payload));
}
