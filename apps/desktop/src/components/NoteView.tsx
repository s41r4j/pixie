// Renders the note.md AST (crates/core/src/note.rs) as paper: answer line, tiles, stickies,
// questions, letters, reviews, charts… `onSay` sends an instruction back to the job.
import { useEffect, useMemo, useState } from "react";
import { api, type Attrs, type Block, type ListItem, type NoteDoc } from "../lib/bridge";
import { Inline, Link } from "../lib/inline";

type Say = (text: string) => void;

const STICKY: Record<string, string> = { yellow: "var(--paper-butter-light)", pink: "#FFD6E2", blue: "var(--paper-sky-light)", green: "var(--paper-mint-light)" };

function num(s: string | undefined): number {
  if (!s) return NaN;
  const m = s.replace(/,/g, "").match(/-?\d+(\.\d+)?/);
  return m ? parseFloat(m[0]) : NaN;
}

function Countdown({ to, label, note }: { to: string; label: string; note?: string }) {
  const target = useMemo(() => new Date(to.replace(" ", "T")).getTime(), [to]);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(t);
  }, []);
  const ms = target - now;
  const days = Math.floor(ms / 86_400_000);
  const hours = Math.floor((ms % 86_400_000) / 3_600_000);
  const value = isNaN(ms) ? to : ms <= 0 ? "now" : days > 0 ? `${days}d ${hours}h` : `${hours}h ${Math.floor((ms % 3_600_000) / 60_000)}m`;
  return <Tile label={label} value={value} note={note} />;
}

function Tile({ label, value, delta, note, good, children }: { label: string; value?: string; delta?: string; note?: string; good?: string; children?: React.ReactNode }) {
  const d = delta ? num(delta) : NaN;
  const better = good === "down" ? d < 0 : d > 0;
  return (
    <div className="tile">
      <div className="tile__label">{label}</div>
      {value !== undefined && <div className="tile__value">{value}</div>}
      {children}
      {delta && <div className={`tile__delta ${isNaN(d) ? "" : better ? "is-good" : "is-bad"}`}>{delta}</div>}
      {note && <div className="tile__note">{note}</div>}
    </div>
  );
}

function Spark({ values, goal }: { values: number[]; goal?: number }) {
  if (values.length < 2) return null;
  const all = goal !== undefined && !isNaN(goal) ? [...values, goal] : values;
  const min = Math.min(...all);
  const max = Math.max(...all);
  const y = (v: number) => 26 - ((v - min) / (max - min || 1)) * 22;
  const pts = values.map((v, i) => `${(i / (values.length - 1)) * 96 + 2},${y(v)}`).join(" ");
  return (
    <svg className="spark" viewBox="0 0 100 30" preserveAspectRatio="none">
      {goal !== undefined && !isNaN(goal) && <line x1="0" x2="100" y1={y(goal)} y2={y(goal)} className="spark__goal" />}
      <polyline points={pts} />
    </svg>
  );
}

function ObjectTile({ name, label, attrs }: { name: string; label: string; attrs: Attrs }) {
  switch (name) {
    case "figure":
      return <Tile label={label} value={attrs.value} delta={attrs.delta} note={attrs.note} good={attrs.good} />;
    case "status": {
      const s = attrs.state ?? "ok";
      return (
        <Tile label={label} note={attrs.note}>
          <div className={`status status--${s}`}>
            <i /> {s === "ok" ? "Fine" : s === "watch" ? "Watch" : s === "bad" ? "Problem" : "Off"}
          </div>
        </Tile>
      );
    }
    case "progress": {
      const v = attrs.value ?? "0";
      const frac = v.includes("/") ? num(v.split("/")[0]) / num(v.split("/")[1]) : v.includes("%") ? num(v) / 100 : num(v) > 1 ? num(v) / 100 : num(v);
      return (
        <Tile label={label} value={v} note={attrs.note}>
          <div className="bar"><span style={{ width: `${Math.max(0, Math.min(1, frac || 0)) * 100}%` }} /></div>
        </Tile>
      );
    }
    case "countdown":
      return <Countdown to={attrs.to ?? ""} label={label} note={attrs.note} />;
    case "spark": {
      const vals = (attrs.values ?? "").split(",").map(num).filter((n) => !isNaN(n));
      const raw = (attrs.values ?? "").split(",").map((s) => s.trim());
      return (
        <Tile label={label} value={`${raw[raw.length - 1] ?? ""}${attrs.unit ?? ""}`} note={attrs.note}>
          <Spark values={vals} goal={num(attrs.goal)} />
        </Tile>
      );
    }
    case "tally": {
      const v = Math.min(60, num(attrs.value) || 0);
      return (
        <Tile label={label} value={attrs.value} note={attrs.new ? `+${attrs.new} new` : attrs.note}>
          <div className="tally">{"|".repeat(Math.min(v, 30))}</div>
        </Tile>
      );
    }
    case "streak":
      return (
        <Tile label={label} note={attrs.note}>
          <div className="streak">
            {(attrs.days ?? "").split("").map((c, i) => (
              <i key={i} className={`streak__d streak__d--${c === "x" ? "done" : c === "." ? "miss" : c === "?" ? "today" : "rest"}`} />
            ))}
          </div>
        </Tile>
      );
    default:
      return <Tile label={label || name} value={attrs.value} note={attrs.note} />;
  }
}

