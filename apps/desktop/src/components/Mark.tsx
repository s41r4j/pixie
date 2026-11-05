// The openclicky mark: the `:)` glyph (design-system geometry), drawn in the current text
// colour. `Mark` puts it on a quiet graphite tile for windows.

export function Smile({ size = 22, live = false, mood = "smile" }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen" }) {
  return (
    <svg className={`smile ${live ? "smile--live" : ""} smile--${mood}`} width={size} height={size} viewBox="230 230 600 560" aria-label="openclicky">
      <g fill="currentColor" className="smile__eyes">
        <circle cx="408" cy="405" r="56" />
        <circle cx="408" cy="607" r="56" />
      </g>
      <path d="M584 316 C721 408 719 612 578 713" fill="none" stroke="currentColor" strokeWidth="96" strokeLinecap="round" />
    </svg>
  );
}

export function Mark({ size = 30, live = false }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen" }) {
  return (
    <span className={`mark ${live ? "mark--live" : ""}`} style={{ width: size, height: size, borderRadius: size * 0.24 }}>
      <Smile size={size * 0.56} />
    </span>
  );
}

/** The wordmark: `:)` + name. */
export function Wordmark() {
  return (
    <span className="wordmark">
      <Smile size={15} /> openclicky
    </span>
  );
}
