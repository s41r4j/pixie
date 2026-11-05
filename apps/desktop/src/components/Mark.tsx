// The openclicky mark. `Smile` is the bare `:)` glyph (design system v2 geometry) in the
// signature violet → rose → gold gradient; `Mark` is the full app-icon tile.
import { useId } from "react";
import { slotUrl } from "../lib/bridge";

export function Smile({ size = 22, live = false, mood = "smile" }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen" }) {
  const id = useId().replace(/:/g, "");
  return (
    <svg
      className={`smile ${live ? "smile--live" : ""} smile--${mood}`}
      width={size}
      height={size}
      viewBox="230 230 600 560"
      aria-label="openclicky"
    >
      <defs>
        <linearGradient id={`sg${id}`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--brand-violet)" />
          <stop offset="0.55" stopColor="#E6ACD3" />
          <stop offset="1" stopColor="var(--brand-gold)" />
        </linearGradient>
      </defs>
      <g fill={`url(#sg${id})`} className="smile__eyes">
        <circle cx="408" cy="405" r="56" />
        <circle cx="408" cy="607" r="56" />
      </g>
      <path d="M584 316 C721 408 719 612 578 713" fill="none" stroke={`url(#sg${id})`} strokeWidth="96" strokeLinecap="round" className="smile__mouth" />
    </svg>
  );
}

export function Mark({ size = 30, live = false }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen" }) {
  return (
    <span className={`mark ${live ? "mark--live" : ""}`} style={{ width: size, height: size }}>
      <img src={slotUrl("icon", "app-icon-1024")} alt="openclicky" draggable={false} />
    </span>
  );
}

/** The wordmark: `:)` + name. */
export function Wordmark() {
  return (
    <span className="wordmark">
      <Smile size={16} /> openclicky
    </span>
  );
}
