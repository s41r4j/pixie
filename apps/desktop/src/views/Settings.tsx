// Settings + first-run welcome: permissions, the model key, voice, sounds, connectors.
import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, onEvent, type PermissionInfo, type PermState, type ResourcesReport, type Settings as SettingsT, type SettingsView, type Voice } from "../lib/bridge";
import { Glyph, Mark, Wordmark } from "../components/Mark";
import { CodingPanel, ModelsPanel, UsagePanel } from "./SettingsModels";
import { chordLabel, Kbd, PRESETS, useShortcuts } from "../lib/keys";

type Tab = "welcome" | "general" | "setup" | "ai" | "usage" | "coding" | "voice" | "sounds" | "connectors" | "about";

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
        {(p.id === "screen" || p.id === "accessibility") && !granted && (
          <div className="row__hint">
            Turn Pixie on in System Settings
            {p.restart_hint && (
              <>
                , then{" "}
                <button className="btn--inline" onClick={() => api.relaunch()}>
                  relaunch
                </button>
              </>
            )}
            . Still off?{" "}
            <button
              className="btn--inline"
              onClick={async () => {
                await api.resetPermission(p.id).catch(console.error);
                onChange();
              }}
            >
              Reset
            </button>
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
                  if (p.id === "screen" || p.id === "accessibility") api.guideShow(p.id);
                } finally {
                  setBusy(false);
                  onChange();
                }
              }}
            >
              {p.state === "unchecked" ? "Check" : "Allow"}
            </button>
            <button
              className="btn btn--link"
              onClick={() => {
                api.openPermissionSettings(p.id);
                if (p.id === "screen" || p.id === "accessibility") api.guideShow(p.id);
              }}
            >
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

const PROVIDER_QUICK = [
  { id: "claude", name: "Claude", field: "anthropic_key" as const, placeholder: "sk-ant-…", where: "console.anthropic.com" },
  { id: "openai", name: "OpenAI", field: "openai_key" as const, placeholder: "sk-…", where: "platform.openai.com" },
  { id: "deepseek", name: "DeepSeek", field: "deepseek_key" as const, placeholder: "sk-…", where: "platform.deepseek.com" },
];

