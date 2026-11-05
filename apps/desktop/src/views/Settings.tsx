// Settings + first-run welcome: permissions, the model key, voice, sounds, connectors.
import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, onEvent, type PermissionInfo, type PermState, type ResourcesReport, type Settings as SettingsT, type SettingsView, type Voice } from "../lib/bridge";
import { Mark, Wordmark } from "../components/Mark";

type Tab = "welcome" | "setup" | "ai" | "voice" | "sounds" | "connectors" | "about";

const PILL: Record<PermState, { text: string; tone: "ok" | "warn" | "muted" }> = {
  granted: { text: "Allowed", tone: "ok" },
  denied: { text: "Off", tone: "warn" },
  restricted: { text: "Blocked by policy", tone: "warn" },
  not_determined: { text: "Not asked yet", tone: "muted" },
  not_granted: { text: "Not allowed", tone: "warn" },
  unchecked: { text: "Not checked", tone: "muted" },
};

function PermRow({ p, onChange }: { p: PermissionInfo; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  const pill = PILL[p.state];
  const granted = p.state === "granted";
  return (
    <li className={`row ${granted ? "row--ok" : ""}`}>
      <div className="row__text">
        <div className="row__label">
          {p.label}
          {!p.required && <span className="row__opt">optional</span>}
        </div>
        <div className="row__why">{p.why}</div>
        {p.restart_hint && !granted && (
          <div className="row__hint">
            Switch openclicky on in System Settings, then{" "}
            <button className="btn btn--link btn--inline" onClick={() => api.relaunch()}>
              relaunch openclicky
            </button>
            .
          </div>
        )}
      </div>
      <div className="row__side">
        <span className={`pill pill--${pill.tone}`}>{pill.text}</span>
        {!granted && (
          <div className="row__actions">
            <button
              className="btn btn--primary"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await api.requestPermission(p.id);
                } finally {
                  setBusy(false);
                  onChange();
                }
              }}
            >
              {p.state === "unchecked" ? "Check" : "Allow"}
            </button>
            <button className="btn btn--link" onClick={() => api.openPermissionSettings(p.id)}>
              Open Settings
            </button>
          </div>
        )}
      </div>
    </li>
  );
}

function usePermissions() {
  const [perms, setPerms] = useState<PermissionInfo[]>([]);
  const refresh = useCallback(() => {
    api.permissions().then(setPerms).catch(console.error);
  }, []);
  useEffect(() => {
    refresh();
    const t = window.setInterval(() => document.visibilityState === "visible" && refresh(), 1500);
    window.addEventListener("focus", refresh);
    return () => {
      clearInterval(t);
      window.removeEventListener("focus", refresh);
    };
  }, [refresh]);
  return { perms, refresh, ready: perms.length > 0 && perms.filter((p) => p.required).every((p) => p.state === "granted") };
}

function Permissions() {
  const { perms, refresh } = usePermissions();
  return (
    <ul className="rows">
      {perms.map((p) => (
        <PermRow key={p.id} p={p} onChange={refresh} />
      ))}
    </ul>
  );
}

function AiPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [ds, setDs] = useState("");
  const [cl, setCl] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (view) {
      setDs(view.settings.deepseek_key);
      setCl(view.settings.anthropic_key);
    }
  }, [view]);
  const save = async (patch: Record<string, string>) => onSaved(await api.setSettings(patch));
  return (
    <div className="stack-gap">
      <div className={`banner ${view?.provider ? "banner--ok" : "banner--warn"}`}>
        {view?.provider ? (
          <>
            Using <b>{view.provider}</b> <span className="muted">key {view.key_hint}</span>
          </>
        ) : (
          <>No model key yet. Paste one below, or add <code>DEEPSEEK=…</code> to <code>.env</code>.</>
        )}
      </div>
      <label className="field">
        <span>DeepSeek key</span>
        <input type="password" placeholder="sk-…" value={ds} onChange={(e) => setDs(e.target.value)} onBlur={() => save({ deepseek_key: ds })} />
      </label>
      <label className="field">
        <span>Claude key (used first when set)</span>
        <input type="password" placeholder="sk-ant-…" value={cl} onChange={(e) => setCl(e.target.value)} onBlur={() => save({ anthropic_key: cl })} />
      </label>
      <label className="field">
        <span>Use</span>
        <select value={view?.settings.provider ?? ""} onChange={(e) => save({ provider: e.target.value })}>
          <option value="">Automatic (Claude if set, else DeepSeek)</option>
          <option value="deepseek">DeepSeek</option>
          <option value="claude">Claude</option>
        </select>
      </label>
      <div className="row-actions">
        <button
          className="btn btn--primary"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setTest(null);
            try {
              setTest({ ok: true, text: await api.providerTest() });
            } catch (e) {
              setTest({ ok: false, text: String(e) });
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "Testing…" : "Test the key"}
        </button>
        {test && <span className={test.ok ? "ok-text" : "bad-text"}>{test.text}</span>}
      </div>
      <p className="muted small">Keys stay on this Mac in openclicky's settings file. Files and screenshots are only sent when a question needs them.</p>
      <label className="toggle">
        <input type="checkbox" checked={view?.settings.share_screen ?? true} onChange={(e) => api.setSettings({ share_screen: e.target.checked }).then(onSaved)} />
        <span>Look at the screen when I ask something (sends a screenshot with voice questions)</span>
      </label>
    </div>
  );
}

function VoicePanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [voices, setVoices] = useState<Voice[]>([]);
  const [def, setDef] = useState("");
  const [vs, setVs] = useState<Awaited<ReturnType<typeof api.voiceStatus>> | null>(null);
  const [installMsg, setInstallMsg] = useState("");
  useEffect(() => {
    api.voices().then((v) => {
      setVoices(v.voices);
      setDef(v.default);
    }).catch(() => {});
    const load = () => api.voiceStatus().then(setVs).catch(() => {});
    load();
    const t = setInterval(load, 2500);
    const un = onEvent((e) => {
      if (e.kind !== "voice.install") return;
      const step = e.payload.step;
      setInstallMsg(step === "runtime" ? "Downloading the voice engine…" : step === "kokoro" ? "Downloading the voice (about 330 MB)…" : step === "done" ? "Ready." : `Couldn't download: ${e.payload.error}`);
      load();
    });
    return () => {
      clearInterval(t);
      un.then((f) => f());
    };
  }, []);
  const s = view?.settings;
  const natural = (s?.voice_engine ?? "kokoro") !== "apple";
  const lang = navigator.language.slice(0, 2);
  const mine = voices.filter((v) => v.language.startsWith(lang));
  const sorted = [...mine].sort((a, b) => ({ premium: 0, enhanced: 1, default: 2 })[a.quality] - ({ premium: 0, enhanced: 1, default: 2 })[b.quality]);
  const save = (patch: Partial<SettingsT>) => api.setSettings(patch).then(onSaved);
  return (
    <div className="stack-gap">
      <label className="toggle">
        <input type="checkbox" checked={s?.speak_answers ?? true} onChange={(e) => save({ speak_answers: e.target.checked })} />
        <span>Speak answers out loud</span>
      </label>
      <div className="seg-control">
        <button className={natural ? "is-on" : ""} onClick={() => save({ voice_engine: "kokoro" })}>
          Natural (Kokoro, on this Mac)
        </button>
        <button className={!natural ? "is-on" : ""} onClick={() => save({ voice_engine: "apple" })}>
          System voice
        </button>
      </div>
      {natural && vs && !vs.installed && (
        <div className="banner banner--warn">
          The natural voice isn't downloaded yet (about 350 MB, runs offline).
          <div className="row-actions" style={{ marginTop: 8 }}>
            <button className="btn btn--primary" disabled={vs.installing} onClick={() => api.voiceInstall().catch((e) => setInstallMsg(String(e)))}>
              {vs.installing ? "Downloading…" : "Download natural voice"}
            </button>
            {installMsg && <span className="muted small">{installMsg}</span>}
          </div>
        </div>
      )}
      {natural && vs?.installed && (
        <>
          <div className={`banner ${vs.ready ? "banner--ok" : "banner--muted"}`}>{vs.ready ? "Natural voice is ready." : "Natural voice is starting…"}</div>
          <div className="voices">
            {vs.voices.map((v) => (
              <button
                key={v.id}
                className={`voice ${s?.kokoro_voice === v.id ? "is-on" : ""}`}
                onClick={() => save({ kokoro_voice: v.id }).then(() => api.voiceTest(`Hi, I'm ${v.name}. I'll be right up here in your notch.`))}
              >
                <b>{v.name}</b>
                <span>{v.description}</span>
              </button>
            ))}
          </div>
          <label className="field">
            <span>Speed · {(s?.voice_speed ?? 1).toFixed(2)}×</span>
            <input type="range" min={0.8} max={1.3} step={0.05} value={s?.voice_speed ?? 1} onChange={(e) => save({ voice_speed: parseFloat(e.target.value) })} />
          </label>
        </>
      )}
      {!natural && (
        <>
          <label className="field">
            <span>Voice</span>
            <select value={s?.voice_id || ""} onChange={(e) => save({ voice_id: e.target.value })}>
              <option value="">Best available ({voices.find((v) => v.id === def)?.name ?? "system"})</option>
              {sorted.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name} · {v.language}
                  {v.quality !== "default" ? ` · ${v.quality}` : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>Speed</span>
            <input type="range" min={0.4} max={0.65} step={0.01} value={s?.speech_rate ?? 0.52} onChange={(e) => save({ speech_rate: parseFloat(e.target.value) })} />
          </label>
        </>
      )}
      <div className="row-actions">
        <button className="btn btn--primary" onClick={() => api.voiceTest()}>Hear it</button>
      </div>
      <label className="field">
        <span>Speech recognition language</span>
        <input placeholder={navigator.language} value={s?.locale ?? ""} onChange={(e) => save({ locale: e.target.value })} />
      </label>
      <label className="field">
        <span>What should I call you?</span>
        <input placeholder="Your name" value={s?.user_name ?? ""} onChange={(e) => save({ user_name: e.target.value })} />
      </label>
    </div>
  );
}

