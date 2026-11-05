// A job's full note on paper, live: lines appear as the job writes them. Plan on top, files
// and next steps under the page, a box to tell it what to change.
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api, onEvent, type ChatEntry, type ChatSummary, type Job, type NoteDoc } from "../lib/bridge";
import { NoteView } from "../components/NoteView";
import { PlanBoxes } from "./Shelf";
import { Mark } from "../components/Mark";

function jobFromHash(): string {
  return new URLSearchParams(location.hash.slice(1)).get("job") ?? "";
}

function when(ms: number) {
  const d = new Date(ms);
  const today = new Date();
  return d.toDateString() === today.toDateString()
    ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : d.toLocaleDateString([], { month: "short", day: "numeric" });
}

/** Past conversations: list on the left, the transcript on the right. */
function ChatsPortal({ initial, slot }: { initial: string; slot: HTMLElement | null }) {
  const [list, setList] = useState<ChatSummary[]>([]);
  const [cur, setCur] = useState(initial);
  const [chat, setChat] = useState<{ title: string; started: number; entries: ChatEntry[] } | null>(null);
  const [q, setQ] = useState("");
  const load = () => api.chatsList().then((l) => {
    setList(l);
    setCur((c) => c || l[0]?.id || "");
  });
  useEffect(() => {
    load();
    const un = onEvent((e) => {
      if (e.kind === "chat.updated" || e.kind === "chats.changed") load();
      if (e.kind === "chat.updated" && e.payload.id && e.payload.id === cur) api.chatGet(cur).then(setChat);
    });
    return () => {
      un.then((f) => f());
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (cur) api.chatGet(cur).then(setChat);
    else setChat(null);
  }, [cur]);
  const shown = list.filter((c) => !q || `${c.title} ${c.preview}`.toLowerCase().includes(q.toLowerCase()));
  const side = (
      <div className="side-list">
        <input className="side-search" placeholder="Search chats" value={q} onChange={(e) => setQ(e.target.value)} />
        {shown.length === 0 && <div className="side-empty">{list.length ? "No matches." : "No chats yet. Hold ⌃⌥ and ask something."}</div>}
        {shown.map((c) => (
          <button key={c.id} className={`side-job side-chat ${c.id === cur ? "is-on" : ""}`} onClick={() => setCur(c.id)}>
            <span className="side-job__t">{c.title}</span>
            <span className="side-chat__meta">{when(c.updated)} · {c.count}</span>
          </button>
        ))}
      </div>
  );
  return (
    <>
      {slot && createPortal(side, slot)}
      <ChatPane
        chat={chat}
        onDelete={() => api.chatDelete(cur).then(() => { setCur(""); load(); })}
      />
    </>
  );
}

function ChatPane({ chat, onDelete }: { chat: { title: string; started: number; entries: ChatEntry[] } | null; onDelete: () => void }) {
  return (
    <main className="notewin__main chatwin">
      <header className="notewin__head" data-tauri-drag-region>
        {chat && (
          <>
            <div className="notewin__title" data-tauri-drag-region>
              <h1>{chat.title}</h1>
              <span className="notewin__state">{new Date(chat.started).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}</span>
            </div>
            <div className="notewin__tools">
              <button
                className="btn btn--ghost"
                onClick={() => navigator.clipboard.writeText(chat.entries.map((e) => `You: ${e.q}\nPixie: ${e.a}`).join("\n\n"))}
              >
                Copy
              </button>
              <button className="btn btn--ghost" onClick={onDelete}>Delete</button>
            </div>
          </>
        )}
      </header>
      <div className="notewin__scroll">
        {!chat && <div className="empty">Pick a chat on the left.</div>}
        {chat && (
          <div className="chatlog chatlog--page">
            {chat.entries.map((m) => (
              <div key={m.at} className="chatlog__turn">
                <div className="chatlog__q">{m.q}</div>
                <div className="chatlog__a">{m.a}</div>
                <div className="chatlog__at">{when(m.at)}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    </main>
  );
}

export function NoteWindow() {
  const start = jobFromHash();
  const [side, setSide] = useState<"jobs" | "chats">(start.startsWith("chats") ? "chats" : "jobs");
  const [chatId, setChatId] = useState(start.startsWith("chats:") ? start.slice(6) : "");
  const [id, setId] = useState(start.startsWith("chats") ? "" : start);
  const [job, setJob] = useState<Job | null>(null);
  const [doc, setDoc] = useState<NoteDoc | null>(null);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");
  const idRef = useRef(id);
  idRef.current = id;

  useEffect(() => {
    if (!id) return;
    api.job(id).then(setJob);
    api.jobNote(id).then(setDoc);
  }, [id]);

  useEffect(() => {
    api.jobs().then(setJobs);
    const un = onEvent((e) => {
      if (e.kind === "note.select") {
        const v = String(e.payload.id ?? "");
        if (v.startsWith("chats")) {
          setSide("chats");
          setChatId(v.slice(6));
        } else {
          setSide("jobs");
          setId(v);
        }
      }
      if (e.kind === "job.updated") {
        const j = e.payload.job as Job;
        setJobs((js) => {
          const i = js.findIndex((x) => x.id === j.id);
          if (i < 0) return [j, ...js];
          const n = [...js];
          n[i] = j;
          return n;
        });
        if (j.id === idRef.current) setJob(j);
      }
      if (e.kind === "job.note" && e.job_id === idRef.current) setDoc(e.payload.doc);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    document.title = job ? `${job.title} — Pixie` : "Pixie";
  }, [job]);

  const say = (text: string) => {
    if (!job) return;
    setErr("");
    api.jobAnswer(job.id, text).catch((e) => setErr(String(e)));
  };

  const running = job?.state === "running";

  return (
    <div className="notewin">
      <aside className="notewin__side" data-tauri-drag-region>
        <div className="notewin__brand" data-tauri-drag-region>
          <Mark size={26} /> <span>Pixie</span>
        </div>
        <div className="tabs side-tabs" role="tablist">
          <button data-tip="Agents: their plans, notes and results" role="tab" aria-selected={side === "jobs"} className={`tab ${side === "jobs" ? "tab--on" : ""}`} onClick={() => setSide("jobs")}>
            Jobs <span className="tab__count">{jobs.length}</span>
          </button>
          <button data-tip="Every conversation you've had with Pixie" role="tab" aria-selected={side === "chats"} className={`tab ${side === "chats" ? "tab--on" : ""}`} onClick={() => setSide("chats")}>
            Chats
          </button>
        </div>
        {side === "jobs" && jobs.length === 0 && <div className="side-empty">No jobs yet. Ask for something big and it shows up here.</div>}
        {side === "jobs" &&
          jobs.map((j) => (
            <button key={j.id} className={`side-job ${j.id === id ? "is-on" : ""}`} onClick={() => setId(j.id)}>
              <span className={`dot dot--${j.state}`} />
              <span className="side-job__t">{j.title}</span>
              {j.kind === "live" && <span className="agent__live">live</span>}
            </button>
          ))}
        {side === "chats" && <div id="chat-side" />}
      </aside>
      {side === "chats" ? <ChatsHost initial={chatId} /> : (
      <main className="notewin__main">
        <header className="notewin__head" data-tauri-drag-region>
          {job && (
            <>
              <div className="notewin__title" data-tauri-drag-region>
                <span className={`dot dot--${job.state}`} />
                <h1>{job.title}</h1>
                <span className="notewin__state">{job.state === "running" ? job.progress || "Working" : job.state === "waiting_on_user" ? "Needs you" : job.state === "done" ? "Ready" : job.error === "Stopped." ? "Stopped" : "Problem"}</span>
              </div>
              <div className="notewin__tools">
                {running ? (
                  <button className="btn btn--ghost" onClick={() => api.jobCancel(job.id)}>Stop</button>
                ) : (
                  <button className="btn btn--ghost" onClick={() => api.openTarget(job.cwd + "/output")}>Files</button>
                )}
                <button className="btn btn--ghost" onClick={() => api.jobDelete(job.id).then(() => setId(jobs.find((x) => x.id !== job.id)?.id ?? ""))}>Delete</button>
              </div>
            </>
          )}
        </header>
        {job && <PlanBoxes job={job} />}

        <div className="notewin__scroll">
          {!job && <div className="empty">Pick a job on the left.</div>}
          {job?.state === "waiting_on_user" && job.question && (
            <div className="needs">
              <div className="needs__q">{job.question.question}</div>
              <div className="needs__a">
                {job.question.choices.map((c) => (
                  <button key={c} className="btn btn--primary" onClick={() => say(c)}>{c}</button>
                ))}
                <button className="btn btn--ghost" onClick={() => say("Use your best guess.")}>Use your best guess</button>
              </div>
            </div>
          )}
          {doc && <NoteView doc={doc} onSay={running ? undefined : say} />}
          {job?.state === "failed" && job.error && <div className="needs needs--bad">{job.error}</div>}
          {job?.report && (
            <div className="report">
              {job.report.artifacts.length > 0 && (
                <div className="report__files">
                  {job.report.artifacts.map((a) => (
                    <div key={a} className="report__file">
                      <span className="report__name">{a.split("/").pop()}</span>
                      <button className="btn btn--primary" onClick={() => api.openTarget(a)}>Open</button>
                      {!a.startsWith("http") && <button className="btn btn--ghost" onClick={() => api.openTarget(a, true)}>Show in Finder</button>}
                    </div>
                  ))}
                </div>
              )}
              {job.report.draft && (
                <div className="report__draft">
                  <div className="report__label">Draft</div>
                  <pre>{job.report.draft}</pre>
                  <button className="btn btn--ghost" onClick={() => navigator.clipboard.writeText(job.report!.draft!)}>Copy</button>
                </div>
              )}
              {job.report.next_actions.length > 0 && (
                <div className="report__next">
                  {job.report.next_actions.map((n) => (
                    <button key={n} className="chip chip--tap" onClick={() => say(n)}>{n}</button>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        {job && (
          <form
            className="notewin__composer"
            onSubmit={(e) => {
              e.preventDefault();
              if (!msg.trim()) return;
              say(msg.trim());
              setMsg("");
            }}
          >
            <input
              value={msg}
              onChange={(e) => setMsg(e.target.value)}
              placeholder={running ? "It's working… you can stop it above." : job.state === "waiting_on_user" ? "Answer…" : "Tell it what to change or what's next…"}
              disabled={running}
            />
            <button className="btn btn--primary" disabled={running || !msg.trim()}>Send</button>
            {err && <span className="notewin__err">{err}</span>}
          </form>
        )}
      </main>
      )}
    </div>
  );
}

/** Chats render their list into the sidebar slot and the transcript into the main area. */
function ChatsHost({ initial }: { initial: string }) {
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  useEffect(() => setSlot(document.getElementById("chat-side")), []);
  return <ChatsPortal slot={slot} initial={initial} />;
}
