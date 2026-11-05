// Settings → Apps (Calendar, Reminders, Contacts, Mail, Google) and Settings → Connectors
// (one-click MCP servers, custom ones from mcp.json, and the Chrome extension).
import { useEffect, useState } from "react";
import { api, type AppsStatus, type ConnectorCard, type Gallery, type SettingsView } from "../lib/bridge";
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
      <h3 className="sub">On this Mac</h3>
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
              <li>Paste the client ID and secret here, then sign in. Tokens stay on this Mac.</li>
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

function ConnectorTile({ c, node, onChange }: { c: ConnectorCard; node: boolean; onChange: (g: Gallery) => void }) {
  const [open, setOpen] = useState(false);
  const [vals, setVals] = useState<Record<string, string>>({});
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const run = (p: Promise<Gallery>) => {
    setBusy(true);
    setErr("");
    p.then((g) => {
      onChange(g);
      setOpen(false);
    })
      .catch((e) => setErr(String(e)))
      .finally(() => setBusy(false));
  };
  const state = !c.added ? "" : !c.on ? "off" : c.error ? "error" : c.tools != null ? `${c.tools} tools` : "starting…";
  const folderArg = c.args?.[c.args.length - 1] ?? "";
  return (
    <div className={`ctile ${c.on ? "ctile--on" : ""} ${c.error && c.on ? "ctile--err" : ""}`}>
      <div className="ctile__top">
        <b>{c.name}</b>
        {c.added ? <Toggle on={c.on} busy={busy} label={c.name} onChange={(v) => run(api.connectorSave(c.id, {}, v))} /> : null}
      </div>
      {c.blurb && <span className="ctile__blurb">{c.blurb}</span>}
      {state && <span className={`ctile__state ${c.error && c.on ? "ctile__state--err" : ""}`} title={c.error ?? ""}>{c.error && c.on ? c.error.slice(0, 90) : state}</span>}
      {!open && (
        <div className="ctile__actions">
          {!c.added ? (
            <button className="btn btn--ghost" disabled={!node && !c.command} onClick={() => (c.fields?.length || c.folder ? setOpen(true) : run(api.connectorSave(c.id, {}, true)))} data-tip={node ? `Add ${c.name}` : "Needs Node.js (npx) first"}>
              <Glyph name="plus" size={12} /> Add
            </button>
          ) : (
            <>
              {(c.fields?.length || c.folder) && (
                <button className="btn btn--link" onClick={() => setOpen(true)}>
                  Edit
                </button>
              )}
              <button className="btn btn--link" onClick={() => run(api.connectorRemove(c.id))}>
                Remove
              </button>
            </>
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
              <input placeholder={c.env?.[f.key] || f.placeholder} value={vals[f.key] ?? ""} onChange={(e) => setVals((v) => ({ ...v, [f.key]: e.target.value }))} spellCheck={false} />
            </label>
          ))}
          {c.folder && (
            <label className="field">
              <span>Folder</span>
              <input placeholder={folderArg || "~/Documents/Projects"} value={vals.folder ?? ""} onChange={(e) => setVals((v) => ({ ...v, folder: e.target.value }))} spellCheck={false} />
            </label>
          )}
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
  return (
    <div className="stack-gap stack-gap--wide">
      {g && !g.node && (
        <div className="banner banner--warn">
          Connectors run as small local servers through Node.js, which isn't on this Mac yet. Install it from{" "}
          <button className="btn btn--link" onClick={() => api.openTarget("https://nodejs.org/en/download")}>
            nodejs.org
          </button>{" "}
          (or <code>brew install node</code>), then come back.
        </div>
      )}
      <div className="ctiles">
        {g?.presets.map((c) => (
          <ConnectorTile key={c.id} c={c} node={g.node} onChange={setG} />
        ))}
      </div>

      {!!g?.custom.length && (
        <>
          <h3 className="sub">Your own (from mcp.json)</h3>
          <div className="ctiles">
            {g.custom.map((c) => (
              <ConnectorTile key={c.id} c={c} node={g.node} onChange={setG} />
            ))}
          </div>
        </>
      )}
      <p className="muted small">
        Any other MCP server goes in <code>mcp.json</code>; Pixie picks up changes made here right away.{" "}
        <button className="btn btn--link" onClick={() => g && api.openTarget(g.path)}>
          Open mcp.json
        </button>
      </p>

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
