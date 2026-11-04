import { useEffect, useState } from "react";
import { api, onEvent, type PanelMode } from "../lib/bridge";

const BARS = 28;

/** Live waveform while listening. TODO(M2): drive bar heights from mic RMS levels. */
function Waveform({ active }: { active: boolean }) {
  return (
    <div className={`wave ${active ? "wave--on" : ""}`} aria-hidden>
      {Array.from({ length: BARS }, (_, i) => (
        <span key={i} style={{ animationDelay: `${(i * 97) % 600}ms` }} />
      ))}
    </div>
  );
}

const COPY: Record<PanelMode, { title: string; sub: string }> = {
  listening: { title: "Listening…", sub: "Let go when you're done." },
  idle: { title: "I'm here.", sub: "Hold ⌃⌥ and talk. My voice arrives in the next build." },
  hidden: { title: "", sub: "" },
};

export function Panel() {
  const [mode, setMode] = useState<PanelMode>("hidden");

  useEffect(() => {
    api.panelMode().then(setMode).catch(() => {});
    const un = onEvent((e) => {
      if (e.kind === "panel.state") setMode((e.payload as { mode: PanelMode }).mode);
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") api.hidePanel();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const listening = mode === "listening";
  const copy = COPY[mode === "hidden" ? "idle" : mode];

  return (
    <main className={`panel panel--${mode}`}>
      <header className="panel__head">
        <div className={`orb ${listening ? "orb--live" : ""}`} />
        <div className="panel__titles">
          <h1>{copy.title}</h1>
          <p>{copy.sub}</p>
        </div>
        <button className="panel__close" onClick={() => api.hidePanel()} aria-label="Close">
          ×
        </button>
      </header>

      <Waveform active={listening} />

      {/* TODO(M5): live note.md preview rendered from the note DSL. */}
      <section className="panel__note">
        <span className="hand">Your words and my answer show up right here.</span>
      </section>

      <footer className="panel__foot">
        <span>
          <kbd>esc</kbd> close
        </span>
        <span>
          tap <kbd>⌃⌥</kbd> to dismiss
        </span>
      </footer>
    </main>
  );
}
