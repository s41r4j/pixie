// Edge job cards + the floating pill. Running jobs show their 3–5 box plan and a status line;
// a job that needs them shows the question with tap answers; a finished job shows the result
// and its files. Click a card for the full note.
import { useEffect, useRef, useState } from "react";
import { api, onEvent, type Job } from "../lib/bridge";
import { useHitRects } from "../lib/hits";
import { Mark } from "../components/Mark";

export function PlanBoxes({ job }: { job: Job }) {
  if (!job.plan.length) return null;
  return (
    <div className="plan">
      {job.plan.map((s, i) => (
        <span key={i} className={`plan__box plan__box--${s.status}`} title={s.label} />
      ))}
      <span className="plan__label">{job.plan.find((s) => s.status === "in_progress")?.label ?? (job.state === "done" ? "Done" : job.plan[0].label)}</span>
    </div>
  );
}

function stateText(j: Job) {
  switch (j.state) {
    case "running":
      return j.progress || "Working";
    case "waiting_on_user":
      return "Needs you";
    case "done":
      return "Ready";
    case "failed":
      return j.error === "Stopped." ? "Stopped" : "Hit a problem";
    default:
      return "Offered";
  }
}

function Card({ job }: { job: Job }) {
  const [reply, setReply] = useState("");
  const firstLine = job.report?.body.split("\n").find((l) => l.trim()) ?? "";
  const files = job.report?.artifacts ?? [];
  return (
    <div className={`card card--${job.state}`} onClick={() => api.openNote(job.id)}>
      <div className="card__head">
        <span className={`dot dot--${job.state}`} />
        <span className="card__title">{job.title}</span>
        <button
          className="card__x"
          aria-label="Dismiss"
          onClick={(e) => {
            e.stopPropagation();
            if (job.state === "running") api.jobCancel(job.id);
            else api.jobDismiss(job.id);
          }}
        >
          {job.state === "running" ? "■" : "×"}
        </button>
      </div>
      <div className="card__status">{stateText(job)}</div>
      {job.state !== "done" && <PlanBoxes job={job} />}

      {job.state === "waiting_on_user" && job.question && (
        <div className="card__ask" onClick={(e) => e.stopPropagation()}>
          <div className="card__q">{job.question.question}</div>
          <div className="card__choices">
            {job.question.choices.map((c) => (
              <button key={c} className="chip chip--tap" onClick={() => api.jobAnswer(job.id, c)}>
                {c}
              </button>
            ))}
            <button className="chip chip--tap" onClick={() => api.jobAnswer(job.id, "Use your best guess.")}>
              Use your best guess
            </button>
          </div>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (reply.trim()) api.jobAnswer(job.id, reply.trim());
              setReply("");
            }}
          >
            <input className="card__input" placeholder="Or type an answer…" value={reply} onChange={(e) => setReply(e.target.value)} />
          </form>
        </div>
      )}

      {job.state === "done" && (
        <>
          {firstLine && <div className="card__body">{firstLine}</div>}
          {files.length > 0 && (
            <div className="card__files" onClick={(e) => e.stopPropagation()}>
              {files.slice(0, 3).map((f) => (
                <button key={f} className="chip chip--file" onClick={() => api.openTarget(f)} title={f}>
                  {f.split("/").pop()}
                </button>
              ))}
            </div>
          )}
        </>
      )}
      {job.state === "failed" && job.error && job.error !== "Stopped." && <div className="card__body card__body--bad">{job.error}</div>}
    </div>
  );
}

export function Shelf() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [open, setOpen] = useState(false);
  const pill = useRef<HTMLDivElement>(null);
  const stack = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.jobs().then(setJobs).catch(() => {});
    const un = onEvent((e) => {
      if (e.kind === "shelf.jobs") setJobs(e.payload.jobs);
      if (e.kind === "job.updated")
        setJobs((js) => {
          const j = e.payload.job as Job;
          const i = js.findIndex((x) => x.id === j.id);
          if (i < 0) return [j, ...js];
          const n = [...js];
          n[i] = j;
          return n;
        });
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const active = jobs.filter((j) => j.state === "running" || j.state === "waiting_on_user" || j.unread);
  const shown = open ? jobs.slice(0, 6) : active.slice(0, 4);
  const running = jobs.filter((j) => j.state === "running").length;
  const needs = jobs.filter((j) => j.state === "waiting_on_user").length;

  useHitRects("shelf", [pill, stack], [shown.length, open, jobs]);

  return (
    <div className="shelf" onMouseLeave={() => setOpen(false)}>
      <div ref={pill} className={`jobpill ${running ? "jobpill--busy" : ""} ${needs ? "jobpill--needs" : ""}`} onMouseEnter={() => setOpen(true)} onClick={() => setOpen(!open)}>
        <Mark size={30} live={running > 0} />
        {(running > 0 || needs > 0) && <span className="jobpill__badge">{needs || running}</span>}
      </div>
      <div ref={stack} className="stack">
        {shown.map((j) => (
          <Card key={j.id} job={j} />
        ))}
      </div>
    </div>
  );
}
