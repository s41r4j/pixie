// Settings → Models, Usage, Coding.
import { useEffect, useState } from "react";
import { api, type Settings, type SettingsView, type UsageSummary } from "../lib/bridge";
import { Glyph } from "../components/Mark";

type Save = (patch: Partial<Settings>) => Promise<void>;

const PROVIDERS = [
  { id: "claude", name: "Claude", by: "Anthropic", keyField: "anthropic_key" as const, placeholder: "sk-ant-…", hint: "Default when its key is set." },
  { id: "openai", name: "OpenAI", by: "OpenAI", keyField: "openai_key" as const, placeholder: "sk-…", hint: "GPT models through Chat Completions." },
  { id: "deepseek", name: "DeepSeek", by: "DeepSeek", keyField: "deepseek_key" as const, placeholder: "sk-…", hint: "deepseek-flash also reads screenshots." },
];

function ProviderCard({ p, view, save, active }: { p: (typeof PROVIDERS)[number]; view: SettingsView; save: Save; active: boolean }) {
  const s = view.settings;
  const [key, setKey] = useState(s[p.keyField]);
  const [models, setModels] = useState<{ id: string; name: string; vision: boolean | null }[]>([]);
  const [state, setState] = useState<"idle" | "loading" | "ok" | "nokey" | "error">("idle");
  const [err, setErr] = useState("");
  const [test, setTest] = useState("");
  const choice = s.models?.[p.id] ?? { fast: "", deep: "" };

  const load = () => {
    setState("loading");
    api
      .modelsAvailable(p.id)
      .then((r) => {
        setModels(r.models);
        setState("ok");
      })
      .catch((e) => {
        const m = String(e);
        setState(/No key/i.test(m) ? "nokey" : "error");
        setErr(m);
      });
  };
  useEffect(load, []); // eslint-disable-line react-hooks/exhaustive-deps

  const setChoice = (k: "fast" | "deep", v: string) => save({ models: { ...(s.models ?? {}), [p.id]: { ...choice, [k]: v } } });

  return (
    <div className={`pcard ${active ? "pcard--on" : ""}`}>
      <div className="pcard__head">
        <div>
          <div className="pcard__name">{p.name}</div>
          <div className="pcard__by">{p.hint}</div>
        </div>
        <span className={`pill ${state === "ok" ? "pill--ok" : "pill--muted"}`}>{state === "ok" ? `${models.length} models` : state === "loading" ? "checking" : state === "nokey" ? "no key" : state === "error" ? "error" : "—"}</span>
      </div>
      <label className="field">
        <span>API key</span>
        <input
          type="password"
          placeholder={p.placeholder}
          value={key}
          onChange={(e) => setKey(e.target.value)}
          onBlur={() => save({ [p.keyField]: key } as Partial<Settings>).then(load)}
        />
      </label>
      {state === "error" && <div className="row__hint">{err}</div>}
      {state === "ok" && (
        <div className="pcard__models">
          <label className="field">
            <span>Voice model (fast)</span>
            <select value={choice.fast} onChange={(e) => setChoice("fast", e.target.value)}>
              <option value="">Default</option>
              {models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.id}
                  {m.vision ? " · sees images" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>Jobs model (deep)</span>
            <select value={choice.deep} onChange={(e) => setChoice("deep", e.target.value)}>
              <option value="">Default</option>
              {models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.id}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      <div className="row-actions">
        <button className={`btn ${active ? "btn--primary" : ""}`} disabled={state !== "ok"} onClick={() => save({ provider: p.id })}>
          {active ? "In use" : "Use " + p.name}
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
        {test && <span className="muted small">{test}</span>}
      </div>
    </div>
  );
}

export function ModelsPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  if (!view) return null;
  const save: Save = (patch) => api.setSettings(patch).then(onSaved);
  const active = view.settings.provider || (view.provider?.startsWith("Claude") ? "claude" : view.provider?.startsWith("OpenAI") ? "openai" : view.provider?.startsWith("DeepSeek") ? "deepseek" : "");
  return (
    <div className="stack-gap stack-gap--wide">
      <div className="banner">
        Now answering with <b>{view.provider ?? "nothing yet"}</b>. Keys stay on this Mac. Pick a model for quick voice turns and a
        stronger one for background jobs.
      </div>
      <div className="pgrid">
        {PROVIDERS.map((p) => (
          <ProviderCard key={p.id} p={p} view={view} save={save} active={active === p.id} />
        ))}
      </div>
    </div>
  );
}

function fmt(n: number) {
  return n >= 1e6 ? `${(n / 1e6).toFixed(2)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}k` : `${n}`;
}

export function UsagePanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [days, setDays] = useState(30);
  const [u, setU] = useState<UsageSummary | null>(null);
  const load = () => api.usageSummary(days).then(setU).catch(() => {});
  useEffect(() => {
    load();
  }, [days]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!u || !view) return null;
  const max = Math.max(1, ...u.by_day.map((d) => d.tokens));
  const setPrice = (model: string, k: "input" | "output", v: number) => {
    const cur = view.settings.prices?.[model] ?? { input: 0, output: 0 };
    api.setSettings({ prices: { ...(view.settings.prices ?? {}), [model]: { ...cur, [k]: v } } }).then((v2) => {
      onSaved(v2);
      load();
    });
  };
  return (
    <div className="stack-gap stack-gap--wide">
      <div className="seg-control">
        {[7, 30, 90].map((d) => (
          <button key={d} className={days === d ? "is-on" : ""} onClick={() => setDays(d)}>
            {d} days
          </button>
        ))}
      </div>
      <div className="stats">
        <div className="stat"><span>requests</span><b>{u.requests}</b></div>
        <div className="stat"><span>tokens in</span><b>{fmt(u.input)}</b></div>
        <div className="stat"><span>tokens out</span><b>{fmt(u.output)}</b></div>
        <div className="stat"><span>est. cost</span><b>${u.cost.toFixed(u.cost < 1 ? 3 : 2)}</b>{u.unpriced && <i>+ unpriced models</i>}</div>
      </div>
      {u.by_day.length > 0 && (
        <div className="bars" aria-label="tokens per day">
          {u.by_day.map((d) => (
            <div key={d.day} className="bars__col" title={`${d.day}: ${fmt(d.tokens)} tokens, $${d.cost.toFixed(3)}`}>
              <i style={{ height: `${Math.max(4, (d.tokens / max) * 100)}%` }} />
              <span>{d.day.slice(8)}</span>
            </div>
          ))}
        </div>
      )}
      <table className="utable">
        <thead>
          <tr>
            <th>model</th>
            <th>calls</th>
            <th>voice / jobs</th>
            <th>in</th>
            <th>out</th>
            <th>$ / M in</th>
            <th>$ / M out</th>
            <th>cost</th>
          </tr>
        </thead>
        <tbody>
          {u.models.length === 0 && (
            <tr>
              <td colSpan={8} className="muted">Nothing yet. Ask Pixie something.</td>
            </tr>
          )}
          {u.models.map((m) => (
            <tr key={m.model}>
              <td>
                <b>{m.model}</b>
                <span className="muted"> {m.provider}</span>
              </td>
              <td>{m.requests}</td>
              <td>
                {m.voice} / {m.job}
              </td>
              <td>{fmt(m.input)}</td>
              <td>{fmt(m.output)}</td>
              <td>
                <input className="price" type="number" step="0.01" min="0" defaultValue={m.price?.input ?? ""} placeholder="—" onBlur={(e) => setPrice(m.model, "input", parseFloat(e.target.value) || 0)} />
              </td>
              <td>
                <input className="price" type="number" step="0.01" min="0" defaultValue={m.price?.output ?? ""} placeholder="—" onBlur={(e) => setPrice(m.model, "output", parseFloat(e.target.value) || 0)} />
              </td>
              <td>{m.cost === null ? "—" : `$${m.cost.toFixed(m.cost < 1 ? 4 : 2)}`}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="muted small">Costs are estimates from the token counts each provider reports. Claude prices are built in; enter prices for other models to include them.</p>
    </div>
  );
}

export function CodingPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [st, setSt] = useState<Awaited<ReturnType<typeof api.codersStatus>> | null>(null);
  useEffect(() => {
    api.codersStatus().then(setSt).catch(() => {});
  }, []);
  if (!view) return null;
  const c = view.settings.coding ?? { agent: "", models: {}, codex_sandbox: "", claude_permission: "", dir: "" };
  const save = (patch: Partial<typeof c>) => api.setSettings({ coding: { ...c, ...patch } }).then(onSaved);
  const agent = c.agent || "auto";
  return (
    <div className="stack-gap stack-gap--wide">
      <div className="banner">
        When a job needs real code, Pixie hands it to a coding agent on this Mac, in your repo, and checks the result.
        {st?.t3code && " T3 Code is installed: it drives these same agents, so its setup carries over."}
      </div>
      <div className="pgrid">
        <button className={`pcard pcard--btn ${agent === "auto" ? "pcard--on" : ""}`} onClick={() => save({ agent: "auto" })}>
          <div className="pcard__name"><Glyph name="sparkle" size={14} /> Automatic</div>
          <div className="pcard__by">First installed: Codex, then Claude Code, then opencode.</div>
        </button>
        {st?.agents.map((a) => (
          <button key={a.id} className={`pcard pcard--btn ${agent === a.id ? "pcard--on" : ""}`} disabled={!a.installed} onClick={() => save({ agent: a.id })}>
            <div className="pcard__name"><Glyph name="terminal" size={14} /> {a.label}</div>
            <div className="pcard__by">{a.installed ? `${a.version || "installed"} · ${a.path}` : "not installed"}</div>
          </button>
        ))}
      </div>
      <ul className="rows">
        {st?.agents
          .filter((a) => a.installed)
          .map((a) => (
            <li key={a.id} className="row row--form">
              <div className="row__text">
                <div className="row__label">{a.label}</div>
                <div className="row__why">Model for this agent (blank = its own default).</div>
              </div>
              <input
                className="inline-input"
                placeholder={a.id === "codex" ? "e.g. gpt-5-codex" : a.id === "claude" ? "e.g. claude-opus-5-5" : "provider/model"}
                defaultValue={c.models?.[a.id] ?? ""}
                onBlur={(e) => save({ models: { ...(c.models ?? {}), [a.id]: e.target.value.trim() } })}
              />
            </li>
          ))}
        <li className="row row--form">
          <div className="row__text">
            <div className="row__label">Codex sandbox</div>
            <div className="row__why">What Codex may touch while it works.</div>
          </div>
          <select className="inline-input" value={c.codex_sandbox || "workspace-write"} onChange={(e) => save({ codex_sandbox: e.target.value })}>
            <option value="read-only">read-only</option>
            <option value="workspace-write">workspace-write</option>
            <option value="danger-full-access">full access</option>
          </select>
        </li>
        <li className="row row--form">
          <div className="row__text">
            <div className="row__label">Claude Code permissions</div>
            <div className="row__why">acceptEdits edits files and runs the listed tools; bypass skips all prompts.</div>
          </div>
          <select className="inline-input" value={c.claude_permission || "acceptEdits"} onChange={(e) => save({ claude_permission: e.target.value })}>
            <option value="acceptEdits">acceptEdits</option>
            <option value="plan">plan only</option>
            <option value="bypassPermissions">bypassPermissions</option>
          </select>
        </li>
        <li className="row row--form">
          <div className="row__text">
            <div className="row__label">Default folder</div>
            <div className="row__why">Where agents work when a job doesn't name a folder (blank = the job's output folder).</div>
          </div>
          <input className="inline-input" placeholder="~/Code" defaultValue={c.dir} onBlur={(e) => save({ dir: e.target.value.trim() })} />
        </li>
      </ul>
    </div>
  );
}
