// Shortcut chords ("ctrl+option") as symbols, and a hook that follows Settings.
import { useEffect, useState } from "react";
import { api, IS_MAC, onEvent } from "./bridge";

const MAC_SYM: Record<string, string> = { ctrl: "⌃", control: "⌃", option: "⌥", opt: "⌥", alt: "⌥", cmd: "⌘", command: "⌘", shift: "⇧", fn: "fn" };
// Windows / Linux: ⌘ is the Windows key, ⌥ is Alt; fn can't be seen, so it reads as Ctrl+Win.
const PC_SYM: Record<string, string> = { ctrl: "Ctrl", control: "Ctrl", option: "Alt", opt: "Alt", alt: "Alt", cmd: "Win", command: "Win", shift: "Shift", fn: "Win" };
const SYM = IS_MAC ? MAC_SYM : PC_SYM;
const ORDER = IS_MAC ? ["fn", "⌃", "⌥", "⇧", "⌘"] : ["Ctrl", "Win", "Alt", "Shift"];

/** "ctrl+option" → ["⌃", "⌥"] in the order macOS shows them. */
export function chordKeys(chord: string): string[] {
  if (!IS_MAC && /(^|\+)\s*fn\s*(\+|$)/i.test(chord)) return ["Ctrl", "Win"];
  return chord
    .split("+")
    .map((p) => SYM[p.trim().toLowerCase()] ?? p.trim())
    .filter((k, i, all) => Boolean(k) && all.indexOf(k) === i)
    .sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b));
}

export function chordLabel(chord: string): string {
  return chordKeys(chord).join(IS_MAC ? "" : "+");
}

export const PRESETS = IS_MAC
  ? ["ctrl+option", "fn", "fn+ctrl", "ctrl+shift", "option+cmd", "ctrl+cmd", "option+shift", "fn+option"]
  : ["ctrl+option", "ctrl+cmd", "ctrl+shift", "option+shift", "option+cmd"];

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
