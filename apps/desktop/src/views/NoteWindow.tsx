// A job's full note on paper, live: lines appear as the job writes them. Plan on top, files
// and next steps under the page, a box to tell it what to change.
import { useEffect, useRef, useState } from "react";
import { api, onEvent, type Job, type NoteDoc } from "../lib/bridge";
import { NoteView } from "../components/NoteView";
import { PlanBoxes } from "./Shelf";
import { Mark } from "../components/Mark";

function jobFromHash(): string {
  return new URLSearchParams(location.hash.slice(1)).get("job") ?? "";
}

export function NoteWindow() {
  const [id, setId] = useState(jobFromHash());
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
      if (e.kind === "note.select") setId(e.payload.id);
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
    document.title = job ? `${job.title} — openclicky` : "openclicky";
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
          <Mark size={26} /> <span>Jobs</span>
        </div>
        {jobs.map((j) => (
          <button key={j.id} className={`side-job ${j.id === id ? "is-on" : ""}`} onClick={() => setId(j.id)}>
            <span className={`dot dot--${j.state}`} />
            <span className="side-job__t">{j.title}</span>
          </button>
        ))}
      </aside>
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
    </div>
  );
}
