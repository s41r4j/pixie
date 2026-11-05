import { slotUrl } from "./bridge";

// Pixel design system v1 fonts (OFL), loaded from slots; CSS stacks fall back to monospace.
const FACES: [family: string, slot: string, weight: string][] = [
  ["IBM Plex Mono", "plex-mono-regular", "400"],
  ["IBM Plex Mono", "plex-mono-medium", "500"],
  ["IBM Plex Mono", "plex-mono-semibold", "600"],
  ["VT323", "vt323", "400"],
  ["Press Start 2P", "press-start", "400"],
  ["Patrick Hand", "patrick-hand", "400"],
];

export function loadSlotFonts(): void {
  for (const [family, slot, weight] of FACES) {
    const face = new FontFace(family, `url("${slotUrl("font", slot)}")`, { weight });
    face
      .load()
      .then((f) => document.fonts.add(f))
      .catch(() => console.warn(`font slot ${slot} unavailable; using monospace`));
  }
}