const SAMPLE_SOUNDS = ["agent-launch", "agent-done", "agent-needs-you", "badge-pop", "target-ping", "pen-flick", "tada", "sparkle", "whoosh", "nope"];

function SoundsPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [res, setRes] = useState<ResourcesReport | null>(null);
  useEffect(() => {
    api.resourcesReport().then(setRes).catch(() => {});
  }, []);
  return (
    <div className="stack-gap">
      <label className="toggle">
        <input type="checkbox" checked={!(view?.settings.muted ?? false)} onChange={(e) => api.setSettings({ muted: !e.target.checked }).then(onSaved)} />
        <span>Play sounds</span>
      </label>
      <div className="soundboard">
        {SAMPLE_SOUNDS.map((s) => (
          <button key={s} className="chip chip--tap" onClick={() => api.playSound(s)}>
            {s}
          </button>
        ))}
      </div>
      {res && (
        <p className="muted small">
          {res.sounds} sound slots from {res.pack === "local" ? "your local pack" : res.pack === "bundled" ? "the app" : res.pack}
          {res.missing.length > 0 && ` · ${res.missing.length} missing`}. Swap any sound by dropping a file into the slot (see resources/README.md).
        </p>
      )}
    </div>
  );
}

function ConnectorsPanel() {
  const [st, setSt] = useState<Awaited<ReturnType<typeof api.status>> | null>(null);
  useEffect(() => {
    api.status().then(setSt);
    const t = setInterval(() => api.status().then(setSt), 3000);
    return () => clearInterval(t);
  }, []);
  return (
    <div className="stack-gap">
      <div className={`banner ${st?.browser ? "banner--ok" : "banner--muted"}`}>
        <b>Chrome</b> {st?.browser ? "is connected." : "isn't connected yet."}
      </div>
      <ol className="steps">
        <li>Open <code>chrome://extensions</code> and switch on Developer mode.</li>
        <li>
          Click <b>Load unpacked</b> and pick the <code>extension</code> folder in openclicky's support folder.
          <button className="btn btn--link" onClick={() => st && api.openTarget(`${st.data_dir}/extension`)}>Show folder</button>
        </li>
        <li>That's it. openclicky can then read pages, click and type in your browser when you ask.</li>
      </ol>
      <div className="banner banner--muted">
        <b>Apps</b> {st?.connectors.length ? `connected: ${st.connectors.join(", ")}` : "— none yet."}
      </div>
      <p className="muted small">
        Connect apps (Gmail, Notion, GitHub…) with MCP servers: add them to <code>mcp.json</code> in the support folder and restart openclicky.
        <button className="btn btn--link" onClick={() => st && api.openTarget(`${st.data_dir}/mcp.json`)}>Open mcp.json</button>
      </p>
    </div>
  );
}

function AboutPanel() {
  const [st, setSt] = useState<Awaited<ReturnType<typeof api.status>> | null>(null);
  useEffect(() => {
    api.status().then(setSt);
  }, []);
  return (
    <div className="about">
      <Mark size={72} />
      <Wordmark />
      <p className="muted">Version {st?.version} · MIT licence · open source</p>
      <ul className="kv">
        <li><span>Model</span><b>{st?.provider ?? "not set"}</b></li>
        <li><span>Native helper</span><b>{st?.helper ? "running" : "missing"}</b></li>
        <li><span>Support folder</span><button className="btn btn--link" onClick={() => st && api.openTarget(st.data_dir)}>Show</button></li>
      </ul>
    </div>
  );
}