/** Onboarding: pick a provider, paste its key, test it. (Full control lives in Settings → Models.) */
function QuickBrain({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const current = view?.settings.provider || (view?.provider?.startsWith("Claude") ? "claude" : view?.provider?.startsWith("OpenAI") ? "openai" : view?.provider?.startsWith("DeepSeek") ? "deepseek" : "claude");
  const [pick, setPick] = useState(current);
  const p = PROVIDER_QUICK.find((x) => x.id === pick) ?? PROVIDER_QUICK[0];
  const [key, setKey] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  useEffect(() => setKey(view?.settings[p.field] ?? ""), [p.field, view]);
  return (
    <div className="stack-gap">
      <div className="choice-cards">
        {PROVIDER_QUICK.map((x) => (
          <button key={x.id} className={`choice ${pick === x.id ? "choice--on" : ""}`} onClick={() => setPick(x.id)}>
            <b>{x.name}</b>
            <span>{view?.settings[x.field] ? "key set" : `key from ${x.where}`}</span>
          </button>
        ))}
      </div>
      <label className="field">
        <span>{p.name} API key</span>
        <input type="password" placeholder={p.placeholder} value={key} onChange={(e) => setKey(e.target.value)} />
      </label>
      <div className="row-actions">
        <button
          className="btn btn--primary"
          disabled={!key.trim()}
          onClick={async () => {
            setTest({ ok: true, text: "checking…" });
            try {
              onSaved(await api.setSettings({ [p.field]: key.trim(), provider: p.id } as Partial<SettingsT>));
              setTest({ ok: true, text: await api.providerTest() });
            } catch (e) {
              setTest({ ok: false, text: String(e) });
            }
          }}
        >
          Save and test
        </button>
        {test && <span className={test.ok ? "ok-text" : "bad-text"}>{test.text}</span>}
      </div>
      <p className="muted small">Keys stay on this Mac. You can add the others and pick exact models later in Settings → Models.</p>
    </div>
  );
}

/** Live mic check: a level meter and the words as they're heard. */
function MicCheck() {
  const [on, setOn] = useState(false);
  const [levels, setLevels] = useState<number[]>(() => Array(24).fill(0));
  const [heard, setHeard] = useState("");
  const [err, setErr] = useState("");
  useEffect(() => {
    const un = onEvent((e) => {
      if (e.kind === "mic.level") setLevels((l) => [...l.slice(1), Math.min(1, e.payload.level * 1.2)]);
      if (e.kind === "mic.partial") setHeard(e.payload.text);
    });
    return () => {
      un.then((f) => f());
      api.micTestStop().catch(() => {});
    };
  }, []);
  const start = async () => {
    setErr("");
    setHeard("");
    try {
      await api.micTestStart();
      setOn(true);
    } catch (e) {
      setErr(String(e));
    }
  };
  const stop = async () => {
    setOn(false);
    const t = await api.micTestStop().catch(() => "");
    if (t) setHeard(t);
    setLevels(Array(24).fill(0));
  };
  return (
    <div className={`miccheck ${on ? "miccheck--on" : ""}`}>
      <div className="miccheck__meter" aria-hidden>
        {levels.map((l, i) => (
          <i key={i} style={{ height: `${6 + Math.round(l * 40)}px` }} />
        ))}
      </div>
      <div className="miccheck__heard">{heard ? `“${heard}”` : on ? "Listening… say “hi Pixie, what's on my screen?”" : "Press the button and say something."}</div>
      <button className={`btn ${on ? "" : "btn--primary"}`} onClick={on ? stop : start}>
        {on ? "Done" : "Test my mic"}
      </button>
      {err && <div className="bad-text small">{/permission|denied|not allowed/i.test(err) ? "Pixie needs Microphone and Speech Recognition (previous step)." : err}</div>}
    </div>
  );
}

function mb(n: number) {
  return `${Math.round(n / 1e6)} MB`;
}

/** Natural voice status with a real progress bar. Starts the download when asked (or `auto`). */
function VoiceDownload({ auto = false }: { auto?: boolean }) {
  const [vs, setVs] = useState<Awaited<ReturnType<typeof api.voiceStatus>> | null>(null);
  const [step, setStep] = useState("");
  const [err, setErr] = useState("");
  useEffect(() => {
    const load = () => api.voiceStatus().then((v) => {
      setVs(v);
      return v;
    });
    load().then((v) => {
      if (auto && v && !v.installed && !v.installing) api.voiceInstall().catch((e) => setErr(String(e)));
    }).catch(() => {});
    const t = setInterval(load, 1000);
    const un = onEvent((e) => {
      if (e.kind !== "voice.install") return;
      setStep(e.payload.step);
      if (e.payload.step === "error") setErr(String(e.payload.error));
      load();
    });
    return () => {
      clearInterval(t);
      un.then((f) => f());
    };
  }, [auto]);
  if (!vs) return null;
  if (vs.installed) return <div className={`banner ${vs.ready ? "banner--ok" : "banner--muted"}`}>{vs.ready ? "■ Natural voice is ready, on this Mac." : "Natural voice is starting…"}</div>;
  const pct = vs.total ? Math.min(100, Math.round((vs.done / vs.total) * 100)) : 0;
  return (
    <div className="banner banner--warn vdl">
      {vs.installing ? (
        <>
          <div className="vdl__row">
            <b>{step === "unpack" ? "Unpacking…" : "Downloading the natural voice"}</b>
            <span>{vs.total ? `${mb(vs.done)} of ${mb(vs.total)} · ${pct}%` : "starting…"}</span>
          </div>
          <div className="vdl__bar"><i style={{ width: `${pct}%` }} /></div>
          <span className="muted small">Pixie uses the system voice until it's done. You can keep going.</span>
        </>
      ) : (
        <>
          <div className="vdl__row">
            <b>The natural voice isn't downloaded yet</b>
            <span>about 370 MB, runs offline</span>
          </div>
          <span className="muted small">Until then Pixie speaks with the system voice.</span>
          <div className="row-actions">
            <button className="btn btn--primary" onClick={() => api.voiceInstall().catch((e) => setErr(String(e)))}>Download natural voice</button>
            {err && <span className="bad-text small">{err}</span>}
          </div>
        </>
      )}
    </div>
  );
}

function VoicePanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [voices, setVoices] = useState<Voice[]>([]);
  const [def, setDef] = useState("");
  const [vs, setVs] = useState<Awaited<ReturnType<typeof api.voiceStatus>> | null>(null);
  useEffect(() => {
    api.voices().then((v) => {
      setVoices(v.voices);
      setDef(v.default);
    }).catch(() => {});
    const load = () => api.voiceStatus().then(setVs).catch(() => {});
    load();
    const t = setInterval(load, 2500);
    return () => clearInterval(t);
  }, []);
  const s = view?.settings;
  const natural = (s?.voice_engine ?? "kokoro") !== "apple";
  const lang = navigator.language.slice(0, 2);
  const mine = voices.filter((v) => v.language.startsWith(lang));
  const sorted = [...mine].sort((a, b) => ({ premium: 0, enhanced: 1, default: 2 })[a.quality] - ({ premium: 0, enhanced: 1, default: 2 })[b.quality]);
  const save = (patch: Partial<SettingsT>) => api.setSettings(patch).then(onSaved);
  return (
    <div className="stack-gap">
      <div className="choice-cards choice-cards--2" data-sid="voice_engine">
        <button className={`choice ${natural ? "choice--on" : ""}`} onClick={() => save({ voice_engine: "kokoro" })}>
          <b>Natural voice</b>
          <span>{vs?.installed ? (vs.ready ? "Ready · runs on this Mac, offline" : "Starting…") : vs?.installing ? "Downloading…" : "Not downloaded yet"}</span>
        </button>
        <button className={`choice ${!natural ? "choice--on" : ""}`} onClick={() => save({ voice_engine: "apple" })}>
          <b>System voice</b>
          <span>The macOS voice you pick below. Nothing to download.</span>
        </button>
      </div>
      {natural && <VoiceDownload />}
      {natural && vs?.installed && (
        <>
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
          <label className="field" data-sid="voice_speed">
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
          <label className="field" data-sid="voice_speed">
            <span>Speed</span>
            <input type="range" min={0.4} max={0.65} step={0.01} value={s?.speech_rate ?? 0.52} onChange={(e) => save({ speech_rate: parseFloat(e.target.value) })} />
          </label>
        </>
      )}
      <div className="row-actions">
        <button className="btn btn--primary" onClick={() => api.voiceTest()}>Hear it</button>
      </div>
      <h3 className="sub">Listening</h3>
      <MicCheck />
      <label className="field" data-sid="locale">
        <span>Speech recognition language</span>
        <input placeholder={navigator.language} value={s?.locale ?? ""} onChange={(e) => save({ locale: e.target.value })} />
      </label>
    </div>
  );
}

