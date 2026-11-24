// The update window: a new version is downloaded and verified. It shows what's new (this
// version's section of CHANGELOG.md, from latest.json) and asks, like the Mac's familiar updater:
// Install and Relaunch, Install on Quit, or Skip This Version. Closing it means Install on Quit.
import { useEffect, useState, type ReactNode } from "react";
import { api, OS, onEvent, type UpdateStatus } from "../lib/bridge";
import { Inline } from "../lib/inline";
import icon from "../assets/app-icon.png";
import "../styles/update.css";

/** CHANGELOG markdown → a tagline, headings, bullets and paragraphs. */
function Notes({ text }: { text: string }) {
  const out: ReactNode[] = [];
  let items: string[] = [];
  let seenHeading = false;
  const flush = () => {
    if (!items.length) return;
    out.push(
      <ul key={`ul${out.length}`}>
        {items.map((t, i) => (
          <li key={i}>
            <Inline text={t} />
          </li>
        ))}
      </ul>,
    );
    items = [];
  };
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    const bullet = /^[-*]\s+(.*)$/.exec(line);
    const heading = /^#{2,4}\s+(.*)$/.exec(line);
    if (bullet) {
      items.push(bullet[1]);
      continue;
    }
    flush();
    if (!line) continue;
    if (heading) {
      seenHeading = true;
      out.push(<h3 key={`h${out.length}`}>{heading[1]}</h3>);
    } else if (!seenHeading && out.length === 0) {
      // The release's one-line summary, before its sections.
      out.push(
        <p key="tag" className="upd__tagline">
          <Inline text={line} />
        </p>,
      );
    } else {
      out.push(
        <p key={`p${out.length}`}>
          <Inline text={line} />
        </p>,
      );
    }
  }
  flush();
  return <>{out}</>;
}

export function Update() {
  const [u, setU] = useState<UpdateStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    api.updateStatus().then(setU).catch(() => {});
    const un = onEvent((e) => e.kind === "update.state" && setU(e.payload as UpdateStatus));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const ready = u?.phase === "ready" && !!u.version;
  const version = u?.version ?? "";

  const install = () => {
    if (!ready || busy) return;
    setBusy(true);
    setError("");
    api.updateRestart().catch((e) => {
      setBusy(false);
      setError(String(e));
    });
  };
  const later = () => api.updateLater().catch(console.error);
  const skip = () => api.updateSkip().catch(console.error);
  const setAuto = (on: boolean) => {
    setU((s) => (s ? { ...s, auto: on } : s));
    api.setSettings({ auto_update: on }).catch(console.error);
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Enter") install();
      if (e.key === "Escape") later();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  return (
    <div className={`upd upd--${OS}`}>
      <div className="upd__drag" data-tauri-drag-region />
      <header className="upd__head">
        <img className="upd__icon" src={icon} alt="" draggable={false} />
        <div>
          <h1>{ready ? "A new version of Pixie is ready to install!" : busy ? "Installing…" : "Pixie is up to date"}</h1>
          <p>
            {ready
              ? `Pixie ${version} has been downloaded and is ready to use! Would you like to install it and relaunch Pixie now?`
              : `You have Pixie ${u?.current ?? ""}.`}
          </p>
        </div>
      </header>

      <section className="upd__notes" tabIndex={0}>
        {u?.notes ? <Notes text={u.notes} /> : <p className="upd__none">{ready ? `Pixie ${version}` : "Nothing new to install."}</p>}
      </section>

      {error && <p className="upd__error">{error}</p>}

      <label className="upd__auto">
        <input type="checkbox" checked={u?.auto ?? true} onChange={(e) => setAuto(e.target.checked)} />
        <span>Automatically download and install updates in the future</span>
      </label>

      <footer className="upd__buttons">
        <button className="upd__btn" disabled={!ready || busy} onClick={skip}>
          Skip This Version
        </button>
        <span className="upd__gap" />
        <button className="upd__btn" disabled={!ready || busy} onClick={later}>
          Install on Quit
        </button>
        <button className="upd__btn upd__btn--primary" disabled={!ready || busy} onClick={install} autoFocus>
          {busy ? "Installing…" : "Install and Relaunch"}
        </button>
      </footer>
    </div>
  );
}