function Welcome({ view, onSaved, done }: { view: SettingsView | null; onSaved: (v: SettingsView) => void; done: () => void }) {
  const [step, setStep] = useState(0);
  const { perms, refresh, ready } = usePermissions();
  useEffect(() => {
    api.playSound("home-reveal").catch(() => {});
  }, []);
  return (
    <div className="welcome">
      {step === 0 && (
        <div className="welcome__hero">
          <Mark size={96} live />
          <h1>Hi, I'm openclicky.</h1>
          <p>I live up in your notch. Hold <kbd>⌃ control</kbd> + <kbd>⌥ option</kbd> and ask me anything: I can see your screen, point at things, and take bigger jobs off your hands.</p>
          <button className="btn btn--primary btn--big" onClick={() => setStep(1)}>Let's set up</button>
        </div>
      )}
      {step === 1 && (
        <div className="welcome__step">
          <h2>A few switches</h2>
          <p className="muted">These let me hear you, see the screen and point at buttons. Everything stays on this Mac unless a question needs a model.</p>
          <ul className="rows">
            {perms.map((p) => (
              <PermRow key={p.id} p={p} onChange={refresh} />
            ))}
          </ul>
          <div className="welcome__nav">
            <button className="btn btn--link" onClick={() => setStep(0)}>Back</button>
            <button className="btn btn--primary" onClick={() => setStep(2)}>{ready ? "Next" : "Skip for now"}</button>
          </div>
        </div>
      )}
      {step === 2 && (
        <div className="welcome__step">
          <h2>My brain</h2>
          <AiPanel view={view} onSaved={onSaved} />
          <div className="welcome__nav">
            <button className="btn btn--link" onClick={() => setStep(1)}>Back</button>
            <button className="btn btn--primary" onClick={() => setStep(3)}>Next</button>
          </div>
        </div>
      )}
      {step === 3 && (
        <div className="welcome__hero">
          <Mark size={80} />
          <h1>Try it now</h1>
          <p>Hold <kbd>⌃</kbd><kbd>⌥</kbd> and say: <span className="hand big-hand">“What's on my screen?”</span></p>
          <p className="muted">Or “where's the share button?”, “make me a one-page PDF about tea”, “summarise this page”. Hold <kbd>fn</kbd><kbd>⌃</kbd> to dictate into any text field.</p>
          <div className="welcome__nav welcome__nav--center">
            <button className="btn btn--ghost" onClick={() => api.voiceTest("Hi! I'm right up here in the notch whenever you need me.")}>Hear my voice</button>
            <button
              className="btn btn--primary btn--big"
              onClick={() => {
                api.onboardingDone();
                api.playSound("tada").catch(() => {});
                done();
              }}
            >
              Done
            </button>
          </div>
        </div>
      )}
      <div className="welcome__dots">
        {[0, 1, 2, 3].map((i) => (
          <i key={i} className={i === step ? "is-on" : ""} />
        ))}
      </div>
    </div>
  );
}

const TABS: { id: Tab; label: string }[] = [
  { id: "setup", label: "Permissions" },
  { id: "ai", label: "AI" },
  { id: "voice", label: "Voice" },
  { id: "sounds", label: "Sounds" },
  { id: "connectors", label: "Connectors" },
  { id: "about", label: "About" },
];

export function Settings() {
  const [tab, setTab] = useState<Tab>("setup");
  const [view, setView] = useState<SettingsView | null>(null);
  useEffect(() => {
    api.settings().then((v) => {
      setView(v);
      if (!v.settings.onboarded) setTab("welcome");
    });
    const un = onEvent((e) => {
      if (e.kind === "settings.tab") setTab(e.payload.tab);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  if (tab === "welcome") {
    return (
      <div className="settings settings--welcome" data-tauri-drag-region>
        <Welcome view={view} onSaved={setView} done={() => { setTab("setup"); getCurrentWindow().hide(); }} />
      </div>
    );
  }

  return (
    <div className="settings">
      <nav className="settings__nav" data-tauri-drag-region>
        <div className="settings__brand" data-tauri-drag-region>
          <Mark size={28} />
          <span className="wordmark">openclicky</span>
        </div>
        {TABS.map((t) => (
          <button key={t.id} className={`navbtn ${tab === t.id ? "is-on" : ""}`} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
        <div className="settings__spacer" />
        <button className="navbtn navbtn--muted" onClick={() => setTab("welcome")}>Show welcome</button>
      </nav>
      <section className="settings__body">
        <header className="settings__head" data-tauri-drag-region>
          <h1>{TABS.find((t) => t.id === tab)?.label}</h1>
        </header>
        <div className="settings__content">
          {tab === "setup" && <Permissions />}
          {tab === "ai" && <AiPanel view={view} onSaved={setView} />}
          {tab === "voice" && <VoicePanel view={view} onSaved={setView} />}
          {tab === "sounds" && <SoundsPanel view={view} onSaved={setView} />}
          {tab === "connectors" && <ConnectorsPanel />}
          {tab === "about" && <AboutPanel />}
        </div>
      </section>
    </div>
  );
}
