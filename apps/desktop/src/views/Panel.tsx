// The notch: a black shape that grows out of the camera housing. It listens (live waveform +
// transcript), thinks, answers (spoken words light up as they're said), shows the paper note,
// takes typed questions, and pops short toasts.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, onEvent, type NoteDoc, type PanelMode, type ScreenInfo } from "../lib/bridge";
import { useHitRects } from "../lib/hits";
import { Mark } from "../components/Mark";
import { NoteView } from "../components/NoteView";

const BARS = 22;

interface Segment {
  id: string;
  text: string;
}
interface Chip {
  name: string;
  label: string;
  done: boolean;
  ok: boolean;
}
interface Toast {
  text: string;
  kind: string;
  job: string | null;
}

const TOOL_LABEL: Record<string, string> = {
  highlight: "Pointing",
  draw: "Drawing",
  mac_step: "Doing it",
  web_search: "Searching",
  fetch_url: "Reading",
  start_job: "Starting a job",
  note: "Writing",
  note_answer: "Writing",
  device_find_files: "Looking in your files",
  device_list_folder: "Looking in your files",
  device_read_file: "Reading a file",
  device_open: "Opening",
  browser: "In your browser",
};

function chipLabel(name: string, input: any): string {
  const base = TOOL_LABEL[name] ?? (name.startsWith("mcp__") ? name.split("__")[1] : name);
  if (name === "highlight" && input?.label) return `Pointing at ${input.label}`;
  if (name === "web_search" && input?.query) return `Searching “${String(input.query).slice(0, 28)}”`;
  if (name === "start_job" && input?.title) return `New job: ${input.title}`;
  if (name === "mac_step" && input?.target) return `${input.action} ${String(input.target).slice(0, 24)}`;
  return base;
}

function Waveform({ levels, live }: { levels: number[]; live: boolean }) {
  return (
    <div className={`wave ${live ? "wave--live" : ""}`} aria-hidden>
      {levels.map((l, i) => (
        <span key={i} style={{ height: `${3 + Math.round(l * 18)}px`, opacity: 0.45 + l * 0.55 }} />
      ))}
    </div>
  );
}

/** A spoken segment with the current word lit. */
function Spoken({ seg, word, spoken }: { seg: Segment; word: { at: number; len: number } | null; spoken: boolean }) {
  if (!word) return <span className={spoken ? "seg seg--spoken" : "seg"}>{seg.text} </span>;
  const { at, len } = word;
  return (
    <span className="seg seg--speaking">
      <span className="seg--spoken">{seg.text.slice(0, at)}</span>
      <mark>{seg.text.slice(at, at + len)}</mark>
      {seg.text.slice(at + len)}{" "}
    </span>
  );
}

