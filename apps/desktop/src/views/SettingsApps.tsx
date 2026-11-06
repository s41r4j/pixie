// Settings → Apps (Calendar, Reminders, Contacts, Mail, Google) and Settings → Connectors
// (one-click MCP servers, custom ones from mcp.json, and the Chrome extension).
import { useEffect, useState } from "react";
import { api, IS_MAC, MACHINE, onEvent, type AppsStatus, type ConnectorCard, type Gallery, type SettingsView } from "../lib/bridge";
import { Glyph } from "../components/Mark";

function Toggle({ on, busy, label, onChange }: { on: boolean; busy?: boolean; label: string; onChange: (v: boolean) => void }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} disabled={busy} data-tip={`${label}: ${on ? "on" : "off"}`} className={`switch ${on ? "switch--on" : ""} ${busy ? "switch--busy" : ""}`} onClick={() => onChange(!on)}>
      <i />
    </button>
  );
}

const MAC_APPS: { id: "calendar" | "reminders" | "contacts" | "mail"; name: string; icon: string; why: string; asks: string }[] = [
  { id: "calendar", name: "Calendar", icon: "grid", why: "“What's on tomorrow?”, “Move lunch to 1”. Every calendar in Calendar.app, including Google and Exchange accounts added in System Settings → Internet Accounts.", asks: "Calendars" },
  { id: "reminders", name: "Reminders", icon: "check", why: "“Remind me to call Sam at 5”, “What's on my list?”", asks: "Reminders" },
  { id: "contacts", name: "Contacts", icon: "chat", why: "Finds people's emails and numbers when you mention them.", asks: "Contacts" },
  { id: "mail", name: "Mail", icon: "layers", why: "Reads your inbox in Apple Mail and drafts replies. Drafts open in Mail for you to send; Pixie never sends.", asks: "Automation → Mail" },
];

const PANE: Record<string, string> = { calendar: "Calendars", reminders: "Reminders", contacts: "Contacts", mail: "Automation" };

const WORD: Record<string, string> = { granted: "allowed", denied: "blocked", restricted: "restricted", not_determined: "not asked yet", write_only: "add-only", ask: "asks on first use", unknown: "…" };

