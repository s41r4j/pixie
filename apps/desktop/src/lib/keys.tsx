// Shortcut chords ("ctrl+option") as symbols, and a hook that follows Settings.
import { useEffect, useState } from "react";
import { api, onEvent } from "./bridge";

const SYM: Record<string, string> = { ctrl: "⌃", control: "⌃", option: "⌥", opt: "⌥", alt: "⌥", cmd: "⌘", command: "⌘", shift: "⇧", fn: "fn" };
const ORDER = ["fn", "⌃", "⌥", "⇧", "⌘"];

/** "ctrl+option" → ["⌃", "⌥"] in the order macOS shows them. */
export function chordKeys(chord: string): string[] {
  return chord
    .split("+")
    .map((p) => SYM[p.trim().toLowerCase()] ?? p.trim())
    .filter(Boolean)
    .sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b));
}

export function chordLabel(chord: string): string {
  return chordKeys(chord).join("");
}

export const PRESETS = ["ctrl+option", "fn", "fn+ctrl", "ctrl+shift", "option+cmd", "ctrl+cmd", "option+shift", "fn+option"];

export function useShortcuts() {
  const [keys, setKeys] = useState({ talk: "ctrl+option", dictate: "fn+ctrl" });
  useEffect(() => {
    const load = () =>
      api
        .settings()
        .then((v) => setKeys({ talk: v.settings.shortcut_talk || "ctrl+option", dictate: v.settings.shortcut_dictate || "fn+ctrl" }))
        .catch(() => {});
    load();
    const un = onEvent((e) => e.kind === "settings.changed" && load());
    return () => {
      un.then((f) => f());
    };
  }, []);
  return keys;
}

export function Kbd({ chord }: { chord: string }) {
  return (
    <>
      {chordKeys(chord).map((k) => (
        <kbd key={k}>{k}</kbd>
      ))}
    </>
  );
}
