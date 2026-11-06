// The Pixie mark: the pixel `:)` from the design system, drawn as a mask so it takes the
// current text colour. `Mark` puts it in a chamfered black tile with a white frame.
import { slotUrl } from "../lib/bridge";
import { personaOf, usePersona } from "../lib/persona";

/** Pixie's face: the persona's (:) 8) XD ;)), or the brand :) with `brand`. */
export function Smile({ size = 22, live = false, mood = "smile", brand = false }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen"; brand?: boolean }) {
  const persona = usePersona();
  const icon = brand ? "brand-mark-white" : personaOf(persona).icon;
  return (
    <i
      className={`pmark ${live ? "pmark--live" : ""} pmark--${mood}`}
      style={{ width: size, height: size, ["--icon" as string]: `url(${slotUrl("icon", icon)})` }}
      aria-label="Pixie"
      role="img"
    />
  );
}

export function Mark({ size = 30, live = false }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen" }) {
  return (
    <span className={`ptile ${live ? "ptile--live" : ""}`} style={{ width: size, height: size }}>
      <Smile size={size * 0.9} />
    </span>
  );
}

/** Pixel glyph from the system icon set, tinted with currentColor. */
export function Glyph({ name, size = 16 }: { name: string; size?: number }) {
  return <i className="pglyph" style={{ width: size, height: size, ["--icon" as string]: `url(${slotUrl("icon", `system-${name}`)})` }} aria-hidden />;
}

export function Wordmark() {
  return (
    <span className="wordmark">
      <Smile size={18} brand /> Pixie
    </span>
  );
}
