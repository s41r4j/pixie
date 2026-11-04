// Full-screen, click-through layer. `highlight` flies a gel pointer out of the notch to the
// target and rings it; `draw` puts hand-drawn marks on screen. Everything fades on its own.
import { useEffect, useRef, useState } from "react";
import { onEvent, type Rect, type ScreenInfo } from "../lib/bridge";

interface Op {
  kind: "circle" | "underline" | "arrow" | "box" | "check" | "cross" | "text";
  x: number;
  y: number;
  w?: number;
  h?: number;
  to_x?: number;
  to_y?: number;
  text?: string;
  color?: string;
}
interface Mark {
  id: number;
  op: Op;
  born: number;
}
interface Target {
  id: number;
  rect: Rect;
  label: string;
}

const COLOR: Record<string, string> = {
  sky: "var(--draw-sky)",
  coral: "var(--draw-coral)",
  butter: "var(--draw-butter)",
  mint: "var(--draw-mint)",
  peach: "var(--draw-peach)",
  pink: "var(--draw-pink)",
  ink: "var(--draw-ink)",
};

/** Slightly wobbly hand-drawn ellipse around a box. */
function ellipsePath(x: number, y: number, w: number, h: number, seed: number) {
  const cx = x + w / 2;
  const cy = y + h / 2;
  const rx = w / 2 + 12;
  const ry = h / 2 + 10;
  const pts: string[] = [];
  const steps = 36;
  for (let i = 0; i <= steps + 3; i++) {
    const t = (i / steps) * Math.PI * 2 - 0.5;
    const wob = 1 + Math.sin(t * 3 + seed) * 0.035;
    pts.push(`${(cx + Math.cos(t) * rx * wob).toFixed(1)},${(cy + Math.sin(t) * ry * wob).toFixed(1)}`);
  }
  return `M${pts.join(" L")}`;
}

function OpShape({ op, ox, oy, seed }: { op: Op; ox: number; oy: number; seed: number }) {
  const x = op.x - ox;
  const y = op.y - oy;
  const w = op.w ?? 80;
  const h = op.h ?? 30;
  const color = COLOR[op.color ?? "coral"] ?? COLOR.coral;
  const common = { stroke: color, fill: "none", strokeWidth: 4, strokeLinecap: "round" as const, strokeLinejoin: "round" as const, pathLength: 1, className: "ink" };
  switch (op.kind) {
    case "circle":
      return <path d={ellipsePath(x, y, w, h, seed)} {...common} />;
    case "box":
      return <path d={`M${x - 8},${y - 6} L${x + w + 9},${y - 8} L${x + w + 7},${y + h + 7} L${x - 7},${y + h + 8} Z`} {...common} />;
    case "underline": {
      const yy = y + h + 6;
      let d = `M${x},${yy}`;
      for (let i = 1; i <= 8; i++) d += ` Q${x + (w * (i - 0.5)) / 8},${yy + (i % 2 ? 4 : -4)} ${x + (w * i) / 8},${yy}`;
      return <path d={d} {...common} />;
    }
    case "arrow": {
      const tx = (op.to_x ?? op.x + 120) - ox;
      const ty = (op.to_y ?? op.y) - oy;
      const mx = (x + tx) / 2 + (ty - y) * 0.2;
      const my = (y + ty) / 2 - (tx - x) * 0.2;
      const ang = Math.atan2(ty - my, tx - mx);
      const a1 = `${tx - 18 * Math.cos(ang - 0.45)},${ty - 18 * Math.sin(ang - 0.45)}`;
      const a2 = `${tx - 18 * Math.cos(ang + 0.45)},${ty - 18 * Math.sin(ang + 0.45)}`;
      return <path d={`M${x},${y} Q${mx},${my} ${tx},${ty} M${a1} L${tx},${ty} L${a2}`} {...common} />;
    }
    case "check":
      return <path d={`M${x},${y + 14} L${x + 12},${y + 28} L${x + 36},${y - 6}`} {...common} strokeWidth={5} />;
    case "cross":
      return <path d={`M${x},${y} L${x + 28},${y + 28} M${x + 28},${y} L${x},${y + 28}`} {...common} strokeWidth={5} />;
    case "text":
      return (
        <text x={x} y={y} className="ink-text hand" fill={color}>
          {op.text}
        </text>
      );
  }
}