/** Every sound Pixie plays, what it's for, grouped. */
const SOUND_GROUPS: { title: string; sounds: [string, string][] }[] = [
  { title: "Notch", sounds: [["agent-launch", "The notch opens"], ["agent-close", "The notch closes"], ["type-keys", "Dictation types your words"], ["nope", "Something went wrong"]] },
  { title: "Agents", sounds: [["paper-place", "An agent starts"], ["badge-pop", "A step is done"], ["agent-needs-you", "An agent has a question"], ["agent-done", "An agent is finished"]] },
  { title: "Buddy & marks", sounds: [["target-ping", "Pointing at something"], ["whoosh", "The buddy flies to a spot"], ["gel-pop", "The buddy clicks"], ["pen-flick", "Drawing a circle or arrow"], ["highlighter", "Underlining or boxing"], ["stamp", "A tick mark"]] },
  { title: "Board & setup", sounds: [["pencil-paper-bed", "Sketching on the board (loops)"], ["home-reveal", "The welcome window"], ["tada", "Setup finished"]] },
];

function fileToBase64(f: File): Promise<string> {
  return new Promise((ok, bad) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).split(",")[1] ?? "");
    r.onerror = () => bad(r.error);
    r.readAsDataURL(f);
  });
}

function SoundRow({ slot, what, view, onSaved }: { slot: string; what: string; view: SettingsView; onSaved: (v: SettingsView) => void }) {
  const pref = view.settings.sounds?.[slot] ?? { volume: 1, muted: false, custom: "" };
  const [vol, setVol] = useState(pref.volume);
  const [err, setErr] = useState("");
  const file = useRef<HTMLInputElement>(null);
  useEffect(() => setVol(pref.volume), [pref.volume]);
  const save = (p: Partial<typeof pref>) => api.setSettings({ sounds: { ...(view.settings.sounds ?? {}), [slot]: { ...pref, ...p } } }).then(onSaved);
  const customName = pref.custom ? pref.custom.split("/").pop() : "";
  return (
    <li className={`row sound ${pref.muted ? "sound--muted" : ""}`} data-sid={`sound-${slot}`}>
      <button className="sound__play" title="Play" onClick={() => api.playSound(slot)} disabled={pref.muted}>▶</button>
      <div className="row__text sound__text">
        <div className="row__label">{what}</div>
        <div className="row__why">
          {customName ? <>your file · {customName}</> : <>default · {slot}</>}
          {err && <span className="bad-text"> · {err}</span>}
        </div>
      </div>
      <input
        className="sound__vol"
        type="range"
        min={0}
        max={1.5}
        step={0.05}
        value={vol}
        disabled={pref.muted}
        title={`${Math.round(vol * 100)}%`}
        onChange={(e) => setVol(parseFloat(e.target.value))}
        onPointerUp={() => save({ volume: vol }).then(() => api.playSound(slot))}
        onKeyUp={() => save({ volume: vol })}
      />
      <span className="sound__pct">{Math.round(vol * 100)}%</span>
      <div className="sound__src">
        <button className="btn btn--ghost btn--sm" onClick={() => file.current?.click()}>{customName ? "Change" : "Custom…"}</button>
        {customName && (
          <button className="btn btn--link btn--sm" onClick={() => api.soundCustom(slot, "", "").then(onSaved)}>Default</button>
        )}
        <input
          ref={file}
          type="file"
          accept=".wav,.mp3,.flac,.ogg,audio/*"
          hidden
          onChange={async (e) => {
            const f = e.target.files?.[0];
            e.target.value = "";
            if (!f) return;
            setErr("");
            try {
              onSaved(await api.soundCustom(slot, f.name, await fileToBase64(f)));
            } catch (x) {
              setErr(String(x));
            }
          }}
        />
      </div>
      <Switch label={`Play “${what}”`} on={!pref.muted} onChange={(v) => save({ muted: !v })} />
    </li>
  );
}

function SoundsPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const [res, setRes] = useState<ResourcesReport | null>(null);
  const [master, setMaster] = useState(view?.settings.master_volume ?? 1);
  useEffect(() => {
    api.resourcesReport().then(setRes).catch(() => {});
  }, []);
  useEffect(() => setMaster(view?.settings.master_volume ?? 1), [view?.settings.master_volume]);
  if (!view) return null;
  const on = !view.settings.muted;
  return (
    <div className="stack-gap stack-gap--wide">
      <ul className="rows">
        <SRow sid="sounds_on" label="Play sounds" why="Off silences every sound below (the voice still speaks).">
          <Switch label="Play sounds" on={on} onChange={(v) => api.setSettings({ muted: !v }).then(onSaved)} />
        </SRow>
        <SRow sid="master_volume" label={`Overall volume · ${Math.round(master * 100)}%`} why="Scales every sound; each one also has its own level.">
          <input
            className="sound__vol sound__vol--wide"
            type="range"
            min={0}
            max={1.5}
            step={0.05}
            value={master}
            disabled={!on}
            onChange={(e) => setMaster(parseFloat(e.target.value))}
            onPointerUp={() => api.setSettings({ master_volume: master }).then((v) => { onSaved(v); api.playSound("badge-pop"); })}
          />
        </SRow>
      </ul>
      {SOUND_GROUPS.map((g) => (
        <div key={g.title} className={on ? "" : "is-dim"}>
          <h3 className="sub">{g.title}</h3>
          <ul className="rows sounds">
            {g.sounds.map(([slot, what]) => (
              <SoundRow key={slot} slot={slot} what={what} view={view} onSaved={onSaved} />
            ))}
          </ul>
        </div>
      ))}
      <p className="muted small">
        Custom sounds: .wav, .mp3, .flac or .ogg under 5 MB, kept in Pixie's support folder.
        {res && ` ${res.sounds} sounds from ${res.pack === "local" ? "your local pack" : "the app"}${res.missing.length ? `, ${res.missing.length} missing` : ""}.`}
      </p>
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
          Click <b>Load unpacked</b> and pick the <code>extension</code> folder in Pixie's support folder.
          <button className="btn btn--link" onClick={() => st && api.openTarget(`${st.data_dir}/extension`)}>Show folder</button>
        </li>
        <li>That's it. Pixie can then read pages, click and type in your browser when you ask.</li>
      </ol>
      <div className="banner banner--muted">
        <b>Apps</b> {st?.connectors.length ? `connected: ${st.connectors.join(", ")}` : "— none yet."}
      </div>
      <p className="muted small">
        Connect apps (Gmail, Notion, GitHub…) with MCP servers: add them to <code>mcp.json</code> in the support folder and restart Pixie.
        <button className="btn btn--link" onClick={() => st && api.openTarget(`${st.data_dir}/mcp.json`)}>Open mcp.json</button>
      </p>
    </div>
  );
}

function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} data-tip={`${label}: ${on ? "on" : "off"}. Click to turn it ${on ? "off" : "on"}.`} className={`switch ${on ? "switch--on" : ""}`} onClick={() => onChange(!on)}>
      <i />
    </button>
  );
}

function SRow({ sid, label, why, children }: { sid: string; label: string; why?: string; children: React.ReactNode }) {
  return (
    <li className="row" data-sid={sid}>
      <div className="row__text">
        <div className="row__label">{label}</div>
        {why && <div className="row__why">{why}</div>}
      </div>
      <div className="row__control">{children}</div>
    </li>
  );
}

const BUDDY_MODES: { id: "always" | "idle" | "off"; label: string; why: string }[] = [
  { id: "always", label: "Always on", why: "Rides next to your pointer all the time." },
  { id: "idle", label: "Idle mode", why: "Invisible until Pixie listens, thinks, talks, points or works on your screen; then it pops in and fades away after." },
  { id: "off", label: "Off", why: "No buddy. Pixie still points and draws with marks." },
];

function GeneralPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const s = view?.settings;
  const save = (patch: Partial<SettingsT>) => api.setSettings(patch).then(onSaved);
  const mode = s?.buddy === false ? "off" : (s?.buddy_mode ?? "always");
  return (
    <div className="stack-gap">
      <h3 className="sub">Buddy</h3>
      <div className="choice-cards" data-sid="buddy_mode">
        {BUDDY_MODES.map((m) => (
          <button key={m.id} className={`choice ${mode === m.id ? "choice--on" : ""}`} onClick={() => save({ buddy: true, buddy_mode: m.id })}>
            <span className={`choice__demo choice__demo--${m.id}`}>
              <span className="choice__cursor"><Glyph name="cursor" size={18} /></span>
              <i className="choice__buddy" />
            </span>
            <b>{m.label}</b>
            <span>{m.why}</span>
          </button>
        ))}
      </div>
      <h3 className="sub">Shortcuts</h3>
      <ShortcutRows view={view} onSaved={onSaved} />
      <h3 className="sub">Doing things</h3>
      <ul className="rows">
        <SRow sid="can_click" label="Let Pixie click and type for me" why="When you ask it to do something, the buddy clicks, types, drags and copies one step at a time, and live agents work on your screen. Off: it only points and tells you where.">
          <Switch label="Let Pixie click" on={s?.can_click ?? true} onChange={(v) => save({ can_click: v })} />
        </SRow>
        <SRow sid="share_screen" label="Look at the screen when I ask" why="Sends a screenshot with voice questions so Pixie can see and point.">
          <Switch label="Look at the screen" on={s?.share_screen ?? true} onChange={(v) => save({ share_screen: v })} />
        </SRow>
      </ul>
      <h3 className="sub">Answers</h3>
      <ul className="rows">
        <SRow sid="speak_answers" label="Speak answers out loud" why="Voice answers are spoken and never shown as text; the notch just widens while Pixie talks. Past answers are in the notch's Chat tab.">
          <Switch label="Speak answers" on={s?.speak_answers ?? true} onChange={(v) => save({ speak_answers: v })} />
        </SRow>
        <SRow sid="user_name" label="What should I call you?">
          <input className="inline-input" placeholder="Your name" defaultValue={s?.user_name ?? ""} onBlur={(e) => save({ user_name: e.target.value })} />
        </SRow>
      </ul>
    </div>
  );
}

