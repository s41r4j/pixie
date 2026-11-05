// Full-screen, click-through layer for Pixie's buddy and marks.
// The buddy (a round pixel :) badge) rides beside the cursor. Marks are drawn *by* the buddy:
// it races along each circle/underline/arrow like a pencil, and highlights are dragged out as a
// marquee selection. No text: Pixie talks, the pointer shows.
import { useEffect, useRef, useState } from "react";
import { onEvent, slotUrl, type Rect, type ScreenInfo } from "../lib/bridge";

export interface Op {
  kind: "circle" | "underline" | "arrow" | "box" | "check" | "cross" | "text" | "line" | "select";
  x: number;
  y: number;
  w?: number;
  h?: number;
  to_x?: number;
  to_y?: number;
  text?: string;
  color?: string;
}

// Board colours (monochrome); kept here because the board imports them.
export const COLOR: Record<string, string> = {
  ink: "#000000", blue: "#000000", red: "#000000", green: "#404040", orange: "#404040", yellow: "#a0a0a0",
  pink: "#404040", white: "#ffffff", sky: "#000000", coral: "#000000", butter: "#a0a0a0", mint: "#404040", peach: "#404040",
};

type P = { x: number; y: number };

/** The path a mark is drawn along, in window coordinates. */
function opPath(op: Op, ox: number, oy: number, seed: number): string | null {
  const x = op.x - ox;
  const y = op.y - oy;
  const w = op.w ?? 80;
  const h = op.h ?? 30;
  switch (op.kind) {
    case "circle": {
      const cx = x + w / 2, cy = y + h / 2, rx = w / 2 + 12, ry = h / 2 + 10;
      const pts: string[] = [];
      for (let i = 0; i <= 40; i++) {
        const t = (i / 36) * Math.PI * 2 - 0.6;
        const wob = 1 + Math.sin(t * 3 + seed) * 0.03;
        pts.push(`${(cx + Math.cos(t) * rx * wob).toFixed(1)},${(cy + Math.sin(t) * ry * wob).toFixed(1)}`);
      }
      return `M${pts.join(" L")}`;
    }
    case "box":
    case "select":
      return `M${x - 6},${y - 6} L${x + w + 6},${y - 6} L${x + w + 6},${y + h + 6} L${x - 6},${y + h + 6} Z`;
    case "underline": {
      const yy = y + h + 5;
      let d = `M${x},${yy}`;
      for (let i = 1; i <= 8; i++) d += ` Q${x + (w * (i - 0.5)) / 8},${yy + (i % 2 ? 3.5 : -3.5)} ${x + (w * i) / 8},${yy}`;
      return d;
    }
    case "arrow": {
      const tx = (op.to_x ?? op.x + 120) - ox, ty = (op.to_y ?? op.y) - oy;
      const mx = (x + tx) / 2 + (ty - y) * 0.18, my = (y + ty) / 2 - (tx - x) * 0.18;
      const a = Math.atan2(ty - my, tx - mx);
      return `M${x},${y} Q${mx},${my} ${tx},${ty} L${tx - 16 * Math.cos(a - 0.5)},${ty - 16 * Math.sin(a - 0.5)} M${tx},${ty} L${tx - 16 * Math.cos(a + 0.5)},${ty - 16 * Math.sin(a + 0.5)}`;
    }
    case "line": {
      const tx = (op.to_x ?? op.x + 120) - ox, ty = (op.to_y ?? op.y) - oy;
      return `M${x},${y} L${tx},${ty}`;
    }
    case "check":
      return `M${x},${y + 14} L${x + 12},${y + 28} L${x + 36},${y - 6}`;
    case "cross":
      return `M${x},${y} L${x + 28},${y + 28} M${x + 28},${y} L${x},${y + 28}`;
    default:
      return null;
  }
}

interface Mark {
  id: number;
  op: Op;
  d: string | null;
}

type Mood = "idle" | "listening" | "thinking" | "speaking";

const FOLLOW = { x: 18, y: 20 };

