import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type PermissionInfo, type PermState, type ResourcesReport } from "../lib/bridge";

const PILL: Record<PermState, { text: string; tone: "ok" | "warn" | "muted" }> = {
  granted: { text: "Allowed", tone: "ok" },
  denied: { text: "Off", tone: "warn" },
  restricted: { text: "Blocked by policy", tone: "warn" },
  not_determined: { text: "Not asked yet", tone: "muted" },
  not_granted: { text: "Not allowed", tone: "warn" },
  unchecked: { text: "Not checked", tone: "muted" },
};

function Row({ p, onChange }: { p: PermissionInfo; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  const pill = PILL[p.state];
  const granted = p.state === "granted";

  const allow = async () => {
    setBusy(true);
    try {
      await api.requestPermission(p.id);
    } finally {
      setBusy(false);
      onChange();
    }
  };

  return (
    <li className={`perm ${granted ? "perm--ok" : ""}`}>
      <div className="perm__text">
        <div className="perm__label">
          {p.label}
          {!p.required && <span className="perm__opt">optional</span>}
        </div>
        <div className="perm__why">{p.why}</div>
        {p.restart_hint && !granted && (
          <div className="perm__hint">After you switch it on, quit and reopen openclicky.</div>
        )}
      </div>
      <div className="perm__side">
        <span className={`pill pill--${pill.tone}`}>{pill.text}</span>
        {!granted && (
          <div className="perm__actions">
            <button className="btn btn--primary" disabled={busy} onClick={allow}>
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

export function Permissions() {
  const [perms, setPerms] = useState<PermissionInfo[]>([]);
  const [res, setRes] = useState<ResourcesReport | null>(null);

  const refresh = useCallback(() => {
    api.permissions().then(setPerms).catch(console.error);
  }, []);

  useEffect(() => {
    refresh();
    api.resourcesReport().then(setRes).catch(console.error);
    // System Settings changes don't notify us; poll while the window is visible.
    const t = window.setInterval(() => {
      if (document.visibilityState === "visible") refresh();
    }, 1500);
    window.addEventListener("focus", refresh);
    return () => {
      window.clearInterval(t);
      window.removeEventListener("focus", refresh);
    };
  }, [refresh]);

  const required = perms.filter((p) => p.required);
  const ready = required.length > 0 && required.every((p) => p.state === "granted");

  return (
    <main className="setup">
      <header className="setup__head" data-tauri-drag-region>
        <div className="orb orb--big" />
        <h1>{ready ? "You're all set." : "Let's get you set up."}</h1>
        <p>
          openclicky hears you, sees your screen and helps in your apps. Everything stays on this
          Mac unless a question needs a model.
        </p>
      </header>

      <ul className="perms">
        {perms.map((p) => (
          <Row key={p.id} p={p} onChange={refresh} />
        ))}
      </ul>

      <footer className="setup__foot">
        <div className="setup__meta">
          {res &&
            (res.missing.length === 0
              ? `${res.sounds} sound slots ready`
              : `${res.missing.length} media file(s) missing. Run npm run placeholders`)}
          <button className="btn btn--link" onClick={() => api.playSound("agent-done")}>
            Test sound
          </button>
        </div>
        <button className="btn btn--primary" onClick={() => getCurrentWindow().hide()}>
          {ready ? "Done" : "Later"}
        </button>
      </footer>
    </main>
  );
}