const LINKS: [string, string, string][] = [
  ["GitHub", "github.com/s41r4j", "https://github.com/s41r4j"],
  ["X", "x.com/s41r4j", "https://x.com/s41r4j"],
  ["Source", "github.com/s41r4j/pixie", "https://github.com/s41r4j/pixie"],
  ["Website", "pixie.s41r4j.in", "https://pixie.s41r4j.in"],
];

function AboutPanel() {
  const [st, setSt] = useState<Awaited<ReturnType<typeof api.status>> | null>(null);
  useEffect(() => {
    api.status().then(setSt);
  }, []);
  return (
    <div className="stack-gap">
      <div className="about-card">
        <Mark size={64} />
        <div>
          <Wordmark />
          <p className="muted">Version {st?.version} · free and open source (MIT)</p>
        </div>
      </div>
      <h3 className="sub">Made by</h3>
      <div className="dev-card">
        <div className="dev-card__name">s41r4j</div>
        <div className="dev-card__links">
          {LINKS.map(([k, label, url]) => (
            <button key={k} className="dev-link" onClick={() => api.openTarget(url)}>
              <span>{k}</span>
              <b>{label}</b>
            </button>
          ))}
        </div>
        <p className="muted small">Ideas, bugs and pull requests are welcome on GitHub.</p>
      </div>
      <h3 className="sub">This Mac</h3>
      <ul className="kv">
        <li><span>Model</span><b>{st?.provider ?? "not set"}</b></li>
        <li><span>Native helper</span><b>{st?.helper ? "running" : "missing"}</b></li>
        <li><span>Support folder</span><button className="btn btn--link" onClick={() => st && api.openTarget(st.data_dir)}>Show</button></li>
      </ul>
    </div>
  );
}

/** Pick a modifier chord: presets (fn can only be picked, not recorded) or record one. */
function ShortcutPicker({ value, other, onPick }: { value: string; other: string; onPick: (c: string) => void }) {
  const [rec, setRec] = useState(false);
  useEffect(() => {
    if (!rec) return;
    const down = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape") return setRec(false);
      const parts = [e.ctrlKey && "ctrl", e.altKey && "option", e.shiftKey && "shift", e.metaKey && "cmd"].filter(Boolean) as string[];
      if (parts.length >= 2) {
        const c = parts.join("+");
        if (chordLabel(c) !== chordLabel(other)) onPick(c);
        setRec(false);
      }
    };
    window.addEventListener("keydown", down);
    return () => window.removeEventListener("keydown", down);
  }, [rec, other, onPick]);
  return (
    <div className="kpick">
      <div className={`kpick__now ${rec ? "kpick__now--rec" : ""}`}>{rec ? <span>Hold the keys…</span> : <Kbd chord={value} />}</div>
      <div className="kpick__presets">
        {PRESETS.filter((p) => chordLabel(p) !== chordLabel(other)).map((p) => (
          <button key={p} className={`tagbox kpick__chip ${chordLabel(p) === chordLabel(value) ? "tagbox--on" : ""}`} onClick={() => onPick(p)}>
            {chordLabel(p)}
          </button>
        ))}
        <button className="btn btn--ghost btn--sm" onClick={() => setRec(!rec)}>{rec ? "Cancel" : "Record"}</button>
      </div>
    </div>
  );
}