export function Panel() {
  const [mode, setMode] = useState<PanelMode>("hidden");
  const [screen, setScreen] = useState<ScreenInfo | null>(null);
  const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0));
  const [partial, setPartial] = useState("");
  const [question, setQuestion] = useState("");
  const [segments, setSegments] = useState<Segment[]>([]);
  const [word, setWord] = useState<{ id: string; at: number; len: number } | null>(null);
  const [spoken, setSpoken] = useState<Set<string>>(new Set());
  const [chips, setChips] = useState<Chip[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [empty, setEmpty] = useState(false);
  const [note, setNote] = useState<NoteDoc | null>(null);
  const [toast, setToast] = useState<Toast | null>(null);
  const [typed, setTyped] = useState("");
  const [dictated, setDictated] = useState("");
  const [bodyH, setBodyH] = useState(0);
  const turn = useRef(0);
  const shape = useRef<HTMLDivElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);

  const reset = () => {
    setPartial("");
    setQuestion("");
    setSegments([]);
    setWord(null);
    setSpoken(new Set());
    setChips([]);
    setError(null);
    setEmpty(false);
    setDictated("");
  };

  useEffect(() => {
    api.panelMode().then(setMode).catch(() => {});
    api.panelScreen().then(setScreen).catch(() => {});
    const un = onEvent((e) => {
      const p = e.payload;
      switch (e.kind) {
        case "panel.state":
          setMode(p.mode);
          if (p.mode === "typing") setTimeout(() => input.current?.focus(), 60);
          break;
        case "panel.screen":
          setScreen(p.screen);
          break;
        case "mic.start":
          turn.current = p.turn;
          reset();
          setNote(null);
          setLevels(Array(BARS).fill(0));
          break;
        case "mic.level":
          setLevels((l) => [...l.slice(1), Math.min(1, p.level * 1.15)]);
          break;
        case "mic.partial":
          setPartial(p.text);
          break;
        case "turn.start":
          if (p.turn !== turn.current) {
            turn.current = p.turn;
            reset();
          }
          setQuestion(p.text);
          setPartial("");
          break;
        case "turn.empty":
          setEmpty(true);
          break;
        case "speech.segment":
          if (p.turn === undefined || p.turn === turn.current) setSegments((s) => [...s, { id: p.id, text: p.text }]);
          break;
        case "speech.word":
          setWord({ id: p.id, at: p.at, len: p.len });
          break;
        case "speech.end":
          setSpoken((s) => new Set(s).add(p.id));
          setWord((w) => (w?.id === p.id ? null : w));
          break;
        case "turn.tool":
          if (p.turn === turn.current) setChips((c) => [...c, { name: p.name, label: chipLabel(p.name, p.input), done: false, ok: true }]);
          break;
        case "turn.tool_done":
          if (p.turn === turn.current)
            setChips((c) => {
              const i = c.findIndex((x) => x.name === p.name && !x.done);
              if (i < 0) return c;
              const n = [...c];
              n[i] = { ...n[i], done: true, ok: p.ok };
              return n;
            });
          break;
        case "turn.error":
          setError(p.message);
          break;
        case "session.note":
          setNote(p.doc && p.doc.pages?.length ? p.doc : p.doc?.title ? p.doc : null);
          break;
        case "toast":
          setToast({ text: p.text, kind: p.kind, job: p.job ?? null });
          break;
        case "dictation.raw":
          setDictated(p.text);
          break;
        case "dictation.done":
          setDictated(p.text);
          setToast({ text: "Typed it", kind: "done", job: null });
          break;
      }
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

  // Measure the body so the shape's height can spring (CSS can't animate to `auto`).
  useLayoutEffect(() => {
    if (!body.current) return;
    const ro = new ResizeObserver(() => setBodyH(body.current?.scrollHeight ?? 0));
    ro.observe(body.current);
    return () => ro.disconnect();
  }, [mode]);

  useHitRects("panel", [shape], [mode, bodyH]);

  const notchW = screen?.notch_width ?? 0;
  const top = Math.round(screen?.top_inset ?? 32);
  const hasNotch = notchW > 0;
  const live = mode === "listening" || mode === "dictating";
  const wide = mode === "thinking" || mode === "answer" || mode === "idle" || mode === "typing";
  const ear = mode === "toast" ? 170 : live ? 96 : 120;
  const width =
    mode === "hidden" ? Math.max(notchW, 120) : wide ? Math.max(notchW + 2 * 150, 540) : Math.max(notchW + 2 * ear, hasNotch ? 0 : 300);
  const showBody = mode !== "hidden" && mode !== "toast" && !(live && !partial && !dictated);
  const height = mode === "hidden" ? top : top + (showBody ? Math.min(bodyH, 380) : 0);
  const speakingNow = word !== null;

  const ask = (text: string) => {
    if (!text.trim()) return;
    api.ask(text.trim()).catch((e) => setError(String(e)));
    setTyped("");
  };

  const rightEar = () => {
    if (mode === "listening") return <Waveform levels={levels} live />;
    if (mode === "dictating")
      return (
        <div className="ear__stack">
          <span className="ear__label">Dictating</span>
          <Waveform levels={levels.slice(-10)} live />
        </div>
      );
    if (mode === "toast")
      return (
        <button className={`ear__toast ear__toast--${toast?.kind ?? "info"}`} onClick={() => toast?.job && api.openNote(toast.job)}>
          {toast?.text}
        </button>
      );
    if (mode === "thinking") return <span className="ear__label ear__label--think">thinking</span>;
    if (speakingNow) return <Waveform levels={levels.map((_, i) => 0.25 + 0.5 * Math.abs(Math.sin((Date.now() / 160 + i) * 0.9)))} live />;
    return <span className="ear__label">{mode === "answer" ? "" : "⌃⌥ to talk"}</span>;
  };

  return (
    <div className={`notch-root ${hasNotch ? "has-notch" : "no-notch"}`} data-mode={mode}>
      <div
        ref={shape}
        className={`notch notch--${mode}`}
        style={{ width, height }}
        onMouseEnter={() => api.panelHover(true)}
        onMouseLeave={() => api.panelHover(false)}
      >
        <div className="notch__bar" style={{ height: top }}>
          <div className="ear ear--left">
            <Mark size={Math.min(24, top - 8)} live={live || speakingNow} mood={mode === "thinking" ? "think" : "smile"} />
          </div>
          <div className="notch__gap" style={{ width: notchW }} />
          <div className="ear ear--right">{rightEar()}</div>
        </div>

        <div className="notch__body" ref={body}>
          {live && (partial || dictated) && <div className="transcript">{dictated || partial}</div>}

          {(mode === "thinking" || mode === "answer") && (
            <div className="turn">
              {question && <div className={`turn__q ${mode === "answer" && segments.length ? "turn__q--small" : ""}`}>{question}</div>}
              {empty && <div className="turn__a turn__a--muted">I didn't catch that. Hold ⌃⌥ and talk to me.</div>}
              {segments.length > 0 && (
                <div className="turn__a">
                  {segments.map((s) => (
                    <Spoken key={s.id} seg={s} word={word?.id === s.id ? word : null} spoken={spoken.has(s.id)} />
                  ))}
                </div>
              )}
              {mode === "thinking" && !segments.length && !error && (
                <div className="shimmer">
                  <i />
                  <i />
                  <i />
                </div>
              )}
              {chips.length > 0 && (
                <div className="chips">
                  {chips.map((c, i) => (
                    <span key={i} className={`chip chip--${c.done ? (c.ok ? "ok" : "bad") : "busy"}`}>
                      {c.label}
                    </span>
                  ))}
                </div>
              )}
              {error && (
                <div className="turn__error">
                  {error}
                  {/key|Settings|permission|Microphone|Speech/i.test(error) && (
                    <button className="link" onClick={() => api.showSettings(/key/i.test(error) ? "ai" : "setup")}>
                      Open Settings
                    </button>
                  )}
                </div>
              )}
              {note && (
                <div className="turn__paper">
                  <NoteView doc={note} compact />
                </div>
              )}
              {mode === "answer" && !error && (
                <div className="turn__foot">
                  <button className="link" onClick={() => api.panelType()}>
                    Ask a follow-up
                  </button>
                  <span>
                    hold <kbd>⌃⌥</kbd> to talk · <kbd>esc</kbd>
                  </span>
                </div>
              )}
            </div>
          )}

          {(mode === "idle" || mode === "typing") && (
            <div className="composer">
              <form
                className="composer__box"
                onMouseDown={() => mode !== "typing" && api.panelType()}
                onSubmit={(e) => {
                  e.preventDefault();
                  ask(typed);
                }}
              >
                <input
                  ref={input}
                  value={typed}
                  placeholder="Ask about anything on your screen…"
                  onChange={(e) => setTyped(e.target.value)}
                  spellCheck={false}
                />
                <button type="submit" className="composer__send" disabled={!typed.trim()} aria-label="Ask">
                  ↑
                </button>
              </form>
              <div className="composer__hint">
                <span>
                  or hold <kbd>⌃</kbd>
                  <kbd>⌥</kbd> and just say it
                </span>
                <span>
                  <kbd>fn</kbd>
                  <kbd>⌃</kbd> dictates · <kbd>esc</kbd> closes
                </span>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
