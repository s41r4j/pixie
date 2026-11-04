// The openclicky mark: `:)` on a gel orb.
export function Mark({ size = 30, live = false, mood = "smile" }: { size?: number; live?: boolean; mood?: "smile" | "think" | "listen" }) {
  return (
    <div
      className={`mark ${live ? "mark--live" : ""} mark--${mood}`}
      style={{ width: size, height: size, fontSize: size * 0.46 }}
      aria-label="openclicky"
    >
      <span className="mark__face">{mood === "think" ? ":o" : ":)"}</span>
    </div>
  );
}

/** The wordmark: `:)` + name. */
export function Wordmark() {
  return (
    <span className="wordmark">
      <span className="wordmark__smile">:)</span> openclicky
    </span>
  );
}