export function Overlay() {
  const [screen, setScreen] = useState<ScreenInfo | null>(null);
  const [win, setWin] = useState<Rect | null>(null);
  const [marks, setMarks] = useState<Mark[]>([]);
  const [fading, setFading] = useState(false);
  const [buddyOn, setBuddyOn] = useState(true);
  const [mood, setMood] = useState<Mood>("idle");
  const [ripples, setRipples] = useState<{ id: number; x: number; y: number }[]>([]);
  const ids = useRef(1);
  const buddyEl = useRef<HTMLDivElement>(null);
  const pathEls = useRef(new Map<number, SVGPathElement>());
  const measure = useRef<SVGPathElement>(null);
  const origin = useRef<P>({ x: 0, y: 0 });
  const queue = useRef<Mark[]>([]);
  const busy = useRef(false);
  // buddy motion state (rAF-driven; never stepped)
  const b = useRef({
    pos: { x: -100, y: -100 } as P,
    cursor: { x: -100, y: -100 } as P,
    mode: "follow" as "follow" | "manual" | "hold" | "return",
    holdUntil: 0,
    from: { x: 0, y: 0 } as P,
    t0: 0,
  });

  const ox = win?.x ?? screen?.frame.x ?? 0;
  const oy = win?.y ?? screen?.frame.y ?? 0;
  origin.current = { x: ox, y: oy };

  // Follow / return loop.
  useEffect(() => {
    let raf = 0;
    const loop = (now: number) => {
      const s = b.current;
      const follow = { x: s.cursor.x - origin.current.x + FOLLOW.x, y: s.cursor.y - origin.current.y + FOLLOW.y };
      if (s.mode === "follow") {
        s.pos.x += (follow.x - s.pos.x) * 0.3;
        s.pos.y += (follow.y - s.pos.y) * 0.3;
      } else if (s.mode === "hold" && now > s.holdUntil) {
        s.mode = "return";
        s.from = { ...s.pos };
        s.t0 = now;
      } else if (s.mode === "return") {
        const t = Math.min(1, (now - s.t0) / 420);
        const e = 1 - Math.pow(1 - t, 3);
        s.pos = { x: s.from.x + (follow.x - s.from.x) * e, y: s.from.y + (follow.y - s.from.y) * e };
        if (t >= 1) s.mode = "follow";
      }
      if (buddyEl.current) buddyEl.current.style.transform = `translate(${s.pos.x - 15}px, ${s.pos.y - 15}px)`;
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);

  /** Move the buddy to p over ms along a slight arc. */
  const flyTo = (p: P, ms: number) =>
    new Promise<void>((done) => {
      const s = b.current;
      s.mode = "manual";
      const from = { ...s.pos };
      const dist = Math.hypot(p.x - from.x, p.y - from.y);
      const ctrl = { x: (from.x + p.x) / 2, y: (from.y + p.y) / 2 - Math.min(dist * 0.2, 80) };
      const t0 = performance.now();
      const step = (now: number) => {
        const t = Math.min(1, (now - t0) / ms);
        const e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
        const u = 1 - e;
        s.pos = { x: u * u * from.x + 2 * u * e * ctrl.x + e * e * p.x, y: u * u * from.y + 2 * u * e * ctrl.y + e * e * p.y };
        if (t < 1) requestAnimationFrame(step);
        else done();
      };
      requestAnimationFrame(step);
    });

  /** The buddy draws a mark: it runs along the path while the stroke appears under it. */
  const trace = (m: Mark) =>
    new Promise<void>((done) => {
      const el = pathEls.current.get(m.id);
      const probe = measure.current;
      if (!el || !probe || !m.d) return done();
      probe.setAttribute("d", m.d);
      const len = probe.getTotalLength();
      const ms = Math.min(620, 220 + len * 0.6);
      const t0 = performance.now();
      const s = b.current;
      s.mode = "manual";
      const step = (now: number) => {
        const t = Math.min(1, (now - t0) / ms);
        const e = 1 - Math.pow(1 - t, 2);
        el.style.strokeDashoffset = `${1 - e}`;
        const pt = probe.getPointAtLength(len * e);
        s.pos = { x: pt.x, y: pt.y };
        if (t < 1) requestAnimationFrame(step);
        else done();
      };
      requestAnimationFrame(step);
    });

  /** Drag out a marquee from the top-left to the bottom-right of a target. */
  const marquee = (m: Mark) =>
    new Promise<void>((done) => {
      const r = { x: m.op.x - origin.current.x, y: m.op.y - origin.current.y, w: m.op.w ?? 40, h: m.op.h ?? 24 };
      const fill = document.getElementById(`sel-${m.id}`);
      const ants = document.getElementById(`ants-${m.id}`);
      const s = b.current;
      s.mode = "manual";
      const t0 = performance.now();
      const ms = 360;
      const step = (now: number) => {
        const t = Math.min(1, (now - t0) / ms);
        const e = 1 - Math.pow(1 - t, 3);
        const w = Math.max(1, (r.w + 12) * e), h = Math.max(1, (r.h + 12) * e);
        for (const el of [fill, ants]) {
          el?.setAttribute("width", `${w}`);
          el?.setAttribute("height", `${h}`);
        }
        s.pos = { x: r.x - 6 + w, y: r.y - 6 + h };
        if (t < 1) requestAnimationFrame(step);
        else done();
      };
      requestAnimationFrame(step);
    });

  const runQueue = async () => {
    if (busy.current) return;
    busy.current = true;
    while (queue.current.length) {
      const m = queue.current.shift()!;
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      if (m.op.kind === "select") {
        await flyTo({ x: m.op.x - origin.current.x - 6, y: m.op.y - origin.current.y - 6 }, 380);
        await marquee(m);
      } else if (m.d && measure.current) {
        measure.current.setAttribute("d", m.d);
        const start = measure.current.getPointAtLength(0);
        await flyTo({ x: start.x, y: start.y }, 260);
        await trace(m);
      } else {
        await flyTo({ x: m.op.x - origin.current.x, y: m.op.y - origin.current.y }, 300);
        const el = document.getElementById(`txt-${m.id}`);
        if (el) el.style.opacity = "1";
      }
    }
    b.current.mode = "hold";
    b.current.holdUntil = performance.now() + 1600;
    busy.current = false;
  };

  const enqueue = (ops: Op[]) => {
    const fresh: Mark[] = ops.map((op) => ({ id: ids.current++, op, d: opPath(op, origin.current.x, origin.current.y, ids.current) }));
    setMarks((m) => [...m, ...fresh]);
    setFading(false);
    queue.current.push(...fresh);
    // Let React mount the paths first.
    setTimeout(runQueue, 16);
  };

  useEffect(() => {
    const un = onEvent((e) => {
      const p = e.payload;
      switch (e.kind) {
        case "overlay.screen":
          setScreen(p.screen);
          setWin(p.window ?? p.screen.frame);
          origin.current = { x: (p.window ?? p.screen.frame).x, y: (p.window ?? p.screen.frame).y };
          break;
        case "buddy.cursor":
          setBuddyOn(true);
          b.current.cursor = { x: p.x, y: p.y };
          break;
        case "buddy.hide":
          setBuddyOn(false);
          break;
        case "buddy.fly":
          // Clicks and drags: go there, then come back.
          flyTo({ x: p.x - origin.current.x, y: p.y - origin.current.y }, p.slow ? 520 : 420).then(() => {
            b.current.mode = "hold";
            b.current.holdUntil = performance.now() + (p.click ? 700 : 1400);
          });
          break;
        case "buddy.click": {
          const id = ids.current++;
          setRipples((r) => [...r, { id, x: p.x, y: p.y }]);
          setTimeout(() => setRipples((r) => r.filter((x) => x.id !== id)), 480);
          break;
        }
        case "mic.start":
          setMood("listening");
          break;
        case "turn.start":
          setMood("thinking");
          break;
        case "speech.start":
          setMood("speaking");
          break;
        case "speech.end":
        case "turn.error":
        case "turn.empty":
          setTimeout(() => setMood((m) => (m === "listening" ? m : "idle")), 700);
          break;
        case "panel.state":
          if (p.mode === "hidden") setMood("idle");
          break;
        case "overlay.highlight":
          enqueue([{ kind: "select", x: p.rect.x, y: p.rect.y, w: p.rect.w, h: p.rect.h }]);
          break;
        case "overlay.draw":
          enqueue(p.ops as Op[]);
          break;
        case "overlay.clear":
          setFading(true);
          setTimeout(() => {
            setMarks([]);
            setFading(false);
            pathEls.current.clear();
          }, 420);
          if (b.current.mode === "hold") b.current.holdUntil = 0;
          break;
      }
    });
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="overlay">
      <div className={`overlay__marks ${fading ? "overlay--fading" : ""}`}>
        <svg className="overlay__ink" width="100%" height="100%">
          <path ref={measure} d="M0,0" fill="none" stroke="none" />
          {marks.map((m) => {
            if (m.op.kind === "select") {
              const x = m.op.x - ox - 6, y = m.op.y - oy - 6;
              return (
                <g key={m.id}>
                  <rect id={`sel-${m.id}`} x={x} y={y} width={1} height={1} className="sel__fill" />
                  <rect id={`ants-${m.id}`} x={x} y={y} width={1} height={1} className="sel__ants" />
                </g>
              );
            }
            if (m.op.kind === "text")
              return (
                <text key={m.id} id={`txt-${m.id}`} x={m.op.x - ox} y={m.op.y - oy} className="ink-text">
                  {m.op.text}
                </text>
              );
            return m.d ? (
              <path
                key={m.id}
                ref={(el) => {
                  if (el) pathEls.current.set(m.id, el);
                }}
                d={m.d}
                pathLength={1}
                className="ink"
              />
            ) : null;
          })}
        </svg>
      </div>
      {ripples.map((rp) => (
        <div key={rp.id} className="ripple-at" style={{ left: rp.x - ox, top: rp.y - oy }}>
          <i className="click-ripple" />
        </div>
      ))}
      <div ref={buddyEl} className={`buddy buddy--${mood}`} style={{ display: buddyOn ? "block" : "none" }}>
        <span className="buddy__ring" />
        <span className="buddy__face" style={{ ["--icon" as string]: `url(${slotUrl("icon", "brand-mark-white")})` }} />
      </div>
    </div>
  );
}
