// Pixie's personalities. They change the style of replies (and the face), never the voice,
// the tools or how careful Pixie is. Pix is Pixie as it always was.
import { useEffect, useState } from "react";
import { api, onEvent } from "./bridge";

export type PersonaId = "pix" | "inos" | "xtrem" | "eon";

export const PERSONAS: { id: PersonaId; name: string; face: string; style: string; sample: string; icon: string }[] = [
  { id: "pix", name: "Pix", face: ":)", style: "Friendly, warm, supportive. Pixie as you know it.", sample: "Found it: the share button's top right. Want me to click it?", icon: "brand-mark-white" },
  { id: "inos", name: "Inos", face: "8)", style: "Intelligent, nerdy, curious. Precise terms and the why.", sample: "Share's top right: the square with the arrow, the standard share glyph on the Mac.", icon: "brand-face-inos" },
  { id: "xtrem", name: "Xtrem", face: "XD", style: "Chaotic, witty, mischievous. Jokes about the situation, never about you.", sample: "Top right, hiding like it owes you money. Want me to grab it?", icon: "brand-face-xtrem" },
  { id: "eon", name: "Eon", face: ";)", style: "Calm, concise, precise. The answer, nothing else.", sample: "Top right.", icon: "brand-face-eon" },
];

export function personaOf(id: string | undefined) {
  return PERSONAS.find((p) => p.id === id) ?? PERSONAS[0];
}

// One shared value for every component in this window, refreshed when settings change anywhere.
let current: PersonaId = "pix";
let loaded = false;
const listeners = new Set<(p: PersonaId) => void>();

function set(p: PersonaId) {
  if (p === current) return;
  current = p;
  listeners.forEach((f) => f(p));
}

function refresh() {
  api
    .settings()
    .then((v) => set(personaOf(v.settings.persona).id))
    .catch(() => {});
}

function ensureLoaded() {
  if (loaded) return;
  loaded = true;
  refresh();
  onEvent((e) => {
    if (e.kind === "settings.changed" || e.kind === "settings.external") refresh();
  });
}

/** The persona in use (re-renders when it changes). */
export function usePersona(): PersonaId {
  const [p, setP] = useState<PersonaId>(current);
  useEffect(() => {
    ensureLoaded();
    listeners.add(setP);
    setP(current);
    return () => {
      listeners.delete(setP);
    };
  }, []);
  return p;
}