export function Overlay() {
  const [screen, setScreen] = useState<ScreenInfo | null>(null);
  const [notchOrigin, setNotchOrigin] = useState<Rect | null>(null);
  const [marks, setMarks] = useState<Mark[]>([]);
  const [target, setTarget] = useState<Target | null>(null);
  const [flying, setFlying] = useState(false);
  const ids = useRef(1);

  useEffect(() => {
    const un = onEvent((e) => {
      const p = e.payload;
      switch (e.kind) {
        case "overlay.screen":
          setScreen(p.screen);
          setNotchOrigin(p.origin);
          break;
        case "overlay.highlight": {
          const id = ids.current++;
          setFlying(false);
          setTarget({ id, rect: p.rect, label: p.label ?? "" });
          setTimeout(() => setFlying(true), 34);
          setTimeout(() => setTarget((t) => (t?.id === id ? null : t)), 7000);
          break;
        }
        case "overlay.draw": {
          const now = Date.now();
          const fresh: Mark[] = (p.ops as Op[]).map((op) => ({ id: ids.current++, op, born: now }));
          setMarks((m) => [...m, ...fresh]);
          setTimeout(() => setMarks((m) => m.filter((x) => !fresh.includes(x))), 8000);
          break;
        }
        case "overlay.clear":
          setMarks([]);
          setTarget(null);
          break;
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const ox = screen?.frame.x ?? 0;
  const oy = screen?.frame.y ?? 0;
  const W = screen?.frame.w ?? window.innerWidth;
  // The pointer starts at the notch (top centre of the screen the panel is on, if it's this one).
  const startX = notchOrigin && notchOrigin.x === ox ? notchOrigin.w / 2 : W / 2;
  const startY = (screen?.top_inset ?? 30) / 2;

  const r = target?.rect;
  // The tip rests just inside the lower right of the target, so the target stays readable.
  const tx = r ? r.x - ox + Math.min(r.w * 0.72, r.w - 6) : startX;
  const ty = r ? r.y - oy + Math.min(r.h * 0.78, r.h - 4) : startY;

  return (
    <div className="overlay">
      <svg className="overlay__ink" width="100%" height="100%">
        {marks.map((m) => (
          <g key={m.id} className="mark-g">
            <OpShape op={m.op} ox={ox} oy={oy} seed={m.id} />
          </g>
        ))}
      </svg>

      {target && r && (
        <>
          <div
            className={`ring ${flying ? "ring--on" : ""}`}
            style={{ left: r.x - ox - 8, top: r.y - oy - 8, width: r.w + 16, height: r.h + 16 }}
          />
          {target.label && (
            <div
              className={`tag hand ${flying ? "tag--on" : ""}`}
              style={{
                left: Math.min(r.x - ox + r.w + 18, W - 220),
                top: r.y - oy + r.h / 2 - 16,
              }}
            >
              {target.label}
            </div>
          )}
          <div
            className={`pointer ${flying ? "pointer--on" : ""}`}
            style={{ transform: `translate(${flying ? tx : startX}px, ${flying ? ty : startY}px)` }}
          >
            <svg width="34" height="34" viewBox="0 0 34 34">
              <defs>
                <radialGradient id="gel" cx="35%" cy="30%" r="80%">
                  <stop offset="0" stopColor="#fff" />
                  <stop offset="0.25" stopColor="var(--focus)" />
                  <stop offset="0.6" stopColor="var(--draw-sky)" />
                  <stop offset="1" stopColor="var(--gel-rim)" />
                </radialGradient>
              </defs>
              <path d="M3 2 L29 14 L17 17 L12 30 Z" fill="url(#gel)" stroke="#fff" strokeWidth="2" strokeLinejoin="round" />
            </svg>
          </div>
        </>
      )}
    </div>
  );
}
