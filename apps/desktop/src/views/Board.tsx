// The board: a clean paper canvas for explaining things visually. Shapes are drawn one after
// another by hand; it stays until closed (×, esc) or the next question.
import { useEffect, useState } from "react";
import { api, onEvent } from "../lib/bridge";
import { COLOR, type Op } from "./Overlay";

const STEP = 380;

function Shape({ op, i }: { op: Op; i: number }) {
  const delay = i * STEP;
  const color = COLOR[op.color ?? "ink"] ?? COLOR.ink;
  const ink = { stroke: color, fill: "none", strokeWidth: 3, strokeLinecap: "square" as const, strokeLinejoin: "miter" as const, shapeRendering: "crispEdges" as const, pathLength: 1, className: "ink", style: { animationDelay: `${delay}ms` } };
  const label = (x: number, y: number, anchor: "middle" | "start" = "middle") =>
    op.text ? (
      <text x={x} y={y} textAnchor={anchor} className="board__label" style={{ animationDelay: `${delay + 220}ms` }}>
        {op.text}
      </text>
    ) : null;
  const { x, y } = op;
  const w = op.w ?? 160;
  const h = op.h ?? 70;
  switch (op.kind) {
    case "box":
      return (
        <g>
          <rect x={x} y={y} width={w} height={h} {...ink} />
          {label(x + w / 2, y + h / 2 + 7)}
        </g>
      );
    case "circle":
      return (
        <g>
          <ellipse cx={x + w / 2} cy={y + h / 2} rx={w / 2} ry={h / 2} {...ink} />
          {label(x + w / 2, y + h / 2 + 7)}
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
          {label((x + tx) / 2, (y + ty) / 2 - 10)}
        </g>
      );
    }
    case "check":
      return <path d={`M${x},${y + 14} L${x + 12},${y + 28} L${x + 36},${y - 6}`} {...ink} strokeWidth={4} />;
    case "cross":
      return <path d={`M${x},${y} L${x + 26},${y + 26} M${x + 26},${y} L${x},${y + 26}`} {...ink} strokeWidth={4} />;
    case "text":
      return (
        <text x={x} y={y} className="board__text" fill={color} style={{ animationDelay: `${delay}ms` }}>
          {op.text}
        </text>
      );
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

  return (
    <div className={`board ${open ? "board--open" : ""}`}>
      <div className="board__sheet">
        <header className="board__head">
          <span className="board__title">{title}</span>
          <button className="board__close" onClick={() => api.boardClose()} aria-label="Close">
            ×
          </button>
        </header>
        <svg key={key} className="board__canvas" viewBox="0 0 1000 640" preserveAspectRatio="xMidYMid meet">
          {ops.map((op, i) => (
            <Shape key={i} op={op} i={i} />
          ))}
        </svg>
      </div>
    </div>
  );
}