export function AppsPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [st, setSt] = useState<AppsStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<Record<string, string>>({});
  const load = () => api.appsStatus().then(setSt).catch(() => {});
  useEffect(() => {
    load();
    // Pixie switched an app on or off itself.
    const un = onEvent((e) => {
      if (e.kind === "settings.external") load();
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const flip = (id: string, on: boolean) => {
    setBusy(id);
    setErr((e) => ({ ...e, [id]: "" }));
    api
      .appSet(id, on)
      .catch((e) => setErr((x) => ({ ...x, [id]: String(e) })))
      .finally(() => {
        setBusy(null);
        load();
        api.settings().then(onSaved).catch(() => {});
      });
  };

  const g = st?.google;
  const s = view?.settings;
  const [cid, setCid] = useState("");
  const [csec, setCsec] = useState("");
  useEffect(() => {
    if (s) {
      setCid(s.google_client_id ?? "");
      setCsec(s.google_client_secret ?? "");
    }
  }, [s?.google_client_id, s?.google_client_secret]);
  const saveClient = () => api.setSettings({ google_client_id: cid.trim(), google_client_secret: csec.trim() }).then(onSaved).then(load);
  const signIn = async () => {
    setBusy("google");
    setErr((e) => ({ ...e, google: "" }));
    try {
      await saveClient();
      await api.googleSignIn();
    } catch (e) {
      setErr((x) => ({ ...x, google: String(e) }));
    } finally {
      setBusy(null);
      load();
    }
  };

  return (
    <div className="stack-gap stack-gap--wide">
      <h3 className="sub">{IS_MAC ? "On this Mac" : "Mac apps"}</h3>
      {st && !st.supported && <div className="banner banner--muted">These use macOS apps, so they're Mac-only.</div>}
      <ul className="app-list">
        {MAC_APPS.map((a) => {
          const on = !!st?.on?.[a.id];
          const perm = st?.[a.id] ?? "unknown";
          return (
            <li key={a.id} className={`app-row ${on ? "app-row--on" : ""}`} data-sid={`app_${a.id}`}>
              <span className="app-row__icon">
                <Glyph name={a.icon} size={18} />
              </span>
              <div className="app-row__text">
                <div className="app-row__name">
                  {a.name}
                  <span className={`pill ${perm === "granted" ? "pill--ok" : perm === "denied" || perm === "restricted" ? "pill--warn" : "pill--muted"}`}>{WORD[perm] ?? perm}</span>
                </div>
                <div className="row__why">{a.why}</div>
                {err[a.id] && (
                  <div className="app-row__err">
                    {err[a.id]}{" "}
                    <button className="btn btn--link" onClick={() => api.openTarget(`x-apple.systempreferences:com.apple.preference.security?Privacy_${PANE[a.id]}`)}>
                      Open System Settings
                    </button>
                  </div>
                )}
              </div>
              <Toggle on={on} busy={busy === a.id} label={a.name} onChange={(v) => flip(a.id, v)} />
            </li>
          );
        })}
      </ul>
      <p className="muted small">Turning one on asks macOS once. Pixie only reads or changes these when you ask it to.</p>

      <h3 className="sub">Google account</h3>
      <div className={`gcard ${g?.signed_in ? "gcard--on" : ""}`}>
        <div className="gcard__head">
          <span className="gcard__logo" aria-hidden>
            G
          </span>
          <div>
            <b>Gmail &amp; Google Calendar</b>
            <div className="row__why">
              {g?.signed_in ? (
                <>
                  Connected as <b>{g.email || "your account"}</b>. Pixie can search and read mail, save drafts (never sends) and read or add events.
                </>
              ) : (
                "Straight from Google, no Mac apps needed. Read and search mail, write drafts, see and add events."
              )}
            </div>
          </div>
          {g?.signed_in ? (
            <button className="btn btn--ghost" onClick={() => api.googleSignOut().then(load)}>
              Sign out
            </button>
          ) : (
            <button className="btn btn--primary" disabled={busy === "google" || !cid.trim()} onClick={signIn}>
              {busy === "google" ? "Waiting for Google…" : "Sign in with Google"}
            </button>
          )}
        </div>
        {err.google && <div className="app-row__err">{err.google}</div>}
        {!g?.signed_in && (
          <details className="gcard__setup" open={!cid}>
            <summary>Your Google OAuth client {cid ? "· set" : "· needed once"}</summary>
            <ol className="steps">
              <li>
                In{" "}
                <button className="btn btn--link" onClick={() => api.openTarget("https://console.cloud.google.com/apis/credentials")}>
                  Google Cloud → Credentials
                </button>{" "}
                create an <b>OAuth client ID</b> of type <b>Desktop app</b>.
              </li>
              <li>
                Enable the <b>Gmail API</b> and <b>Google Calendar API</b>, and add your address as a test user on the OAuth consent screen.
              </li>
              <li>Paste the client ID and secret here, then sign in. Tokens stay on this {MACHINE}.</li>
            </ol>
            <div className="gcard__fields">
              <label className="field">
                <span>Client ID</span>
                <input value={cid} placeholder="1234…apps.googleusercontent.com" onChange={(e) => setCid(e.target.value)} onBlur={saveClient} spellCheck={false} />
              </label>
              <label className="field">
                <span>Client secret</span>
                <input value={csec} placeholder="GOCSPX-…" onChange={(e) => setCsec(e.target.value)} onBlur={saveClient} spellCheck={false} />
              </label>
            </div>
          </details>
        )}
      </div>
    </div>
  );
}

const GROUPS: { id: string; title: string; blurb: string }[] = [
  { id: "work", title: "Work and code", blurb: "Issues, pull requests, projects and errors." },
  { id: "notes", title: "Notes and messages", blurb: "Your pages and conversations." },
  { id: "search", title: "Search and docs", blurb: "Fresh results from the web and library docs." },
  { id: "files", title: "Files", blurb: "A folder Pixie may read and write." },
];

const AUTH: Record<string, string> = { token: "Token", browser: "Browser sign-in", none: "No setup", folder: "Pick a folder", custom: "Your server" };

type Status = { tone: "idle" | "wait" | "ok" | "off" | "err"; text: string };

function statusOf(c: ConnectorCard): Status {
  if (!c.added) return { tone: "idle", text: "Not added" };
  if (!c.on) return { tone: "off", text: "Off" };
  if (c.error) return { tone: "err", text: "Couldn't connect" };
  if (c.tools != null) return { tone: "ok", text: `Connected · ${c.tools} tool${c.tools === 1 ? "" : "s"}` };
  return { tone: "wait", text: c.auth === "browser" ? "Connecting… sign in in your browser if it asks" : "Connecting…" };
}

/** Plain words for the usual failures; the raw message stays one click away. */
function explain(err: string): string {
  const e = err.toLowerCase();
  if (e.includes("isn't installed")) return "Node.js isn't installed, so the connector can't start.";
  if (e.includes("401") || e.includes("unauthorized") || e.includes("bad credentials") || e.includes("invalid token") || e.includes("invalid_auth")) return "The token was refused. Check it, then Edit and save it again.";
  if (e.includes("403") || e.includes("forbidden")) return "The token works but isn't allowed to do this. Give it more permissions.";
  if (e.includes("404") || e.includes("not found")) return "The server couldn't be found. Check your connection and try again.";
  if (e.includes("timed out")) return "It took too long to answer. It may still be downloading the first time; try again.";
  if (e.includes("enotfound") || e.includes("network") || e.includes("econnrefused")) return "No connection to the service. Check your internet and try again.";
  return "It stopped while starting.";
}

function Monogram({ name }: { name: string }) {
  const letters = name.replace(/[^A-Za-z0-9 ]/g, " ").split(/\s+/).filter((w, i, all) => Boolean(w) && !(i === 0 && all.length > 1 && /^(a|an|the)$/i.test(w))).map((w) => w[0]).join("").slice(0, 2).toUpperCase();
  return <span className="cmark" aria-hidden>{letters || "?"}</span>;
}

function ConnectorTile({ c, node, onChange }: { c: ConnectorCard; node: boolean; onChange: (g: Gallery) => void }) {
  const [open, setOpen] = useState(false);
  const [showTools, setShowTools] = useState(false);
  const [showErr, setShowErr] = useState(false);
  const [vals, setVals] = useState<Record<string, string>>({});
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const run = (p: Promise<Gallery>, close = true) => {
    setBusy(true);
    setErr("");
    p.then((g) => {
      onChange(g);
      if (close) setOpen(false);
    })
      .catch((e) => setErr(String(e)))
      .finally(() => setBusy(false));
  };
  const st = statusOf(c);
  const needsForm = !!(c.fields?.length || c.folder);
  const folderArg = c.args?.[c.args.length - 1] ?? "";
  const remove = () => {
    if (!confirmRemove) {
      setConfirmRemove(true);
      window.setTimeout(() => setConfirmRemove(false), 4000);
      return;
    }
    run(api.connectorRemove(c.id));
  };
  return (
    <div className={`ctile ctile--${st.tone}`}>
      <div className="ctile__top">
        <Monogram name={c.name} />
        <div className="ctile__title">
          <b>{c.name}</b>
          <span className="ctile__chips">
            <span className="chip">{AUTH[c.auth ?? "custom"] ?? "Server"}</span>
            {c.community && (
              <span className="chip chip--muted" data-tip="Made by the community, not by the service itself.">
                Community
              </span>
            )}
          </span>
        </div>
        {c.added ? <Toggle on={c.on} busy={busy} label={c.name} onChange={(v) => run(api.connectorSave(c.id, {}, v))} /> : null}
      </div>
      {c.blurb && <span className="ctile__blurb">{c.blurb}</span>}
      {!c.blurb && c.command && (
        <code className="ctile__cmd">
          {c.command} {(c.args ?? []).join(" ")}
        </code>
      )}
      <span className={`ctile__state ctile__state--${st.tone}`}>
        <i className="dot" /> {st.text}
      </span>
      {st.tone === "err" && c.error && (
        <div className="ctile__err">
          <span>{explain(c.error)}</span>
          <span className="ctile__errbtns">
            <button className="btn btn--link" onClick={() => run(api.connectorRetry(c.id), false)} disabled={busy}>
              Retry
            </button>
            <button className="btn btn--link" onClick={() => setShowErr((v) => !v)}>
              {showErr ? "Hide details" : "Details"}
            </button>
          </span>
          {showErr && <code className="ctile__raw">{c.error}</code>}
        </div>
      )}
      {st.tone === "ok" && !!c.tool_list?.length && (
        <>
          <button className="btn btn--link ctile__more" onClick={() => setShowTools((v) => !v)}>
            {showTools ? "Hide what it can do" : "What it can do"}
          </button>
          {showTools && (
            <ul className="ctile__tools">
              {c.tool_list.map((t) => (
                <li key={t.name} data-tip={t.description || t.name}>
                  {t.name}
                </li>
              ))}
            </ul>
          )}
        </>
      )}
      {!open && (
        <div className="ctile__actions">
          {!c.added ? (
            <button
              className="btn btn--ghost"
              disabled={!node || busy}
              onClick={() => (needsForm ? setOpen(true) : run(api.connectorSave(c.id, {}, true)))}
              data-tip={node ? (needsForm ? `Set up ${c.name}` : `Add ${c.name}: no setup needed`) : "Needs Node.js first"}
            >
              <Glyph name="plus" size={12} /> {needsForm ? "Set up" : "Add"}
            </button>
          ) : (
            <>
              {needsForm && (
                <button className="btn btn--link" onClick={() => setOpen(true)}>
                  Edit
                </button>
              )}
              <button className={`btn btn--link ${confirmRemove ? "btn--link-danger" : ""}`} onClick={remove}>
                {confirmRemove ? "Remove for good?" : "Remove"}
              </button>
            </>
          )}
          {c.help_url && !open && (
            <button className="btn btn--link ctile__help" onClick={() => api.openTarget(c.help_url!)} data-tip="Their setup guide">
              Guide
            </button>
          )}
        </div>
      )}
      {open && (
        <div className="ctile__form">
          {c.help && (
            <p className="muted small">
              {c.help}{" "}
              {c.help_url && (
                <button className="btn btn--link" onClick={() => api.openTarget(c.help_url!)}>
                  Get it
                </button>
              )}
            </p>
          )}
          {c.fields?.map((f) => (
            <label key={f.key} className="field">
              <span>{f.label}</span>
              <input type="password" placeholder={c.env?.[f.key] || f.placeholder} value={vals[f.key] ?? ""} onChange={(e) => setVals((v) => ({ ...v, [f.key]: e.target.value }))} spellCheck={false} autoComplete="off" />
            </label>
          ))}
          {c.folder && (
            <label className="field">
              <span>Folder</span>
              <input placeholder={folderArg || "~/Documents/Projects"} value={vals.folder ?? ""} onChange={(e) => setVals((v) => ({ ...v, folder: e.target.value }))} spellCheck={false} />
            </label>
          )}
          <p className="muted small">Saved on this {MACHINE} in mcp.json, and only sent to {c.name}.</p>
          {err && <div className="app-row__err">{err}</div>}
          <div className="ctile__actions">
            <button className="btn btn--primary" disabled={busy} onClick={() => run(api.connectorSave(c.id, vals, true))}>
              {busy ? "Connecting…" : c.added ? "Save" : "Connect"}
            </button>
            <button className="btn btn--link" onClick={() => setOpen(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {!open && err && <div className="app-row__err">{err}</div>}
    </div>
  );
}

/** Split a command line the way a shell would, quotes included. */
function words(line: string): string[] {
  const out: string[] = [];
  const re = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(line))) out.push(m[1] ?? m[2] ?? m[3]);
  return out;
}

function AddCustom({ onChange }: { onChange: (g: Gallery) => void }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [line, setLine] = useState("");
  const [env, setEnv] = useState("");
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const add = () => {
    const parts = words(line);
    const vars: Record<string, string> = {};
    for (const l of env.split("\n")) {
      const i = l.indexOf("=");
      if (i > 0) vars[l.slice(0, i).trim()] = l.slice(i + 1).trim();
    }
    setBusy(true);
    setErr("");
    api
      .connectorAddCustom(name, parts[0] ?? "", parts.slice(1), vars)
      .then((g) => {
        onChange(g);
        setOpen(false);
        setName("");
        setLine("");
        setEnv("");
      })
      .catch((e) => setErr(String(e)))
      .finally(() => setBusy(false));
  };
  if (!open)
    return (
      <button className="btn btn--ghost" onClick={() => setOpen(true)}>
        <Glyph name="plus" size={12} /> Add your own server
      </button>
    );
  return (
    <div className="ctile ctile--form-wide">
      <b className="ctile__formtitle">Your own MCP server</b>
      <label className="field">
        <span>Name</span>
        <input value={name} placeholder="my-server" onChange={(e) => setName(e.target.value)} spellCheck={false} />
      </label>
      <label className="field">
        <span>Command</span>
        <input value={line} placeholder="npx -y @scope/some-mcp-server --flag" onChange={(e) => setLine(e.target.value)} spellCheck={false} />
      </label>
      <label className="field">
        <span>Environment (one KEY=value per line, optional)</span>
        <textarea rows={3} value={env} placeholder="API_KEY=…" onChange={(e) => setEnv(e.target.value)} spellCheck={false} />
      </label>
      <p className="muted small">Only add servers you trust: they run on this {MACHINE} with your permissions.</p>
      {err && <div className="app-row__err">{err}</div>}
      <div className="ctile__actions">
        <button className="btn btn--primary" disabled={busy || !name.trim() || !line.trim()} onClick={add}>
          {busy ? "Adding…" : "Add"}
        </button>
        <button className="btn btn--link" onClick={() => setOpen(false)}>
          Cancel
        </button>
      </div>
    </div>
  );
}

export function ConnectorsPanel() {
  const [g, setG] = useState<Gallery | null>(null);
  const [st, setSt] = useState<Awaited<ReturnType<typeof api.status>> | null>(null);
  useEffect(() => {
    const tick = () => {
      api.connectorsGallery().then(setG).catch(() => {});
      api.status().then(setSt).catch(() => {});
    };
    tick();
    const t = setInterval(tick, 3000);
    return () => clearInterval(t);
  }, []);
  const all = [...(g?.presets ?? []), ...(g?.custom ?? [])];
  const live = all.filter((c) => c.added && c.on && c.tools != null);
  const broken = all.filter((c) => c.added && c.on && c.error);
  const toolCount = live.reduce((n, c) => n + (c.tools ?? 0), 0);
  return (
    <div className="stack-gap stack-gap--wide">
      <div className="csummary">
        <div>
          <b>{live.length}</b>
          <span>connected</span>
        </div>
        <div>
          <b>{toolCount}</b>
          <span>tools Pixie can use</span>
        </div>
        <div className={broken.length ? "csummary--err" : ""}>
          <b>{broken.length}</b>
          <span>need attention</span>
        </div>
        <div>
          <b>{g ? (g.node ? "Ready" : "Missing") : "…"}</b>
          <span>Node.js</span>
        </div>
      </div>
      {g && !g.node && (
        <div className="banner banner--warn">
          Connectors run as small local servers through Node.js, which isn't on this {MACHINE} yet. Install it from{" "}
          <button className="btn btn--link" onClick={() => api.openTarget("https://nodejs.org/en/download")}>
            nodejs.org
          </button>{" "}
          {IS_MAC && (
            <>
              (or <code>brew install node</code>)
            </>
          )}
          , then come back.
        </div>
      )}
      <p className="muted small">
        Connectors give Pixie tools from other services. Pixie uses them when you ask (“what's assigned to me in Linear?”) and asks before anything that posts, sends or deletes. Tokens stay on this {MACHINE}.
      </p>

      {GROUPS.map((grp) => {
        const list = (g?.presets ?? []).filter((c) => (c.group ?? "work") === grp.id);
        if (!list.length) return null;
        return (
          <section key={grp.id} className="cgroup">
            <h3 className="sub">
              {grp.title} <span className="muted">· {grp.blurb}</span>
            </h3>
            <div className="ctiles">
              {list.map((c) => (
                <ConnectorTile key={c.id} c={c} node={g!.node} onChange={setG} />
              ))}
            </div>
          </section>
        );
      })}

      <section className="cgroup">
        <h3 className="sub">
          Your own servers <span className="muted">· any MCP server, from mcp.json or added here</span>
        </h3>
        {!!g?.custom.length && (
          <div className="ctiles">
            {g.custom.map((c) => (
              <ConnectorTile key={c.id} c={c} node={g.node} onChange={setG} />
            ))}
          </div>
        )}
        <div className="cfoot">
          {g && <AddCustom onChange={setG} />}
          <button className="btn btn--link" onClick={() => g && api.openTarget(g.path)}>
            Open mcp.json
          </button>
        </div>
      </section>

      <h3 className="sub">Chrome</h3>
      <div className={`banner ${st?.browser ? "banner--ok" : "banner--muted"}`}>
        <b>Chrome</b> {st?.browser ? "is connected." : "isn't connected yet."}
      </div>
      {!st?.browser && (
        <ol className="steps">
          <li>
            Open <code>chrome://extensions</code> and switch on Developer mode.
          </li>
          <li>
            Click <b>Load unpacked</b> and pick the <code>extension</code> folder in Pixie's support folder.
            <button className="btn btn--link" onClick={() => st && api.openTarget(`${st.data_dir}/extension`)}>
              Show folder
            </button>
          </li>
          <li>Pixie can then read pages, click and type in your browser when you ask.</li>
        </ol>
      )}
    </div>
  );
}
