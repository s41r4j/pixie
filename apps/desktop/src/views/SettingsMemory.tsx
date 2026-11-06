// Settings → Memory: what Pixie remembers about you across days, and how today's chat runs.
import { useEffect, useState } from "react";
import { api, onEvent, type MemoryNote, type SettingsView } from "../lib/bridge";

const MODES: { id: "auto" | "manual" | "off"; label: string; why: string }[] = [
  { id: "auto", label: "Automatic", why: "Keeps what you ask it to, and learns lasting things from your chats once they go quiet." },
  { id: "manual", label: "Only when I ask", why: "Keeps a note only when you say \"remember…\"." },
  { id: "off", label: "Off", why: "Doesn't read or write notes. The ones you have stay here until you delete them." },
];

const KINDS: { id: string; label: string }[] = [
  { id: "about", label: "About you" },
  { id: "preference", label: "Preference" },
  { id: "person", label: "Person" },
  { id: "project", label: "Project" },
  { id: "howto", label: "How-to" },
  { id: "other", label: "Other" },
];

const SOURCE: Record<string, string> = { said: "you asked", you: "added here", learned: "learned from a chat", job: "from a job" };

const hourLabel = (h: number) => (h === 0 ? "00:00 (midnight)" : `${String(h).padStart(2, "0")}:00`);

export function MemoryPanel({ view, onSaved }: { view: SettingsView | null; onSaved: (v: SettingsView) => void }) {
  const s = view?.settings;
  const mode = (s?.memory as "auto" | "manual" | "off" | undefined) ?? "auto";
  const [notes, setNotes] = useState<MemoryNote[] | null>(null);
  const [text, setText] = useState("");
  const [kind, setKind] = useState("about");
  const [err, setErr] = useState("");
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [q, setQ] = useState("");

  const load = () => api.memoryList().then(setNotes).catch(() => setNotes([]));
  useEffect(() => {
    load();
    const un = onEvent((e) => {
      if (e.kind === "memory.changed") load();
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const save = (patch: Parameters<typeof api.setSettings>[0]) => api.setSettings(patch).then(onSaved);
  const add = () => {
    if (!text.trim()) return;
    setErr("");
    api
      .memoryAdd(text.trim(), kind)
      .then(() => {
        setText("");
        load();
      })
      .catch((e) => setErr(String(e)));
  };
  const commit = () => {
    if (!editing) return;
    api
      .memoryUpdate(editing.id, editing.text)
      .then(() => {
        setEditing(null);
        load();
      })
      .catch((e) => setErr(String(e)));
  };
  const clearAll = () => {
    if (!confirmClear) {
      setConfirmClear(true);
      window.setTimeout(() => setConfirmClear(false), 4000);
      return;
    }
    setConfirmClear(false);
    api.memoryClear().then(load);
  };

  const shown = (notes ?? []).filter((n) => !q.trim() || n.text.toLowerCase().includes(q.trim().toLowerCase()));

  return (
    <div className="stack-gap">
      <div className="banner banner--muted">
        Notes stay on this computer, in <code>memory.json</code>. The most useful ones go with every question; Pixie looks up the rest when they matter. Passwords and keys are never kept.
      </div>

      <h3 className="sub">Remembering</h3>
      <div className="choice-cards choice-cards--plain" data-sid="memory_mode">
        {MODES.map((m) => (
          <button key={m.id} className={`choice ${mode === m.id ? "choice--on" : ""}`} onClick={() => save({ memory: m.id })}>
            <b>{m.label}</b>
            <span>{m.why}</span>
          </button>
        ))}
      </div>

      <h3 className="sub">Today's chat</h3>
      <ul className="rows">
        <li className="row" data-sid="chat_reset_hour">
          <div className="row__text">
            <div className="row__label">A new chat starts each day at</div>
            <div className="row__why">Until then, Pixie carries on with today's conversation, even after quiet gaps or a restart. New chat in the notch starts one any time.</div>
          </div>
          <div className="row__control">
            <select className="mem-select" value={s?.chat_reset_hour ?? 0} onChange={(e) => save({ chat_reset_hour: Number(e.target.value) })}>
              {Array.from({ length: 24 }, (_, h) => (
                <option key={h} value={h}>
                  {hourLabel(h)}
                </option>
              ))}
            </select>
          </div>
        </li>
      </ul>

      <h3 className="sub">
        What Pixie knows about you{notes ? ` · ${notes.length}` : ""}
      </h3>
      <div className="memory-add" data-sid="memory_add">
        <input
          value={text}
          placeholder='Add a note, e.g. "Prefers short answers"'
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          {KINDS.map((k) => (
            <option key={k.id} value={k.id}>
              {k.label}
            </option>
          ))}
        </select>
        <button className="btn btn--primary" onClick={add} disabled={!text.trim()}>
          Add
        </button>
      </div>
      {err && <div className="banner banner--warn">{err}</div>}
      {notes && notes.length > 6 && <input className="memory-search" value={q} placeholder="Search notes" onChange={(e) => setQ(e.target.value)} />}
      {notes === null ? null : notes.length === 0 ? (
        <div className="memory-empty">Nothing yet. Tell Pixie "remember that…", or add a note above.</div>
      ) : (
        <ul className="rows memory-list">
          {shown.map((n) => (
            <li key={n.id} className="row">
              <div className="row__text">
                {editing?.id === n.id ? (
                  <input
                    className="memory-edit"
                    autoFocus
                    value={editing.text}
                    onChange={(e) => setEditing({ id: n.id, text: e.target.value })}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") commit();
                      if (e.key === "Escape") setEditing(null);
                    }}
                    onBlur={commit}
                  />
                ) : (
                  <div className="row__label memory-text" onClick={() => setEditing({ id: n.id, text: n.text })} data-tip="Click to edit">
                    {n.text}
                  </div>
                )}
                <div className="row__why">
                  {KINDS.find((k) => k.id === n.kind)?.label ?? "Other"} · {SOURCE[n.source] ?? n.source} · {new Date(n.updated).toLocaleDateString()}
                  {n.uses ? ` · used ${n.uses}×` : ""}
                </div>
              </div>
              <div className="row__control">
                <button className="btn btn--link" onClick={() => api.memoryDelete(n.id).then(load)} data-tip="Forget this">
                  Forget
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {notes && notes.length > 0 && (
        <div>
          <button className={`btn ${confirmClear ? "btn--danger" : "btn--ghost"}`} onClick={clearAll}>
            {confirmClear ? "Forget all notes for good?" : "Forget everything"}
          </button>
        </div>
      )}
    </div>
  );
}
