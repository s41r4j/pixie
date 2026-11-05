// Settings → Models, Usage, Coding.
import { useEffect, useState } from "react";
import { api, type Settings, type SettingsView, type UsageSummary } from "../lib/bridge";
import { Glyph } from "../components/Mark";

type Save = (patch: Partial<Settings>) => Promise<void>;

const PROVIDERS = [
  { id: "claude", name: "Claude", by: "Anthropic", keyField: "anthropic_key" as const, placeholder: "sk-ant-…", hint: "Used first when its key is set.", keys: "console.anthropic.com" },
  { id: "openai", name: "OpenAI", by: "OpenAI", keyField: "openai_key" as const, placeholder: "sk-…", hint: "GPT models through Chat Completions.", keys: "platform.openai.com" },
  { id: "deepseek", name: "DeepSeek", by: "DeepSeek", keyField: "deepseek_key" as const, placeholder: "sk-…", hint: "Fast and cheap; reads screenshots.", keys: "platform.deepseek.com" },
];

type Model = { id: string; name: string; vision: boolean | null; created: number | null; price: { input: number; output: number } | null; context: number | null };

function money(n: number) {
  return String(parseFloat(n.toFixed(n >= 1 ? 2 : 3)));
}
function ctx(n: number | null) {
  if (!n) return "";
  return n >= 1e6 ? `${(n / 1e6).toFixed(n % 1e6 ? 1 : 0)}M` : `${Math.round(n / 1e3)}k`;
}