const TILE_KINDS = new Set(["figure", "status", "progress", "countdown", "spark", "tally", "streak"]);

function Answer({ text, attrs }: { text: string; attrs: Attrs }) {
  return (
    <div className="answer">
      {attrs.big && <div className="answer__big">{attrs.big}</div>}
      <div className="answer__line"><Inline text={text} /></div>
      {attrs.note && <div className="answer__note hand">{attrs.note}</div>}
    </div>
  );
}

function Items({ items, ordered }: { items: ListItem[]; ordered: boolean }) {
  const Tag = ordered ? "ol" : "ul";
  return (
    <Tag className={`items ${items.some((i) => i.checked !== null) ? "items--boxes" : ""}`}>
      {items.map((it, i) => (
        <li key={i} className={`${it.checked ? "is-done" : ""} ${it.marks.includes("now") ? "is-now" : ""}`}>
          {it.checked !== null && <span className={`box ${it.checked ? "box--on" : ""}`} />}
          <span className="items__text">
            <Inline text={it.text} />
            {it.marks.includes("new") && <span className="mark-new">new</span>}
            {it.marks.includes("pick") && <span className="mark-pick">★ pick</span>}
            {it.detail.map((d, j) => (
              <span key={j} className="items__detail"><Inline text={d} /></span>
            ))}
          </span>
        </li>
      ))}
    </Tag>
  );
}

function Letter({ label, lines }: { label: string; lines: string[] }) {
  const text = lines.join("\n").trim();
  const [copied, setCopied] = useState(false);
  return (
    <div className="letter">
      <div className="letter__head">
        <span>{label || "Draft"}</span>
        <button
          className="chip"
          onClick={() => {
            navigator.clipboard.writeText(text);
            setCopied(true);
            api.playSound("ding").catch(() => {});
            setTimeout(() => setCopied(false), 1500);
          }}
        >
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <div className="letter__body">{text}</div>
    </div>
  );
}

function Review({ label, attrs, lines, onSay }: { label: string; attrs: Attrs; lines: string[]; onSay?: Say }) {
  const items = lines.filter((l) => /^\s*- \[[ xX]\]/.test(l)).map((l) => l.replace(/^\s*- \[[ xX]\]\s*/, ""));
  const [on, setOn] = useState<boolean[]>(() => items.map(() => true));
  return (
    <div className="review">
      <div className="review__head">{label}</div>
      {items.map((it, i) => (
        <label key={i} className="review__row">
          <input type="checkbox" checked={on[i]} onChange={() => setOn(on.map((v, j) => (j === i ? !v : v)))} />
          <Inline text={it} />
        </label>
      ))}
      {onSay && (
        <button className="btn btn--ink" onClick={() => onSay(`${attrs.go ?? "Go ahead"} — only these:\n${items.filter((_, i) => on[i]).map((t) => `- ${t}`).join("\n")}`)}>
          {attrs.go_label ?? "Go"}
        </button>
      )}
    </div>
  );
}

