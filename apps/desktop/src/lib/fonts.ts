import { slotUrl } from "./bridge";

// Load slot fonts at runtime; if a slot is missing the CSS stack falls back to system fonts.
const FACES: [family: string, slot: string, weight: string][] = [
  ["Barlow", "barlow-regular", "400"],
  ["Barlow", "barlow-medium", "500"],
  ["Barlow", "barlow-semibold", "600"],
  ["Patrick Hand", "patrick-hand", "400"],
];

export function loadSlotFonts(): void {
  for (const [family, slot, weight] of FACES) {
    const face = new FontFace(family, `url("${slotUrl("font", slot)}")`, { weight });
    face
      .load()
      .then((f) => document.fonts.add(f))
      .catch(() => console.warn(`font slot ${slot} unavailable; using system font`));
  }
}
