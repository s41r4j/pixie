// Typed bridge to the Rust side. Media is addressed by slot name only (resources/manifest.json).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const EVENT_CHANNEL = "openclicky://event";

export type PanelMode = "hidden" | "listening" | "idle";

export interface CoreEvent<P = Record<string, unknown>> {
  kind: string;
  job_id: string | null;
  payload: P;
}

export type PermState =
  | "granted"
  | "denied"
  | "restricted"
  | "not_determined"
  | "not_granted"
  | "unchecked";

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
  sounds: number;
  missing: { kind: string; slot: string; file: string }[];
}

export type SlotKind = "sound" | "icon" | "video" | "font";

/** URL for a resource slot, served by the Rust `slot://` protocol. */
export function slotUrl(kind: SlotKind, slot: string): string {
  const windowsLike = /Windows|Android/.test(navigator.userAgent);
  return windowsLike ? `http://slot.localhost/${kind}/${slot}` : `slot://localhost/${kind}/${slot}`;
}

export const api = {
  panelMode: () => invoke<PanelMode>("panel_mode"),
  hidePanel: () => invoke<void>("hide_panel"),
  playSound: (slot: string) => invoke<void>("play_sound", { slot }),
  stopBed: () => invoke<void>("stop_bed"),
  showPermissions: () => invoke<void>("show_permissions"),
  resourcesReport: () => invoke<ResourcesReport>("resources_report"),
  permissions: () => invoke<PermissionInfo[]>("permissions_status"),
  requestPermission: (id: string) => invoke<PermissionInfo>("permission_request", { id }),
  openPermissionSettings: (id: string) => invoke<void>("permission_open_settings", { id }),
};

export function onEvent(cb: (e: CoreEvent) => void): Promise<UnlistenFn> {
  return listen<CoreEvent>(EVENT_CHANNEL, (e) => cb(e.payload));
}