function ProviderPane({ p, view, save, active }: { p: (typeof PROVIDERS)[number]; view: SettingsView; save: Save; active: boolean }) {
  const s = view.settings;
  const [key, setKey] = useState(s[p.keyField]);
  const [models, setModels] = useState<Model[]>([]);
  const [state, setState] = useState<"loading" | "ok" | "nokey" | "error">("loading");
  const [err, setErr] = useState("");
  const [test, setTest] = useState("");
  const [filter, setFilter] = useState("");
  const [all, setAll] = useState(false);
  const choice = s.models?.[p.id] ?? { fast: "", deep: "" };
  const eff = view.effective?.[p.id] ?? choice;

  const load = () => {
    setState("loading");
    api
      .modelsAvailable(p.id)
      .then((r) => {
        setModels(r.models as Model[]);
        setState("ok");
      })
      .catch((e) => {
        const m = String(e);
        setState(/No key/i.test(m) ? "nokey" : "error");
        setErr(m);
      });
  };
  useEffect(() => {
    setKey(s[p.keyField]);
    load();
  }, [p.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const setChoice = (k: "fast" | "deep", v: string) => save({ models: { ...(s.models ?? {}), [p.id]: { ...choice, [k]: choice[k] === v ? "" : v } } });
  const fb = `${p.id}:`;
  const setFallback = (id: string) => save({ fallback: s.fallback === fb + id ? "" : fb + id });
  const sameName = (m: Model) => !m.name || m.name.toLowerCase().replace(/[\s._]/g, "-") === m.id.toLowerCase();
  const shown = models.filter((m) => !filter || `${m.id} ${m.name}`.toLowerCase().includes(filter.toLowerCase()));
  const used = (m: Model) => [choice.fast || eff.fast, choice.deep || eff.deep].includes(m.id) || s.fallback === `${p.id}:${m.id}`;
  const ordered = [...shown.filter(used), ...shown.filter((m) => !used(m))];
  const list = all || filter ? ordered : ordered.slice(0, 10);
  const newest = models.find((m) => m.created)?.id;

  return (
    <div className="ppane">
      <div className="ppane__key">
        <label className="field">
          <span>
            {p.name} API key <span className="muted">· from {p.keys}</span>
          </span>
          <input
            type="password"
            placeholder={p.placeholder}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onBlur={() => key !== s[p.keyField] && save({ [p.keyField]: key } as Partial<Settings>).then(load)}
          />
        </label>
        <div className="row-actions">
          <button data-tip={active ? `${p.name} is answering now` : `Make ${p.name} answer from now on`} className={`btn ${active ? "btn--primary" : ""}`} disabled={state !== "ok" || active} onClick={() => save({ provider: p.id })}>
            {active ? "■ In use" : `Use ${p.name}`}
          </button>
          {active && (
            <button
              className="btn btn--ghost"
              onClick={() => {
                setTest("testing…");
                api.providerTest().then(setTest).catch((e) => setTest(String(e)));
              }}
            >
              Test
            </button>
          )}
          <button className="btn btn--ghost" onClick={load} title="Fetch the newest list">
            Refresh
          </button>
          {test && <span className="muted small">{test}</span>}
        </div>
      </div>

      {state === "loading" && <div className="mlist__empty">Fetching {p.name}'s models…</div>}
      {state === "nokey" && <div className="mlist__empty">Paste a key above to see {p.name}'s models.</div>}
      {state === "error" && <div className="mlist__empty bad-text">{err}</div>}
      {state === "ok" && (
        <>
          <div className="mlist__bar">
            <input className="inline-input mlist__filter" placeholder={`Filter ${models.length} models`} value={filter} onChange={(e) => setFilter(e.target.value)} />
            <span className="mlist__legend">
              <span className="tagbox tagbox--on">T</span> talk <span className="tagbox tagbox--on">A</span> agents <span className="tagbox tagbox--on">F</span> fallback
            </span>
          </div>
          <ul className="mlist">
            {list.map((m) => {
              const isFast = (choice.fast || eff.fast) === m.id;
              const isDeep = (choice.deep || eff.deep) === m.id;
              const isFb = s.fallback === fb + m.id;
              return (
                <li key={m.id} className={`mrow ${isFast || isDeep || isFb ? "mrow--on" : ""}`}>
                  <div className="mrow__id">
                    <b>
                      {m.id}
                      {!sameName(m) && <span className="mrow__name"> · {m.name}</span>}
                    </b>
                    <span className="mrow__tags">
                      {active && (isFast || isDeep) && <span className="tag tag--use">■ in use · {isFast && isDeep ? "talk + agents" : isFast ? "talk" : "agents"}</span>}
                      {!active && (isFast || isDeep) && <span className="tag">picked · {isFast && isDeep ? "talk + agents" : isFast ? "talk" : "agents"}</span>}
                      {isFb && <span className="tag tag--use">fallback</span>}
                      {m.id === newest && <span className="tag tag--new">newest</span>}
                      {m.vision && <span className="tag">sees images</span>}
                      {m.context && <span className="tag">{ctx(m.context)} ctx</span>}
                    </span>
                  </div>
                  <div className="mrow__price" title="USD per million tokens, input / output">
                    {m.price ? (
                      <>
                        ${money(m.price.input)} <span className="muted">/</span> ${money(m.price.output)}
                      </>
                    ) : (
                      <span className="muted">no price</span>
                    )}
                  </div>
                  <div className="mrow__pick">
                    <button className={`tagbox ${isFast ? "tagbox--on" : ""}`} title="Talk model: answers when you hold the talk keys (quick)" onClick={() => setChoice("fast", m.id)}>
                      T
                    </button>
                    <button className={`tagbox ${isDeep ? "tagbox--on" : ""}`} title="Use for background and live agents (deep)" onClick={() => setChoice("deep", m.id)}>
                      A
                    </button>
                    <button className={`tagbox ${isFb ? "tagbox--on" : ""}`} title="Fallback: answers when the main model fails (one for all providers)" onClick={() => setFallback(m.id)}>
                      F
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
          {!filter && shown.length > 10 && (
            <button className="btn btn--link" onClick={() => setAll(!all)}>
              {all ? "Show fewer" : `Show all ${shown.length}`}
            </button>
          )}
        </>
      )}
    </div>
  );
}

export function ModelsPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const detected = view?.active ?? "";
  const [open, setOpen] = useState(detected || "claude");
  if (!view) return null;
  const save: Save = (patch) => api.setSettings(patch).then(onSaved);
  const eff = view.effective?.[detected] ?? { fast: "", deep: "" };
  const p = PROVIDERS.find((x) => x.id === open) ?? PROVIDERS[0];
  return (
    <div className="stack-gap stack-gap--wide">
      <div className="now now--4" data-sid="models_now">
        <div className="now__cell">
          <span>answering with</span>
          <b>{PROVIDERS.find((x) => x.id === detected)?.name ?? "nothing yet"}</b>
        </div>
        <div className="now__cell" title="Answers when you hold the talk keys. Not the speaking voice: that's Settings → Voice.">
          <span>talk model</span>
          <b>{eff.fast || "—"}</b>
        </div>
        <div className="now__cell" title="Background and live agents.">
          <span>agent model</span>
          <b>{eff.deep || "—"}</b>
        </div>
        <div className="now__cell" title="Answers only when the main model fails.">
          <span>fallback</span>
          <b>
            {view.settings.fallback ? view.settings.fallback.split(":").slice(1).join(":") : "none"}
            {view.settings.fallback && (
              <button className="btn btn--link now__clear" onClick={() => save({ fallback: "" })} title="Remove the fallback">
                ×
              </button>
            )}
          </b>
        </div>
      </div>
      <div className="ptabs" role="tablist">
        {PROVIDERS.map((x) => (
          <button key={x.id} data-tip={`${x.name}: key, models and prices`} role="tab" aria-selected={open === x.id} className={`ptab ${open === x.id ? "ptab--on" : ""}`} onClick={() => setOpen(x.id)}>
            <b>{x.name}</b>
            <span>{detected === x.id ? "■ in use" : view.settings[x.keyField] ? "key set" : "no key"}</span>
          </button>
        ))}
      </div>
      <ProviderPane key={p.id} p={p} view={view} save={save} active={detected === p.id} />
      <p className="muted small">
        Lists come live from each provider, newest first. Prices are USD per million tokens (input / output) from the public price list. Pick <b>T</b> for the talk model (what answers when you hold the talk keys; the speaking voice itself is in Settings → Voice) and <b>A</b> for background and live agents; unpicked means the provider's default, marked "in use". <b>F</b> is one fallback model, from any provider with a key, used only when the main one fails before answering. Some ids are the provider's own aliases for its newest model (DeepSeek's <code>deepseek-flash</code> is V4.1-Flash today); the official name is shown next to them.
      </p>
    </div>
  );
}

function fmt(n: number) {
  return n >= 1e6 ? `${(n / 1e6).toFixed(2)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}k` : `${n}`;
}

function ago(ms: number) {
  if (!ms) return "never";
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? "just now" : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} d ago`;
}

const PNAME: Record<string, string> = { claude: "Claude", openai: "OpenAI", deepseek: "DeepSeek" };

/** Tokens per day across the whole range (empty days included), so one busy day isn't a wall. */
function DayChart({ days, byDay, since }: { days: number; byDay: { day: string; tokens: number; cost: number }[]; since: number | null }) {
  const key = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const map = new Map(byDay.map((d) => [d.day, d]));
  const span = days > 0 ? days : Math.min(366, Math.max(7, Math.ceil((Date.now() - (since ?? Date.now())) / 86_400_000) + 1));
  const n = Math.max(7, span);
  const series = Array.from({ length: n }, (_, i) => {
    const d = new Date();
    d.setDate(d.getDate() - (n - 1 - i));
    const k = key(d);
    return { day: k, label: d.getDate(), tokens: map.get(k)?.tokens ?? 0, cost: map.get(k)?.cost ?? 0 };
  });
  const max = Math.max(1, ...series.map((d) => d.tokens));
  const every = n <= 14 ? 1 : n <= 31 ? 3 : n <= 90 ? 7 : 30;
  return (
    <div className="chart" aria-label="tokens per day">
      <div className="chart__max">{fmt(max)}</div>
      <div className="chart__bars">
        {series.map((d, i) => (
          <div key={d.day} className={`chart__col ${d.tokens ? "" : "chart__col--empty"}`} title={`${d.day}: ${fmt(d.tokens)} tokens · $${d.cost.toFixed(3)}`}>
            <i style={{ height: d.tokens ? `${Math.max(3, (d.tokens / max) * 100)}%` : "2px" }} />
            <span>{i % every === 0 || i === n - 1 ? d.label : ""}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export function UsagePanel({ view }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [days, setDays] = useState(30);
  const [provider, setProvider] = useState("");
  const [kind, setKind] = useState("");
  const [u, setU] = useState<UsageSummary | null>(null);
  const [refreshing, setRefreshing] = useState("");
  const load = () => api.usageSummary(days, provider, kind).then(setU).catch(() => {});
  useEffect(() => {
    load();
    const t = setInterval(load, 15000);
    return () => clearInterval(t);
  }, [days, provider, kind]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!u || !view) return null;
  return (
    <div className="stack-gap stack-gap--wide">
      {u.all_time && (
        <div className="now" data-sid="usage_total">
          <div className="now__cell">
            <span>spent so far</span>
            <b className="big-num">${u.all_time.cost.toFixed(u.all_time.cost < 1 ? 3 : 2)}</b>
          </div>
          <div className="now__cell">
            <span>tokens so far</span>
            <b className="big-num">{fmt(u.all_time.tokens)}</b>
          </div>
          <div className="now__cell">
            <span>calls since</span>
            <b className="big-num">{u.all_time.requests} · {u.all_time.since ? new Date(u.all_time.since).toLocaleDateString([], { month: "short", day: "numeric" }) : "—"}</b>
          </div>
        </div>
      )}
      <div className="usage__filters">
        <div className="seg-control">
          {[
            [1, "today"],
            [7, "7 days"],
            [30, "30 days"],
            [90, "90 days"],
            [0, "all time"],
          ].map(([d, label]) => (
            <button key={d} className={days === d ? "is-on" : ""} onClick={() => setDays(d as number)}>
              {label}
            </button>
          ))}
        </div>
        <div className="seg-control">
          {["", ...(u.providers ?? [])].map((p) => (
            <button key={p || "all"} className={provider === p ? "is-on" : ""} onClick={() => setProvider(p)}>
              {p ? PNAME[p] ?? p : "all models"}
            </button>
          ))}
        </div>
        <div className="seg-control">
          {[
            ["", "both"],
            ["voice", "talk"],
            ["job", "agents"],
          ].map(([k, label]) => (
            <button key={k || "both"} className={kind === k ? "is-on" : ""} onClick={() => setKind(k)}>
              {label}
            </button>
          ))}
        </div>
      </div>
      <div className="usage__bar">
        <span className="muted small" data-sid="prices">
          prices updated {ago(u.prices_at ?? 0)} ·{" "}
          <button
            className="btn btn--link"
            onClick={() => {
              setRefreshing("refreshing…");
              api
                .pricesRefresh()
                .then(() => {
                  setRefreshing("");
                  load();
                })
                .catch((e) => setRefreshing(String(e)));
            }}
          >
            refresh
          </button>{" "}
          {refreshing}
        </span>
      </div>
      <div className="stats">
        <div className="stat"><span>requests</span><b>{u.requests}</b></div>
        <div className="stat"><span>tokens in</span><b>{fmt(u.input)}</b></div>
        <div className="stat"><span>tokens out</span><b>{fmt(u.output)}</b></div>
        <div className="stat"><span>cost</span><b>${u.cost.toFixed(u.cost < 1 ? 3 : 2)}</b>{u.unpriced && <i>+ models without a price</i>}</div>
      </div>
      {u.by_day.length > 0 && <DayChart days={days} byDay={u.by_day} since={u.all_time?.since ?? null} />}
      <div className="utable-wrap">
        <table className="utable">
          <thead>
            <tr>
              <th>model</th>
              <th className="num">calls</th>
              <th className="num" title="Calls from talking to Pixie / from agents">talk · agents</th>
              <th className="num">tokens in</th>
              <th className="num">tokens out</th>
              <th className="num" title="USD per million tokens, input · output">$/M</th>
              <th className="num">cost</th>
            </tr>
          </thead>
          <tbody>
            {u.models.length === 0 && (
              <tr>
                <td colSpan={7} className="muted">Nothing yet. Ask Pixie something.</td>
              </tr>
            )}
            {u.models.map((m) => (
              <tr key={m.model}>
                <td className="utable__model">
                  <b>{m.model}</b>
                  <span>{PNAME[m.provider] ?? m.provider}</span>
                </td>
                <td className="num">{m.requests}</td>
                <td className="num">{m.voice} · {m.job}</td>
                <td className="num">{fmt(m.input)}</td>
                <td className="num">{fmt(m.output)}</td>
                <td className="num">{m.price ? `${money(m.price.input)} · ${money(m.price.output)}` : <span className="muted">—</span>}</td>
                <td className="num"><b>{m.cost === null ? "—" : `$${m.cost.toFixed(m.cost < 1 ? 4 : 2)}`}</b></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="muted small">Token counts are what each provider reports for every call. Prices come from the public price list (OpenRouter's model catalogue, which carries Anthropic, OpenAI and DeepSeek list prices) and refresh daily.</p>
    </div>
  );
}

const AGENTS = [
  { id: "codex", label: "Codex CLI", by: "OpenAI", install: "npm i -g @openai/codex", models: "openai", hint: "e.g. gpt-5-codex" },
  { id: "claude", label: "Claude Code", by: "Anthropic", install: "npm i -g @anthropic-ai/claude-code", models: "claude", hint: "e.g. claude-opus-5-5" },
  { id: "opencode", label: "opencode", by: "any provider", install: "npm i -g opencode-ai", models: "", hint: "provider/model" },
];

const SANDBOX = [
  ["read-only", "Read only", "Looks, never writes."],
  ["workspace-write", "Workspace", "Writes inside the project folder."],
  ["danger-full-access", "Full access", "Anything on this Mac. Careful."],
];
const PERMS = [
  ["plan", "Plan only", "Proposes changes, edits nothing."],
  ["acceptEdits", "Edit files", "Edits and runs the allowed tools."],
  ["bypassPermissions", "Bypass", "Never asks. Careful."],
];

function Copy({ text }: { text: string }) {
  const [ok, setOk] = useState(false);
  return (
    <button
      className="copy"
      data-tip="Copy, then paste it in Terminal"
      onClick={(e) => {
        e.stopPropagation();
        navigator.clipboard.writeText(text).then(() => {
          setOk(true);
          setTimeout(() => setOk(false), 1200);
        });
      }}
    >
      <code>{text}</code>
      <span>{ok ? "copied" : "copy"}</span>
    </button>
  );
}

export function CodingPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [st, setSt] = useState<Awaited<ReturnType<typeof api.codersStatus>> | null>(null);
  const [suggest, setSuggest] = useState<Record<string, string[]>>({});
  const load = () => api.codersStatus().then(setSt).catch(() => {});
  useEffect(() => {
    load();
    for (const p of ["openai", "claude"]) api.modelsAvailable(p).then((r) => setSuggest((x) => ({ ...x, [p]: r.models.map((m) => m.id) }))).catch(() => {});
  }, []);
  if (!view) return null;
  const c = view.settings.coding ?? { agent: "", models: {}, codex_sandbox: "", claude_permission: "", dir: "" };
  const save = (patch: Partial<typeof c>) => api.setSettings({ coding: { ...c, ...patch } }).then(onSaved);
  const status = (id: string) => st?.agents.find((a) => a.id === id);
  const firstInstalled = AGENTS.find((a) => status(a.id)?.installed)?.id;
  const pick = c.agent || "auto";
  const effective = pick === "auto" ? firstInstalled : pick;
  const sel = AGENTS.find((a) => a.id === effective);
  const anyInstalled = !!firstInstalled;
  return (
    <div className="stack-gap stack-gap--wide">
      <div className="now now--4">
        <div className="now__cell"><span>code goes to</span><b>{sel ? sel.label : "nothing installed"}</b></div>
        <div className="now__cell"><span>model</span><b>{(effective && c.models?.[effective]) || "agent's default"}</b></div>
        <div className="now__cell"><span>folder</span><b>{c.dir || "the job's output/"}</b></div>
        <div className="now__cell"><span>found</span><b>{st ? `${st.agents.filter((a) => a.installed).length} of 3` : "…"}</b></div>
      </div>

      <div className="agent-cards">
        <button className={`acard ${pick === "auto" ? "acard--on" : ""}`} onClick={() => save({ agent: "auto" })} data-tip="Uses the first one installed: Codex, then Claude Code, then opencode.">
          <div className="acard__top"><Glyph name="sparkle" size={16} /><b>Automatic</b></div>
          <span className="acard__by">first one installed</span>
          <span className="acard__state">{firstInstalled ? `→ ${AGENTS.find((a) => a.id === firstInstalled)?.label}` : "none yet"}</span>
        </button>
        {AGENTS.map((a) => {
          const s2 = status(a.id);
          return (
            <div key={a.id} className={`acard ${pick === a.id ? "acard--on" : ""} ${s2?.installed ? "" : "acard--missing"}`} role="button" tabIndex={0} onClick={() => s2?.installed && save({ agent: a.id })} data-tip={s2?.installed ? `Send code tasks to ${a.label}` : `${a.label} isn't installed yet`}>
              <div className="acard__top"><Glyph name="terminal" size={16} /><b>{a.label}</b></div>
              <span className="acard__by">by {a.by}</span>
              {s2?.installed ? <span className="acard__state">■ {s2.version || "installed"}</span> : <Copy text={a.install} />}
            </div>
          );
        })}
      </div>
      {st?.t3code && <p className="muted small">T3 Code is installed: it drives these same agents, so its setup carries over.</p>}

      {!anyInstalled && (
        <div className="banner banner--warn">
          No coding agent on this Mac yet. Install one with the command on its card, then <button className="btn btn--link" onClick={load}>check again</button>. Until then, agents write code themselves with plain file tools.
        </div>
      )}

      {sel && (
        <div className="apane">
          <h3 className="sub">{sel.label}</h3>
          <label className="field">
            <span>Model <span className="muted">· blank = {sel.label}'s own default</span></span>
            <input
              list={`models-${sel.id}`}
              placeholder={sel.hint}
              defaultValue={c.models?.[sel.id] ?? ""}
              key={sel.id}
              onBlur={(e) => save({ models: { ...(c.models ?? {}), [sel.id]: e.target.value.trim() } })}
            />
            <datalist id={`models-${sel.id}`}>
              {(sel.models ? suggest[sel.models] ?? [] : [...(suggest.openai ?? []).map((m) => `openai/${m}`), ...(suggest.claude ?? []).map((m) => `anthropic/${m}`)]).slice(0, 60).map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          </label>
          {sel.id === "codex" && (
            <div className="field">
              <span>What Codex may touch</span>
              <div className="choice-cards">
                {SANDBOX.map(([id, name, why]) => (
                  <button key={id} className={`choice ${(c.codex_sandbox || "workspace-write") === id ? "choice--on" : ""}`} onClick={() => save({ codex_sandbox: id })}>
                    <b>{name}</b>
                    <span>{why}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
          {sel.id === "claude" && (
            <div className="field">
              <span>What Claude Code may do</span>
              <div className="choice-cards">
                {PERMS.map(([id, name, why]) => (
                  <button key={id} className={`choice ${(c.claude_permission || "acceptEdits") === id ? "choice--on" : ""}`} onClick={() => save({ claude_permission: id })}>
                    <b>{name}</b>
                    <span>{why}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
          {sel.id === "opencode" && <p className="muted small">opencode runs non-interactively with its own permission rules (<code>opencode run --auto</code>). Models are written provider/model.</p>}
          {s2Path(st, sel.id) && <p className="muted small">Found at <code>{s2Path(st, sel.id)}</code></p>}
        </div>
      )}

      <ul className="rows">
        <li className="row row--form" data-sid="coding_dir">
          <div className="row__text">
            <div className="row__label">Default folder</div>
            <div className="row__why">Where agents work when a task doesn't name a folder. Blank: each job's own output folder.</div>
          </div>
          <input className="inline-input" placeholder="~/Code" defaultValue={c.dir} onBlur={(e) => save({ dir: e.target.value.trim() })} />
        </li>
      </ul>
      <p className="muted small">Try: “build me a script that renames these photos by date” or “fix the failing test in ~/Code/app”. The agent's summary comes back in the job's note.</p>
    </div>
  );
}

function s2Path(st: Awaited<ReturnType<typeof api.codersStatus>> | null, id: string) {
  return st?.agents.find((a) => a.id === id)?.path ?? "";
}