function ShortcutRows({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const s = view?.settings;
  const talk = s?.shortcut_talk || "ctrl+option";
  const dictate = s?.shortcut_dictate || "fn+ctrl";
  return (
    <ul className="rows">
      <SRow sid="shortcut_talk" label="Talk to Pixie" why="Hold to talk, let go to send. A quick tap opens the text box.">
        <ShortcutPicker value={talk} other={dictate} onPick={(c) => api.setSettings({ shortcut_talk: c }).then(onSaved)} />
      </SRow>
      <SRow sid="shortcut_dictate" label="Dictate" why="Hold and speak: clean text is typed into whatever field has focus.">
        <ShortcutPicker value={dictate} other={talk} onPick={(c) => api.setSettings({ shortcut_dictate: c }).then(onSaved)} />
      </SRow>
      <SRow sid="shortcut_esc" label="Stop" why="Stops speaking, the answer and any live agent, and closes the notch.">
        <kbd>esc</kbd>
      </SRow>
    </ul>
  );
}

const STEPS = ["Hi", "You", "Switches", "Brain", "Voice", "Keys", "Go"];

function Welcome({ view, onSaved, done }: { view: SettingsView | null; onSaved: (v: SettingsView) => void; done: () => void }) {
  const [step, setStep] = useState(0);
  const [name, setName] = useState("");
  const { perms, refresh, ready } = usePermissions();
  const keys = useShortcuts();
  useEffect(() => {
    api.playSound("home-reveal").catch(() => {});
  }, []);
  useEffect(() => setName(view?.settings.user_name ?? ""), [view?.settings.user_name]);
  const nav = (back: number, next: number, label = "Next", skip?: () => void) => (
    <div className="welcome__nav">
      <button className="btn btn--link" onClick={() => setStep(back)}>Back</button>
      <span className="welcome__navr">
        {skip && <button className="btn btn--link" onClick={skip}>Skip</button>}
        <button className="btn btn--primary" onClick={() => setStep(next)}>{label}</button>
      </span>
    </div>
  );
  return (
    <div className="welcome">
      <div className="welcome__steps">
        {STEPS.map((t, i) => (
          <span key={t} className={i === step ? "is-on" : i < step ? "is-done" : ""}>{t}</span>
        ))}
      </div>
      {step === 0 && (
        <div className="welcome__hero">
          <Mark size={96} live />
          <h1>Hi, I'm Pixie.</h1>
          <p>I live up in your notch. Hold <Kbd chord={keys.talk} /> and ask me anything: I see your screen, point at things, do things for you, and take bigger jobs off your hands.</p>
          {/* The natural voice starts downloading now, so it's ready by the voice step. */}
          <VoiceDownloadSilent />
          <button className="btn btn--primary btn--big" onClick={() => setStep(1)}>Let's set up</button>
        </div>
      )}
      {step === 1 && (
        <div className="welcome__step">
          <h2>What should I call you?</h2>
          <p className="muted">I'll use it when I talk to you. You can change it any time in Settings → General.</p>
          <label className="field">
            <span>Your name</span>
            <input autoFocus placeholder="e.g. Sam" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && api.setSettings({ user_name: name.trim() }).then((v) => { onSaved(v); setStep(2); })} />
          </label>
          <div className="welcome__nav">
            <button className="btn btn--link" onClick={() => setStep(0)}>Back</button>
            <span className="welcome__navr">
              <button className="btn btn--link" onClick={() => setStep(2)}>Skip</button>
              <button className="btn btn--primary" disabled={!name.trim()} onClick={() => api.setSettings({ user_name: name.trim() }).then((v) => { onSaved(v); setStep(2); })}>Next</button>
            </span>
          </div>
        </div>
      )}
      {step === 2 && (
        <div className="welcome__step">
          <h2>A few switches</h2>
          <p className="muted">These let me hear you, see the screen and point at buttons. Everything stays on this Mac unless a question needs a model.</p>
          <ul className="rows">
            {perms.map((p) => (
              <PermRow key={p.id} p={p} onChange={refresh} />
            ))}
          </ul>
          {nav(1, 3, ready ? "Next" : "Skip for now")}
        </div>
      )}
      {step === 3 && (
        <div className="welcome__step">
          <h2>My brain</h2>
          <p className="muted">Pick who answers and paste a key.</p>
          <QuickBrain view={view} onSaved={onSaved} />
          {nav(2, 4)}
        </div>
      )}
      {step === 4 && (
        <div className="welcome__step">
          <h2>Can we hear each other?</h2>
          <p className="muted">First you: test the mic and watch your words appear. Then me.</p>
          <MicCheck />
          <VoiceDownload auto />
          <div className="row-actions">
            <button className="btn btn--ghost" onClick={() => api.voiceTest(`Hi${name.trim() ? ` ${name.trim()}` : ""}! I'm right up here in the notch whenever you need me.`)}>Hear my voice</button>
          </div>
          {nav(3, 5)}
        </div>
      )}
      {step === 5 && (
        <div className="welcome__step">
          <h2>The keys</h2>
          <p className="muted">Everything else is talking. Change these any time in Settings → General.</p>
          <ShortcutRows view={view} onSaved={onSaved} />
          <ul className="tips">
            <li><b>Hover the notch</b> · Chat (this conversation) and Agents (work in the background or on your screen)</li>
            <li><b>“Where's…?” “Circle the total”</b> · the buddy points and draws; marks fade on their own</li>
            <li><b>“Do this for me”</b> · it clicks, types and copies; bigger tasks become a live agent you can watch</li>
            <li><b>Click anywhere</b> · puts a board diagram away</li>
          </ul>
          {nav(4, 6)}
        </div>
      )}
      {step === 6 && (
        <div className="welcome__hero">
          <Mark size={80} />
          <h1>Try it now</h1>
          <p>Hold <Kbd chord={keys.talk} /> and say: <span className="hand big-hand">“What's on my screen?”</span></p>
          <p className="muted">Or “where's the share button?”, “make me a one-page PDF about tea”, “rename these files for me”. Hold <Kbd chord={keys.dictate} /> to dictate into any text field.</p>
          <div className="welcome__nav welcome__nav--center">
            <button className="btn btn--link" onClick={() => setStep(5)}>Back</button>
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
    </div>
  );
}

/** Kick off the natural-voice download quietly on the first screen. */
function VoiceDownloadSilent() {
  useEffect(() => {
    api
      .voiceStatus()
      .then((v) => {
        if (!v.installed && !v.installing) api.voiceInstall().catch(() => {});
      })
      .catch(() => {});
  }, []);
  return null;
}

type TabDef = { id: Tab; label: string; icon: string; blurb: string };
const GROUPS: { title: string; tabs: TabDef[] }[] = [
  {
    title: "Pixie",
    tabs: [
      { id: "general", label: "General", icon: "sliders", blurb: "The buddy, clicking for you, and how answers come back." },
      { id: "voice", label: "Voice", icon: "speaker", blurb: "How Pixie sounds and which language it listens for." },
      { id: "sounds", label: "Sounds", icon: "waveform", blurb: "Little sounds for starts, finishes and nudges." },
    ],
  },
  {
    title: "Brains",
    tabs: [
      { id: "ai", label: "Models", icon: "sparkle", blurb: "Claude, OpenAI or DeepSeek: live model lists, newest first, with prices." },
      { id: "usage", label: "Usage", icon: "grid", blurb: "Tokens and cost per model, priced from the live price list." },
      { id: "coding", label: "Coding", icon: "terminal", blurb: "Which coding agent on this Mac takes code tasks, and with which model." },
    ],
  },
  {
    title: "Mac",
    tabs: [
      { id: "setup", label: "Permissions", icon: "lock", blurb: "What Pixie may hear, see and do on this Mac." },
      { id: "connectors", label: "Connectors", icon: "layers", blurb: "Your browser and apps over MCP." },
      { id: "about", label: "About", icon: "eye", blurb: "Version and where Pixie keeps its files." },
    ],
  },
];
const TABS = GROUPS.flatMap((g) => g.tabs);

/** Every setting, for search. `sid` matches a data-sid in the panel so it can be found and flashed. */
const INDEX: { tab: Tab; sid?: string; title: string; words: string }[] = [
  { tab: "general", sid: "buddy_mode", title: "Buddy: always on, idle mode, off", words: "pointer cursor follow character companion hide invisible idle" },
  { tab: "general", sid: "can_click", title: "Let Pixie click and type for me", words: "computer use control mouse keyboard type copy clipboard live agent pointer only" },
  { tab: "general", sid: "share_screen", title: "Look at the screen when I ask", words: "screenshot privacy see vision" },
  { tab: "general", sid: "speak_answers", title: "Speak answers out loud", words: "voice text mute talk" },
  { tab: "general", sid: "user_name", title: "What should I call you?", words: "name" },
  { tab: "general", sid: "shortcut_talk", title: "Talk shortcut", words: "hotkey keys keyboard shortcut ctrl option hold push to talk" },
  { tab: "general", sid: "shortcut_dictate", title: "Dictate shortcut", words: "hotkey keys keyboard shortcut fn dictation" },
  { tab: "voice", sid: "locale", title: "Test my mic", words: "microphone check hear listen" },
  { tab: "sounds", sid: "master_volume", title: "Overall volume", words: "loud quiet master" },
  { tab: "sounds", title: "Custom sounds", words: "file replace change own default" },
  { tab: "ai", title: "Fallback model", words: "backup failover second model outage" },
  { tab: "about", title: "Made by s41r4j", words: "developer author github x twitter credits" },
  { tab: "voice", title: "Natural voice (Kokoro) or system voice", words: "tts speech speaker download kokoro" },
  { tab: "voice", title: "Voice speed", words: "rate fast slow" },
  { tab: "voice", title: "Speech recognition language", words: "locale language dictation listen" },
  { tab: "sounds", title: "Play sounds", words: "mute audio effects" },
  { tab: "ai", title: "Claude key and models", words: "anthropic api key opus sonnet haiku fable" },
  { tab: "ai", title: "OpenAI key and models", words: "gpt api key chatgpt" },
  { tab: "ai", title: "DeepSeek key and models", words: "api key flash pro" },
  { tab: "ai", title: "Voice model and agents model", words: "fast deep jobs choose pick latest newest context window" },
  { tab: "usage", title: "Tokens and cost", words: "spend money price bill analytics tokens per day" },
  { tab: "usage", title: "Refresh prices", words: "openrouter price list cost per million" },
  { tab: "coding", title: "Coding agent: Codex, Claude Code, opencode", words: "code harness t3 local agent model sandbox permissions folder" },
  { tab: "setup", title: "Microphone, Speech, Screen Recording, Accessibility", words: "permission privacy allow reset tcc" },
  { tab: "connectors", title: "Chrome extension", words: "browser tabs extension" },
  { tab: "connectors", title: "Apps over MCP", words: "mcp gmail notion github connectors" },
  { tab: "about", title: "Version and support folder", words: "about version folder logs" },
  { tab: "usage", title: "Filter by provider or voice/agents", words: "filter all time total spent" },
];

function search(q: string) {
  const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [];
  return INDEX.map((e) => {
    const tab = TABS.find((t) => t.id === e.tab)!;
    const hay = `${e.title} ${e.words} ${tab.label}`.toLowerCase();
    const score = terms.every((t) => hay.includes(t)) ? terms.reduce((n, t) => n + (e.title.toLowerCase().includes(t) ? 2 : 1), 0) : 0;
    return { e, tab, score };
  })
    .filter((r) => r.score > 0)
    .sort((a, b) => b.score - a.score);
}

export function Settings() {
  const [tab, setTab] = useState<Tab>("general");
  const [view, setView] = useState<SettingsView | null>(null);
  const [q, setQ] = useState("");
  const [flash, setFlash] = useState<string | null>(null);
  const searchBox = useRef<HTMLInputElement>(null);
  useEffect(() => {
    api.settings().then((v) => {
      setView(v);
      if (!v.settings.onboarded) setTab("welcome");
    });
    const un = onEvent((e) => {
      if (e.kind === "settings.tab") setTab(e.payload.tab);
    });
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey && (e.key === "f" || e.key === "k")) || (e.key === "/" && document.activeElement?.tagName !== "INPUT")) {
        e.preventDefault();
        searchBox.current?.focus();
        searchBox.current?.select();
      }
      if (e.key === "Escape" && document.activeElement === searchBox.current) setQ("");
    };
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  // After jumping to a setting, scroll it into view and flash it.
  useEffect(() => {
    if (!flash) return;
    const t = setTimeout(() => {
      const el = document.querySelector(`[data-sid="${flash}"]`);
      el?.scrollIntoView({ block: "center", behavior: "smooth" });
      el?.classList.add("flash");
      setTimeout(() => el?.classList.remove("flash"), 1400);
      setFlash(null);
    }, 80);
    return () => clearTimeout(t);
  }, [flash, tab]);

  if (tab === "welcome") {
    return (
      <div className="settings settings--welcome" data-tauri-drag-region>
        <Welcome view={view} onSaved={setView} done={() => { setTab("setup"); getCurrentWindow().hide(); }} />
      </div>
    );
  }

  const results = search(q);
  const go = (t: Tab, sid?: string) => {
    setTab(t);
    setQ("");
    if (sid) setFlash(sid);
  };
  const cur = TABS.find((t) => t.id === tab);

  return (
    <div className="settings">
      <nav className="settings__nav" data-tauri-drag-region>
        <div className="settings__brand" data-tauri-drag-region>
          <Mark size={26} />
          <span className="wordmark">Pixie</span>
        </div>
        <label className="ssearch">
          <Glyph name="eye" size={13} />
          <input
            data-tip="Find any setting. ⌘F from anywhere in Settings."
            ref={searchBox}
            value={q}
            placeholder="Search settings"
            spellCheck={false}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && results[0]) go(results[0].e.tab, results[0].e.sid);
            }}
          />
          <kbd>⌘F</kbd>
        </label>
        {GROUPS.map((g) => (
          <div key={g.title} className="navgroup">
            <div className="navgroup__title">{g.title}</div>
            {g.tabs.map((t) => (
              <button key={t.id} className={`navbtn ${tab === t.id && !q ? "is-on" : ""}`} onClick={() => go(t.id)} data-tip={t.blurb}>
                <Glyph name={t.icon} size={14} />
                {t.label}
              </button>
            ))}
          </div>
        ))}
        <div className="settings__spacer" />
        <button className="navbtn navbtn--muted" onClick={() => setTab("welcome")}>Show welcome</button>
      </nav>
      <section className="settings__body">
        {q ? (
          <>
            <header className="settings__head" data-tauri-drag-region>
              <h1>Search</h1>
              <p className="settings__blurb">{results.length ? `${results.length} match${results.length === 1 ? "" : "es"} for “${q}”` : `Nothing matches “${q}”.`}</p>
            </header>
            <div className="settings__content">
              <ul className="sresults">
                {results.map(({ e, tab: t }, i) => (
                  <li key={i}>
                    <button className={`sresult ${i === 0 ? "sresult--first" : ""}`} onClick={() => go(e.tab, e.sid)}>
                      <Glyph name={t.icon} size={14} />
                      <span className="sresult__title">{e.title}</span>
                      <span className="sresult__where">{t.label}</span>
                      {i === 0 && <kbd>↵</kbd>}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          </>
        ) : (
          <>
            <header className="settings__head" data-tauri-drag-region>
              <h1>{cur?.label}</h1>
              {cur?.blurb && <p className="settings__blurb">{cur.blurb}</p>}
            </header>
            <div className="settings__content">
              {tab === "general" && <GeneralPanel view={view} onSaved={setView} />}
              {tab === "setup" && <Permissions />}
              {tab === "ai" && <ModelsPanel view={view} onSaved={setView} />}
              {tab === "usage" && <UsagePanel view={view} onSaved={setView} />}
              {tab === "coding" && <CodingPanel view={view} onSaved={setView} />}
              {tab === "voice" && <VoicePanel view={view} onSaved={setView} />}
              {tab === "sounds" && <SoundsPanel view={view} onSaved={setView} />}
              {tab === "connectors" && <ConnectorsPanel />}
              {tab === "about" && <AboutPanel />}
            </div>
          </>
        )}
      </section>
    </div>
  );
}
