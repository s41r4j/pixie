// The board: a clean paper canvas for explaining things visually. Shapes are drawn one after
// another by hand. Text is laid out before drawing: labels wrap and shrink to fit inside their
// box, free text and arrow labels are nudged so nothing overlaps, and the view fits the
// drawing. A click anywhere (or esc) puts it away, like the marks on screen.
import { useEffect, useMemo, useState } from "react";
import { api, onEvent } from "../lib/bridge";
import { COLOR, type Op } from "./Overlay";

const STEP = 380;
const W = 1000;
const H = 640;
/** VT323 is narrow: about half an em per character. */
const CHAR = 0.5;

type Box = { x: number; y: number; w: number; h: number };
type Lines = { lines: string[]; size: number; x: number; y: number; anchor: "middle" | "start" };
type Laid = { op: Op; w: number; h: number; label: Lines | null; box: Box };

function wrap(text: string, maxW: number, size: number): string[] {
  const per = Math.max(4, Math.floor(maxW / (size * CHAR)));
  const out: string[] = [];
  let cur = "";
  for (const word of text.split(/\s+/).filter(Boolean)) {
    if (word.length > per) {
      if (cur) out.push(cur);
      for (let i = 0; i < word.length; i += per) out.push(word.slice(i, i + per));
      cur = "";
      continue;
    }
    if (!cur) cur = word;
    else if ((cur + " " + word).length <= per) cur += " " + word;
    else {
      out.push(cur);
      cur = word;
    }
  }
  if (cur) out.push(cur);
  return out;
}

const overlap = (a: Box, b: Box, pad = 6) => a.x < b.x + b.w + pad && b.x < a.x + a.w + pad && a.y < b.y + b.h + pad && b.y < a.y + a.h + pad;

/** Fit every label, then move free text and arrow labels off anything they'd cover. */
function layout(ops: Op[]): { items: Laid[]; view: Box } {
  const items: Laid[] = [];
  const taken: Box[] = [];
  for (const op of ops) {
    let w = op.w ?? 160;
    let h = op.h ?? 70;
    let label: Lines | null = null;
    let box: Box = { x: op.x, y: op.y, w, h };
    if ((op.kind === "box" || op.kind === "circle") && op.text) {
      // Inside: shrink from 26 to 16 px; if it still doesn't fit, grow the shape.
      const inner = op.kind === "circle" ? 0.72 : 0.9;
      let size = 26;
      let lines = wrap(op.text, w * inner - 12, size);
      while (size > 16 && lines.length * size * 0.95 > h * inner - 8) {
        size -= 2;
        lines = wrap(op.text, w * inner - 12, size);
      }
      const need = lines.length * size * 0.95 + 16;
      if (need > h * inner) h = Math.ceil(need / inner);
      const longest = Math.max(...lines.map((l) => l.length)) * size * CHAR + 20;
      if (longest > w * inner) w = Math.ceil(longest / inner);
      label = { lines, size, x: op.x + w / 2, y: op.y + h / 2 - ((lines.length - 1) * size * 0.95) / 2 + size * 0.32, anchor: "middle" };
      box = { x: op.x, y: op.y, w, h };
    } else if (op.kind === "text" && op.text) {
      const size = 28;
      const lines = wrap(op.text, Math.min(440, W - op.x - 20), size);
      const tw = Math.max(...lines.map((l) => l.length)) * size * CHAR;
      const th = lines.length * size;
      let y = op.y;
      let guard = 0;
      while (taken.some((t) => overlap(t, { x: op.x, y: y - size * 0.8, w: tw, h: th })) && guard++ < 14) y += 22;
      label = { lines, size, x: op.x, y, anchor: "start" };
      box = { x: op.x, y: y - size * 0.8, w: tw, h: th };
    } else if ((op.kind === "arrow" || op.kind === "line") && op.text) {
      const tx = op.to_x ?? op.x + 140;
      const ty = op.to_y ?? op.y;
      const size = 22;
      const lines = wrap(op.text, Math.max(120, Math.hypot(tx - op.x, ty - op.y) - 10), size);
      const tw = Math.max(...lines.map((l) => l.length)) * size * CHAR;
      const th = lines.length * size;
      // Start just above the middle of the line; step outward until it's clear.
      const mx = (op.x + tx) / 2;
      let my = (op.y + ty) / 2 - 12 - (lines.length - 1) * size;
      let guard = 0;
      const at = (yy: number): Box => ({ x: mx - tw / 2, y: yy - size * 0.8, w: tw, h: th });
      while (taken.some((t) => overlap(t, at(my), 2)) && guard++ < 10) my -= 18;
      label = { lines, size, x: mx, y: my, anchor: "middle" };
      box = { x: Math.min(op.x, tx), y: Math.min(op.y, ty), w: Math.abs(tx - op.x), h: Math.abs(ty - op.y) };
      taken.push(at(my));
    } else if (op.kind === "arrow" || op.kind === "line") {
      const tx = op.to_x ?? op.x + 140;
      const ty = op.to_y ?? op.y;
      box = { x: Math.min(op.x, tx), y: Math.min(op.y, ty), w: Math.abs(tx - op.x), h: Math.abs(ty - op.y) };
    }
    if (op.kind !== "arrow" && op.kind !== "line") taken.push(box);
    items.push({ op, w, h, label, box });
  }
  // Fit the view to the drawing (never smaller than the canvas), with a margin.
  const all = items.map((i) => i.box).concat(taken);
  const x0 = Math.min(0, ...all.map((b) => b.x - 20));
  const y0 = Math.min(0, ...all.map((b) => b.y - 30));
  const x1 = Math.max(W, ...all.map((b) => b.x + b.w + 20));
  const y1 = Math.max(H, ...all.map((b) => b.y + b.h + 20));
  return { items, view: { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } };
}