function Chart({ label, attrs, lines }: { label: string; attrs: Attrs; lines: string[] }) {
  const rows = lines
    .map((l) => l.split(":"))
    .filter((p) => p.length >= 2)
    .map(([k, ...v]) => ({ k: k.trim().replace(/^- /, ""), v: num(v.join(":")), raw: v.join(":").trim() }))
    .filter((r) => !isNaN(r.v));
  const max = Math.max(...rows.map((r) => r.v), 1);
  return (
    <figure className="chart">
      {label && <figcaption>{label}</figcaption>}
      {rows.map((r, i) => (
        <div key={i} className="chart__row">
          <span className="chart__k">{r.k}</span>
          <span className="chart__bar"><i style={{ width: `${(r.v / max) * 100}%` }} /></span>
          <span className="chart__v">{r.raw}{attrs.unit ?? ""}</span>
        </div>
      ))}
    </figure>
  );
}

function Pipeline({ label, lines }: { label: string; lines: string[] }) {
  const stages = lines.map((l) => l.split(":")).filter((p) => p.length >= 2);
  return (
    <div className="pipeline">
      {label && <div className="pipeline__label">{label}</div>}
      <div className="pipeline__row">
        {stages.map(([k, ...v], i) => (
          <div key={i} className="pipeline__stage">
            <b>{v.join(":").trim()}</b>
            <span>{k.trim()}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Form({ label, attrs, lines, onSay }: { label: string; attrs: Attrs; lines: string[]; onSay?: Say }) {
  const qs = lines.map((l) => l.replace(/^\s*[-*]\s*/, "").trim()).filter(Boolean);
  const [vals, setVals] = useState<string[]>(() => qs.map(() => ""));
  return (
    <div className="form">
      <div className="form__head">{label}</div>
      {qs.map((q, i) => (
        <label key={i} className="form__field">
          <span>{q.replace(/\{[^}]*\}$/, "")}</span>
          <input value={vals[i]} onChange={(e) => setVals(vals.map((v, j) => (j === i ? e.target.value : v)))} />
        </label>
      ))}
      {onSay && (
        <div className="form__actions">
          <button className="btn btn--ink" onClick={() => onSay(`${attrs.submit ?? "Here are my answers"}:\n${qs.map((q, i) => `- ${q}: ${vals[i] || "(use your best guess)"}`).join("\n")}`)}>
            {attrs.submit ?? "Send"}
          </button>
          <button className="btn btn--ghost-ink" onClick={() => onSay("Use your best guess for all of these.")}>Use your best guess</button>
        </div>
      )}
    </div>
  );
}

function Sketch({ attrs, lines }: { attrs: Attrs; lines: string[] }) {
  const h = num(attrs.h) || 160;
  const COLORS: Record<string, string> = { blue: "var(--draw-sky)", red: "var(--draw-coral)", green: "var(--draw-mint)", orange: "var(--draw-peach)", purple: "#A78BFA", pink: "var(--draw-pink)", yellow: "var(--draw-butter)", gray: "#9093A0", ink: "var(--ink)" };
  const shapes = lines.map((l, i) => {
    const label = /"([^"]*)"/.exec(l)?.[1];
    const words = l.replace(/"[^"]*"/, "").trim().split(/\s+/);
    const [kind, ...rest] = words;
    const n = rest.map(Number).filter((x) => !isNaN(x));
    const color = COLORS[rest.find((w) => COLORS[w]) ?? "ink"];
    const fill = rest.includes("fill") ? color : "none";
    const dash = rest.includes("dashed") ? "5 4" : undefined;
    const common = { stroke: color, strokeWidth: 2, fill, fillOpacity: 0.18, strokeDasharray: dash };
    switch (kind) {
      case "box":
        return (
          <g key={i}>
            <rect x={n[0]} y={n[1]} width={n[2]} height={n[3]} rx={8} {...common} />
            {label && <text x={n[0] + n[2] / 2} y={n[1] + n[3] / 2 + 5} textAnchor="middle">{label}</text>}
          </g>
        );
      case "circle":
        return (
          <g key={i}>
            <circle cx={n[0]} cy={n[1]} r={n[2]} {...common} />
            {label && <text x={n[0]} y={n[1] + 5} textAnchor="middle">{label}</text>}
          </g>
        );
      case "arrow":
      case "line": {
        const curve = rest.includes("curve");
        const d = curve ? `M${n[0]},${n[1]} Q${(n[0] + n[2]) / 2},${Math.min(n[1], n[3]) - 30} ${n[2]},${n[3]}` : `M${n[0]},${n[1]} L${n[2]},${n[3]}`;
        return (
          <g key={i}>
            <path d={d} stroke={color} strokeWidth={2} fill="none" strokeDasharray={dash} markerEnd={kind === "arrow" ? "url(#ah)" : undefined} />
            {label && <text x={(n[0] + n[2]) / 2} y={(n[1] + n[3]) / 2 - 8} textAnchor="middle">{label}</text>}
          </g>
        );
      }
      case "dot":
      case "num":
        return (
          <g key={i}>
            <circle cx={n[0]} cy={n[1]} r={kind === "num" ? 11 : 4} fill={color} />
            {label && <text x={n[0] + (kind === "num" ? 0 : 10)} y={n[1] + 5} textAnchor={kind === "num" ? "middle" : "start"} className={kind === "num" ? "sketch__num" : ""}>{label}</text>}
          </g>
        );
      case "text":
        return <text key={i} x={n[0]} y={n[1]}>{label}</text>;
      default:
        return null;
    }
  });
  return (
    <svg className="sketch" viewBox={`0 0 420 ${h}`} style={{ maxHeight: h + 20 }}>
      <defs>
        <marker id="ah" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0,0 L10,5 L0,10 z" fill="var(--ink)" />
        </marker>
      </defs>
      {shapes}
    </svg>
  );
}

function ObjectBlock({ name, label, attrs, onSay }: { name: string; label: string; attrs: Attrs; onSay?: Say }) {
  if (TILE_KINDS.has(name)) return <div className="tiles"><ObjectTile name={name} label={label} attrs={attrs} /></div>;
  switch (name) {
    case "sticky":
      return <div className="sticky hand" style={{ background: STICKY[attrs.color ?? "yellow"] ?? STICKY.yellow }}>{label}</div>;
    case "note":
      return <div className="margin hand">{label}</div>;
    case "ask":
      return (
        <button className="ask" disabled={!onSay} onClick={() => onSay?.(attrs.say ?? label)}>
          {label} <span>→</span>
        </button>
      );
    case "question": {
      const yes = attrs.yes ?? "Yes";
      const no = attrs.no ?? "No";
      return (
        <div className="question">
          <div className="question__q">{label}</div>
          {onSay && (
            <div className="question__a">
              <button className="btn btn--ink" onClick={() => onSay(yes)}>{attrs.yes_label ?? "Yes"}</button>
              <button className="btn btn--ghost-ink" onClick={() => onSay(no)}>{attrs.no_label ?? "No"}</button>
            </div>
          )}
        </div>
      );
    }
    case "link":
      return (
        <div className="linkrow">
          <Link href={attrs.url ?? "#"}>{label || attrs.url}</Link>
          <span>{(attrs.url ?? "").replace(/^https?:\/\/(www\.)?/, "").split("/")[0]}</span>
        </div>
      );
    case "file":
      return (
        <button className="filerow" onClick={() => attrs.path && api.openTarget(attrs.path)}>
          <span className="filerow__icon">▤</span> {label || attrs.path}
        </button>
      );
    case "quiet":
      return <div className="quiet">{label}{attrs.checked && <span> · checked {attrs.checked}</span>}</div>;
    case "run":
      return <div className="runline">Run {attrs.n} · {attrs.at}{attrs.next && ` · next ${attrs.next}`}{attrs.note && ` · ${attrs.note}`}</div>;
    case "connect":
      return (
        <button className="ask" onClick={() => api.showSettings("connectors")}>
          Connect {label} <span>→</span>
        </button>
      );
    case "event":
      return <div className="eventrow"><b>{label}</b><span>{attrs.when}{attrs.where && ` · ${attrs.where}`}</span></div>;
    case "place":
      return <div className="eventrow"><b>{label}</b><span>{[attrs.note, attrs.rating && `★ ${attrs.rating}`, attrs.hours].filter(Boolean).join(" · ")}</span></div>;
    case "ticket":
      return <div className="eventrow"><b>{label}</b><span>{[attrs.from, attrs.to && `→ ${attrs.to}`, attrs.when, attrs.price].filter(Boolean).join(" · ")}</span></div>;
    case "photo":
      return <div className="eventrow"><b>{label}</b><span>photo: {attrs.query ?? attrs.path}</span></div>;
    default:
      return <div className="eventrow"><b>{label || name}</b><span>{Object.entries(attrs).map(([k, v]) => `${k}: ${v}`).join(" · ")}</span></div>;
  }
}

export function Blocks({ blocks, onSay }: { blocks: Block[]; onSay?: Say }) {
  return (
    <>
      {blocks.map((b, i) => {
        switch (b.type) {
          case "answer":
            return <Answer key={i} text={b.text} attrs={b.attrs} />;
          case "quote":
            return <blockquote key={i}><Inline text={b.text} /></blockquote>;
          case "callout":
            return <div key={i} className={`callout callout--${b.kind}`}><Inline text={b.text} /></div>;
          case "heading":
            return <h4 key={i}><Inline text={b.text} /></h4>;
          case "object":
            return <ObjectBlock key={i} {...b} onSay={onSay} />;
          case "container": {
            switch (b.name) {
              case "tiles":
                return (
                  <div key={i} className="tiles">
                    {b.children.map((c, j) => (c.type === "object" ? <ObjectTile key={j} {...c} /> : null))}
                  </div>
                );
              case "letter":
                return <Letter key={i} label={b.label} lines={b.lines} />;
              case "review":
                return <Review key={i} label={b.label} attrs={b.attrs} lines={b.lines} onSay={onSay} />;
              case "chart":
                return <Chart key={i} label={b.label} attrs={b.attrs} lines={b.lines} />;
              case "pipeline":
                return <Pipeline key={i} label={b.label} lines={b.lines} />;
              case "form":
                return <Form key={i} label={b.label} attrs={b.attrs} lines={b.lines} onSay={onSay} />;
              case "sketch":
                return <Sketch key={i} attrs={b.attrs} lines={b.lines} />;
              case "receipt":
                return (
                  <div key={i} className="receipt">
                    <div className="receipt__head">{b.label}</div>
                    {b.lines.map((l, j) => {
                      const [k, ...v] = l.replace(/^\s*-\s*/, "").split(":");
                      return <div key={j} className="receipt__row"><span>{k}</span><span>{v.join(":")}</span></div>;
                    })}
                    {b.attrs.total && <div className="receipt__row receipt__total"><span>Total</span><span>{b.attrs.total}</span></div>}
                  </div>
                );
              default:
                return (
                  <div key={i} className={`box-block box-block--${b.name}`}>
                    {b.label && <div className="box-block__label">{b.label}</div>}
                    <Blocks blocks={b.children} onSay={onSay} />
                  </div>
                );
            }
          }
          case "list":
            return <Items key={i} items={b.items} ordered={b.ordered} />;
          case "table":
            return (
              <div key={i} className="table-wrap">
                <table>
                  <thead><tr>{b.header.map((h, j) => <th key={j}><Inline text={h} /></th>)}</tr></thead>
                  <tbody>{b.rows.map((r, j) => <tr key={j}>{r.map((c, k) => <td key={k}><Inline text={c} /></td>)}</tr>)}</tbody>
                </table>
              </div>
            );
          case "code":
            return <pre key={i}><code>{b.text}</code></pre>;
          case "paragraph":
            return <p key={i}><Inline text={b.text} /></p>;
          case "rule":
            return <hr key={i} />;
          default:
            return null;
        }
      })}
    </>
  );
}

/** A whole note on paper. `compact` shows the title and first page only. */
export function NoteView({ doc, onSay, compact = false }: { doc: NoteDoc; onSay?: Say; compact?: boolean }) {
  const pages = compact ? doc.pages.slice(0, 1) : doc.pages;
  return (
    <article className={`paper ${compact ? "paper--compact" : ""}`}>
      {doc.title && <h2 className="paper__title">{doc.title}</h2>}
      {pages.map((p, i) => (
        <section key={i} className="paper__page">
          {p.name && (i > 0 || !compact) && <h3 className="paper__page-name">{p.name}</h3>}
          <Blocks blocks={p.blocks} onSay={onSay} />
        </section>
      ))}
      {compact && doc.pages.length > 1 && <div className="paper__more">+{doc.pages.length - 1} more page{doc.pages.length > 2 ? "s" : ""}</div>}
    </article>
  );
}