function Label({ l, delay, cls }: { l: Lines; delay: number; cls: string }) {
  return (
    <text x={l.x} y={l.y} textAnchor={l.anchor} className={cls} style={{ fontSize: l.size, animationDelay: `${delay}ms` }}>
      {l.lines.map((line, i) => (
        <tspan key={i} x={l.x} dy={i === 0 ? 0 : l.size * 0.95}>
          {line}
        </tspan>
      ))}
    </text>
  );
}

function Shape({ item, i }: { item: Laid; i: number }) {
  const { op, w, h, label } = item;
  const delay = i * STEP;
  const color = COLOR[op.color ?? "ink"] ?? COLOR.ink;
  const ink = { stroke: color, fill: "none", strokeWidth: 3, strokeLinecap: "square" as const, strokeLinejoin: "miter" as const, shapeRendering: "crispEdges" as const, pathLength: 1, className: "board__ink", style: { animationDelay: `${delay}ms` } };
  const lab = label ? <Label l={label} delay={delay + 220} cls={op.kind === "text" ? "board__text" : "board__label"} /> : null;
  const { x, y } = op;
  switch (op.kind) {
    case "box":
      return (
        <g>
          <rect x={x} y={y} width={w} height={h} {...ink} fill="#fff" fillOpacity={0.92} />
          {lab}
        </g>
      );
    case "circle":
      return (
        <g>
          <ellipse cx={x + w / 2} cy={y + h / 2} rx={w / 2} ry={h / 2} {...ink} fill="#fff" fillOpacity={0.92} />
          {lab}
        </g>
      );
    case "underline":
      return <path d={`M${x},${y + h} Q${x + w / 2},${y + h + 8} ${x + w},${y + h}`} {...ink} />;
    case "line":
    case "arrow": {
      const tx = op.to_x ?? x + 140;
      const ty = op.to_y ?? y;
      const ang = Math.atan2(ty - y, tx - x);
      const head =
        op.kind === "arrow"
          ? ` M${tx - 16 * Math.cos(ang - 0.45)},${ty - 16 * Math.sin(ang - 0.45)} L${tx},${ty} L${tx - 16 * Math.cos(ang + 0.45)},${ty - 16 * Math.sin(ang + 0.45)}`
          : "";
      return (
        <g>
          <path d={`M${x},${y} L${tx},${ty}${head}`} {...ink} />
          {lab}
        </g>
      );
    }
    case "check":
      return <path d={`M${x},${y + 14} L${x + 12},${y + 28} L${x + 36},${y - 6}`} {...ink} strokeWidth={4} />;
    case "cross":
      return <path d={`M${x},${y} L${x + 26},${y + 26} M${x + 26},${y} L${x},${y + 26}`} {...ink} strokeWidth={4} />;
    case "text":
      return lab;
    default:
      return null;
  }
}

export function Board() {
  const [title, setTitle] = useState("");
  const [ops, setOps] = useState<Op[]>([]);
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState(0);

  useEffect(() => {
    const un = onEvent((e) => {
      if (e.kind === "board.show") {
        setTitle(e.payload.title ?? "");
        setOps(e.payload.ops ?? []);
        setKey((k) => k + 1);
        setOpen(true);
      }
      if (e.kind === "board.hide") setOpen(false);
    });
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && api.boardClose();
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  // Text and shapes are drawn first, labels last, so a label is never under a box.
  const { items, view } = useMemo(() => layout(ops), [ops]);
  const order = [...items.keys()].sort((a, b) => Number(items[a].op.kind === "text") - Number(items[b].op.kind === "text"));

  return (
    <div className={`board ${open ? "board--open" : ""}`} onMouseDown={() => api.boardClose()} title="Click anywhere to put it away">
      <div className="board__sheet">
        <header className="board__head">
          <span className="board__title">{title}</span>
          <span className="board__hint">click to close</span>
        </header>
        <svg key={key} className="board__canvas" viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`} preserveAspectRatio="xMidYMid meet">
          {order.map((i) => (
            <Shape key={i} item={items[i]} i={i} />
          ))}
        </svg>
      </div>
    </div>
  );
}
