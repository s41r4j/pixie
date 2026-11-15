// The Excalidraw canvas Pixie draws on: diagrams and explanations the user can edit and keep.
// The model's `canvas` tool queues operations in Rust; this window takes them, draws the shapes
// one by one, saves the scene to <app_data>/canvases/<id>.excalidraw, and reports back.
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Excalidraw, MainMenu, convertToExcalidrawElements, exportToBlob, exportToSvg, newElementWith, restore, restoreElements, serializeAsJSON } from "@excalidraw/excalidraw";
import type { ExcalidrawImperativeAPI } from "@excalidraw/excalidraw/types";
import "@excalidraw/excalidraw/index.css";
import { IS_MAC, onEvent } from "../lib/bridge";

type Skeleton = Parameters<typeof convertToExcalidrawElements>[0] extends (infer T)[] | null ? T : never;
type Op = { req: number; canvas: string; action: "draw" | "edit" | "read" | "clear"; input: Record<string, unknown> };
type Item = { id: string; title: string; modified: number; shapes: number };
type Elements = ReturnType<ExcalidrawImperativeAPI["getSceneElements"]>;

const idFromHash = () => new URLSearchParams(location.hash.slice(1)).get("canvas") ?? "";

/** Bottom edge of what's drawn, to put a new diagram under it instead of on top. */
function bottom(els: Elements): number {
  const live = els.filter((e) => !e.isDeleted);
  return live.length ? Math.max(...live.map((e) => e.y + e.height)) + 80 : 0;
}

/** What's on the canvas, compact, for the model (`read`). */
function describe(els: Elements): string {
  const live = els.filter((e) => !e.isDeleted);
  const text = new Map<string, string>();
  for (const e of live) if (e.type === "text" && "containerId" in e && e.containerId) text.set(e.containerId, (e as { text: string }).text);
  const lines = live
    .filter((e) => !(e.type === "text" && "containerId" in e && e.containerId))
    .slice(0, 300)
    .map((e) => {
      const r = (v: number) => Math.round(v);
      const words = e.type === "text" ? (e as { text: string }).text : text.get(e.id);
      const ends =
        "startBinding" in e && (e.startBinding || e.endBinding)
          ? ` from=${e.startBinding?.elementId ?? "-"} to=${e.endBinding?.elementId ?? "-"}`
          : "";
      return `${e.id} ${e.type} x=${r(e.x)} y=${r(e.y)} w=${r(e.width)} h=${r(e.height)}${words ? ` "${words.replace(/\s+/g, " ").slice(0, 80)}"` : ""}${ends}`;
    });
  return lines.length ? `${lines.length} element(s):\n${lines.join("\n")}` : "The canvas is empty.";
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Box = { x: number; y: number; w: number; h: number; kind: string };

/** Where the line from a shape's centre towards (tx, ty) leaves its outline, plus a small gap. */
function edge(b: Box, tx: number, ty: number, gap = 8): [number, number] {
  const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
  const dx = tx - cx, dy = ty - cy;
  if (!dx && !dy) return [cx, cy];
  const hw = b.w / 2, hh = b.h / 2;
  const t =
    b.kind === "ellipse"
      ? 1 / Math.sqrt((dx / hw) ** 2 + (dy / hh) ** 2)
      : b.kind === "diamond"
        ? 1 / (Math.abs(dx) / hw + Math.abs(dy) / hh)
        : Math.min(Math.abs(dx) ? hw / Math.abs(dx) : Infinity, Math.abs(dy) ? hh / Math.abs(dy) : Infinity);
  const len = Math.hypot(dx, dy);
  return [cx + dx * t + (dx / len) * gap, cy + dy * t + (dy / len) * gap];
}

/** Arrows given as start/end ids get their real geometry: edge of one shape to the edge of the other. */
function routeArrows(skeletons: Skeleton[], scene: Elements): Skeleton[] {
  const boxes = new Map<string, Box>();
  for (const e of scene) if (!e.isDeleted) boxes.set(e.id, { x: e.x, y: e.y, w: e.width, h: e.height, kind: e.type });
  for (const s of skeletons as unknown as Record<string, unknown>[]) {
    if (typeof s.id === "string" && typeof s.x === "number" && typeof s.y === "number") {
      boxes.set(s.id, { x: s.x, y: s.y, w: Number(s.width ?? 0), h: Number(s.height ?? 0), kind: String(s.type) });
    }
  }
  return skeletons.map((sk) => {
    const s = sk as unknown as Record<string, unknown>;
    if (s.type !== "arrow" && s.type !== "line") return sk;
    const a = boxes.get(String((s.start as { id?: string } | undefined)?.id ?? ""));
    const b = boxes.get(String((s.end as { id?: string } | undefined)?.id ?? ""));
    if (!a || !b) return sk;
    const [x1, y1] = edge(a, b.x + b.w / 2, b.y + b.h / 2);
    const [x2, y2] = edge(b, a.x + a.w / 2, a.y + a.h / 2);
    // The points too: without them a straight-down arrow (width 0) is given a default width
    // and comes out slanted.
    return { ...s, x: x1, y: y1, width: x2 - x1, height: y2 - y1, points: [[0, 0], [x2 - x1, y2 - y1]] } as unknown as Skeleton;
  });
}

type Change = { id?: string; label?: string; color?: string; x?: number; y?: number; width?: number; height?: number; remove?: boolean };

/** The element a name means: its id, or the shape whose label says it. */
function findElement(els: Elements, target: string): Elements[number] | undefined {
  const live = els.filter((e) => !e.isDeleted);
  const byId = live.find((e) => e.id === target);
  if (byId) return byId;
  const want = target.trim().toLowerCase();
  if (!want) return undefined;
  const says = (e: Elements[number]) => (e.type === "text" ? (e as unknown as { text: string }).text.trim().toLowerCase() : "");
  const text = live.find((e) => says(e) === want) ?? live.find((e) => says(e).includes(want));
  if (!text) return undefined;
  const container = "containerId" in text && text.containerId ? live.find((e) => e.id === text.containerId) : undefined;
  return container ?? text;
}

/** `edit`: change shapes that are already there (move, resize, recolour, relabel, remove),
 *  keeping their labels inside them and their arrows attached. */
function applyChanges(els: Elements, changes: Change[]): { next: Elements; done: number; missing: string[] } {
  type El = Elements[number];
  const live = els.filter((e) => !e.isDeleted);
  const byId = new Map(live.map((e) => [e.id, e]));
  const labelOf = new Map<string, El>();
  for (const e of live) if (e.type === "text" && "containerId" in e && e.containerId) labelOf.set(e.containerId, e);
  const patch = new Map<string, Record<string, unknown>>();
  const put = (id: string, p: Record<string, unknown>) => patch.set(id, { ...(patch.get(id) ?? {}), ...p });
  const gone = new Set<string>();
  const moved = new Set<string>();
  const relabelled = new Set<string>();
  const missing: string[] = [];
  let done = 0;
  for (const c of changes) {
    const el = byId.get(String(c.id ?? ""));
    if (!el) {
      missing.push(String(c.id ?? "?"));
      continue;
    }
    done += 1;
    const label = labelOf.get(el.id);
    if (c.remove) {
      gone.add(el.id);
      if (label) gone.add(label.id);
      // Arrows that ended on it go too.
      for (const a of live) if ("startBinding" in a && (a.startBinding?.elementId === el.id || a.endBinding?.elementId === el.id)) gone.add(a.id);
      continue;
    }
    const x = typeof c.x === "number" ? c.x : el.x;
    const y = typeof c.y === "number" ? c.y : el.y;
    const w = typeof c.width === "number" && c.width > 4 ? c.width : el.width;
    const h = typeof c.height === "number" && c.height > 4 ? c.height : el.height;
    if (x !== el.x || y !== el.y || w !== el.width || h !== el.height) {
      put(el.id, { x, y, width: w, height: h });
      moved.add(el.id);
      // Its label stays centred in it.
      if (label) put(label.id, { x: x + (w - label.width) / 2, y: y + (h - label.height) / 2 });
    }
    if (typeof c.color === "string" && c.color) {
      const lineLike = el.type === "text" || el.type === "line" || el.type === "arrow";
      put(el.id, lineLike ? { strokeColor: c.color } : { backgroundColor: c.color, fillStyle: "solid" });
    }
    if (typeof c.label === "string") {
      const text = el.type === "text" ? el : label;
      if (text) {
        put(text.id, { text: c.label, originalText: c.label });
        relabelled.add(text.id);
      } else done -= 1;
    }
  }
  const box = (e: El): Box => {
    const p = patch.get(e.id) ?? {};
    return { x: Number(p.x ?? e.x), y: Number(p.y ?? e.y), w: Number(p.width ?? e.width), h: Number(p.height ?? e.height), kind: e.type };
  };
  // Arrows follow the shapes they join.
  for (const a of live) {
    if (!("startBinding" in a) || gone.has(a.id)) continue;
    const from = a.startBinding ? byId.get(a.startBinding.elementId) : undefined;
    const to = a.endBinding ? byId.get(a.endBinding.elementId) : undefined;
    if (!from || !to || (!moved.has(from.id) && !moved.has(to.id))) continue;
    const [fb, tb] = [box(from), box(to)];
    const [x1, y1] = edge(fb, tb.x + tb.w / 2, tb.y + tb.h / 2);
    const [x2, y2] = edge(tb, fb.x + fb.w / 2, fb.y + fb.h / 2);
    put(a.id, { x: x1, y: y1, width: Math.abs(x2 - x1), height: Math.abs(y2 - y1), points: [[0, 0], [x2 - x1, y2 - y1]] });
    const text = labelOf.get(a.id);
    if (text) put(text.id, { x: (x1 + x2) / 2 - text.width / 2, y: (y1 + y2) / 2 - text.height / 2 });
  }
  const changed = live.filter((e) => !gone.has(e.id)).map((e) => (patch.has(e.id) ? newElementWith(e, patch.get(e.id) as Partial<El>) : e));
  // New label text is measured again so it sits right in its shape.
  const measured = restoreElements(changed, null, { refreshDimensions: true, repairBindings: true }) as unknown as Elements;
  // A longer label wraps onto more lines: the shape grows to hold it and keeps it centred.
  const grow = new Map<string, Record<string, unknown>>();
  for (const t of measured) {
    if (t.type !== "text" || !("containerId" in t) || !t.containerId || !relabelled.has(t.id)) continue;
    const box = measured.find((e) => e.id === t.containerId);
    if (!box || box.type === "arrow" || box.type === "line") continue;
    const h = Math.max(box.height, t.height + 24);
    if (h !== box.height) grow.set(box.id, { height: h });
    grow.set(t.id, { x: box.x + (box.width - t.width) / 2, y: box.y + (h - t.height) / 2 });
  }
  const next = (grow.size ? measured.map((e) => (grow.has(e.id) ? newElementWith(e, grow.get(e.id) as Partial<El>) : e)) : measured) as Elements;
  return { next, done, missing };
}

/** Text is measured when it's converted: the hand-drawn fonts must be loaded first. */
async function fontsReady() {
  await Promise.all(["Excalifont", "Nunito", "Comic Shanns", "Lilita One"].map((f) => document.fonts.load(`20px "${f}"`).catch(() => [])));
  await document.fonts.ready;
}

function base64(blob: Blob): Promise<string> {
  return new Promise((ok, fail) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).split(",")[1] ?? "");
    r.onerror = () => fail(r.error);
    r.readAsDataURL(blob);
  });
}

export function Canvas() {
  const [id, setId] = useState(idFromHash);
  const [initial, setInitial] = useState<{ id: string; data: object | null } | null>(null);
  const [api, setApi] = useState<ExcalidrawImperativeAPI | null>(null);
  const [title, setTitle] = useState("");
  const [list, setList] = useState<Item[]>([]);
  const [saved, setSaved] = useState<string>("");
  const [exported, setExported] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const busy = useRef(false);
  const deleted = useRef(new Set<string>());
  const titleRef = useRef("");
  const saveTimer = useRef<number | undefined>(undefined);
  const focusTimer = useRef<number | undefined>(undefined);

  // Load the scene whenever the canvas changes.
  useEffect(() => {
    if (!id) return;
    setApi(null);
    invoke<string | null>("canvas_load", { id }).then((json) => {
      let data: object | null = null;
      if (json) {
        try {
          const parsed = JSON.parse(json);
          const r = restore(parsed, null, null);
          data = { elements: r.elements, appState: { ...r.appState, collaborators: new Map() }, files: r.files };
          const t = parsed?.appState?.name ?? "";
          setTitle(t);
          titleRef.current = t;
        } catch {
          data = null;
        }
      } else {
        setTitle("");
        titleRef.current = "";
      }
      setInitial({ id, data });
    });
    invoke<Item[]>("canvas_list").then(setList).catch(() => {});
  }, [id]);

  useEffect(() => {
    const un = onEvent((e) => {
      if (e.kind === "canvas.select") {
        const next = String((e.payload as { id: string }).id);
        history.replaceState(null, "", `#canvas=${next}`);
        setId(next);
      }
      if (e.kind === "canvas.ops") take();
      if (e.kind === "canvas.focus" && api) {
        // A spoken sentence is about this part: glide to it and light it for a moment.
        const el = findElement(api.getSceneElements(), String((e.payload as { target: string }).target));
        if (el) {
          api.scrollToContent(el, { fitToViewport: true, viewportZoomFactor: 0.5, animate: true, duration: 450 });
          api.updateScene({ appState: { selectedElementIds: { [el.id]: true } } });
          window.clearTimeout(focusTimer.current);
          focusTimer.current = window.setTimeout(() => api.updateScene({ appState: { selectedElementIds: {} } }), 2800);
        }
      }
      if (e.kind === "canvas.deleted") {
        const gone = String((e.payload as { id: string }).id);
        deleted.current.add(gone);
        if (gone === id) window.clearTimeout(saveTimer.current);
        invoke<Item[]>("canvas_list").then(setList).catch(() => {});
      }
    });
    return () => {
      un.then((f) => f());
    };
  });

  const save = useCallback(
    (now = false) => {
      if (!api || !id || deleted.current.has(id)) return;
      window.clearTimeout(saveTimer.current);
      const run = () => {
        if (deleted.current.has(id)) return;
        const scene = JSON.parse(serializeAsJSON(api.getSceneElements(), api.getAppState(), api.getFiles(), "local"));
        // The "local" format drops the name; keep the title with the drawing.
        scene.appState = { ...(scene.appState ?? {}), name: titleRef.current };
        invoke("canvas_save", { id, scene: JSON.stringify(scene) }).then(() => setSaved("Saved")).catch((e) => setSaved(String(e)));
      };
      if (now) run();
      else saveTimer.current = window.setTimeout(run, 700);
    },
    [api, id],
  );

  // Clicking outside hides the window: save what's there first.
  useEffect(() => {
    const flush = () => save(true);
    window.addEventListener("blur", flush);
    return () => window.removeEventListener("blur", flush);
  }, [save]);

  /** Add new elements a few at a time so it looks drawn, not pasted. */
  const animateIn = useCallback(
    async (fresh: ReturnType<typeof convertToExcalidrawElements>) => {
      if (!api) return;
      const before = api.getSceneElements();
      // Hidden windows run timers slowly: draw it all at once rather than time out.
      if (document.hidden) {
        api.updateScene({ elements: [...before, ...fresh] });
        api.scrollToContent(undefined, { fitToViewport: true, viewportZoomFactor: 0.8 });
        return;
      }
      const step = Math.max(1, Math.ceil(fresh.length / 40));
      const pause = Math.min(110, 3200 / Math.max(1, fresh.length / step));
      for (let i = step; i < fresh.length + step; i += step) {
        api.updateScene({ elements: [...before, ...fresh.slice(0, i)] });
        await sleep(pause);
      }
      api.scrollToContent(undefined, { fitToViewport: true, viewportZoomFactor: 0.8 });
    },
    [api],
  );

  const apply = useCallback(
    async (op: Op): Promise<string> => {
      if (!api) throw new Error("The canvas isn't ready.");
      if (op.action === "read") return describe(api.getSceneElements());
      if (op.action === "clear") {
        api.updateScene({ elements: [] });
        save(true);
        return "Cleared the canvas.";
      }
      if (op.action === "edit") {
        await fontsReady();
        const r = applyChanges(api.getSceneElements(), (Array.isArray(op.input.changes) ? op.input.changes : []) as Change[]);
        if (!r.done) throw new Error(`None of those ids are on the canvas (${r.missing.join(", ")}); action=read lists what is.`);
        api.updateScene({ elements: r.next });
        save(true);
        return `Changed ${r.done} shape(s).${r.missing.length ? ` Not on the canvas: ${r.missing.join(", ")}.` : ""}`;
      }
      const name = typeof op.input.title === "string" ? op.input.title : "";
      if (name && !titleRef.current) {
        api.updateScene({ appState: { name } });
        setTitle(name);
        titleRef.current = name;
      }
      await fontsReady();
      let skeletons: Skeleton[] = [];
      let files = {};
      const below = bottom(api.getSceneElements());
      if (typeof op.input.mermaid === "string" && op.input.mermaid.trim()) {
        const { parseMermaidToExcalidraw } = await import("@excalidraw/mermaid-to-excalidraw");
        try {
          const r = await parseMermaidToExcalidraw(op.input.mermaid, { themeVariables: { fontSize: "20px" } });
          skeletons = (r.elements as Skeleton[]).map((e) => ({ ...e, y: (e.y ?? 0) + below }) as Skeleton);
          files = r.files ?? {};
        } catch (e) {
          throw new Error(`The Mermaid didn't parse: ${e instanceof Error ? e.message : String(e)}`);
        }
      } else if (Array.isArray(op.input.elements)) {
        // Things Pixie laid out (`make`) go under what's already there, not on top of it.
        const given = op.input.elements as Skeleton[];
        const placed = op.input.place_below && below ? given.map((e) => (typeof (e as { y?: unknown }).y === "number" ? ({ ...e, y: (e as { y: number }).y + below }) as Skeleton : e)) : given;
        skeletons = routeArrows(placed, api.getSceneElements());
      }
      if (!skeletons.length) throw new Error("Nothing to draw: give elements or mermaid.");
      let fresh: ReturnType<typeof convertToExcalidrawElements>;
      try {
        fresh = convertToExcalidrawElements(skeletons, { regenerateIds: false });
      } catch (e) {
        throw new Error(`Those elements didn't work: ${e instanceof Error ? e.message : String(e)}`);
      }
      // Ids the model reused from an earlier drawing would collide: give those new ones.
      const taken = new Set(api.getSceneElements().map((e) => e.id));
      if (fresh.some((e) => taken.has(e.id))) fresh = convertToExcalidrawElements(skeletons, { regenerateIds: true });
      if (Object.keys(files).length) api.addFiles(Object.values(files));
      await animateIn(fresh);
      save(true);
      const shapes = fresh.filter((e) => !(e.type === "text" && "containerId" in e && e.containerId)).length;
      return `Drew ${shapes} shape(s) on the canvas "${api.getAppState().name || "Untitled"}". They can move and edit everything, and save it as .excalidraw, PNG or SVG.`;
    },
    [api, animateIn, save],
  );

  const take = useCallback(async () => {
    if (!api || !id || busy.current) return;
    busy.current = true;
    try {
      const ops = await invoke<Op[]>("canvas_take_ops", { id });
      for (const op of ops) {
        try {
          const text = await apply(op);
          await invoke("canvas_done", { req: op.req, ok: true, text });
        } catch (e) {
          await invoke("canvas_done", { req: op.req, ok: false, text: e instanceof Error ? e.message : String(e) });
        }
      }
    } finally {
      busy.current = false;
    }
  }, [api, id, apply]);

  // Once the editor is up, run whatever was queued while it opened.
  useEffect(() => {
    if (api) take();
  }, [api, take]);

  const exportAs = async (ext: "excalidraw" | "png" | "svg") => {
    if (!api) return;
    const elements = api.getSceneElements();
    const appState = api.getAppState();
    const files = api.getFiles();
    const name = title || "Pixie drawing";
    let data = "";
    if (ext === "excalidraw") data = serializeAsJSON(elements, appState, files, "local");
    if (ext === "png") data = await base64(await exportToBlob({ elements, appState: { ...appState, exportBackground: true }, files, mimeType: "image/png", exportPadding: 24 }));
    if (ext === "svg") data = (await exportToSvg({ elements, appState: { ...appState, exportBackground: true }, files, exportPadding: 24 })).outerHTML;
    invoke<string>("canvas_export", { name, ext, data })
      .then((path) => {
        setSaved(`Saved to ${path.replace(/^.*[\\/](Downloads)/, "$1")}`);
        setExported(path);
      })
      .catch((e) => setSaved(String(e)));
  };

  const remove = () => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      window.setTimeout(() => setConfirmDelete(false), 4000);
      return;
    }
    setConfirmDelete(false);
    window.clearTimeout(saveTimer.current);
    deleted.current.add(id);
    invoke("canvas_delete", { id }).catch((e) => setSaved(String(e)));
  };

  const rename = (name: string) => {
    setTitle(name);
    titleRef.current = name;
    api?.updateScene({ appState: { name } });
    save();
  };

  if (!id) return <div className="canvas-empty">No canvas.</div>;
  return (
    <div className={`canvas-win ${IS_MAC ? "canvas-win--mac" : ""}`}>
      <header className="canvas-bar" data-tauri-drag-region>
        <input className="canvas-title" value={title} placeholder="Untitled drawing" onChange={(e) => rename(e.target.value)} />
        <span className="canvas-saved">
          {saved}
          {exported && saved.startsWith("Saved to") && (
            <button className="canvas-link" onClick={() => invoke("open_target", { target: exported, reveal: true })}>
              Show
            </button>
          )}
        </span>
        <span className="canvas-actions">
          <button onClick={() => exportAs("excalidraw")} data-tip="Save an .excalidraw file to Downloads (opens in excalidraw.com or any Excalidraw app).">.excalidraw</button>
          <button onClick={() => exportAs("png")} data-tip="Save a PNG to Downloads.">PNG</button>
          <button onClick={() => exportAs("svg")} data-tip="Save an SVG to Downloads.">SVG</button>
          <button onClick={() => invoke<string>("canvas_open", { id: null, fresh: true })} data-tip="Start a new, empty canvas.">New</button>
          <button
            className={confirmDelete ? "canvas-danger" : ""}
            onClick={remove}
            data-tip="Delete this canvas (the file goes to the Trash)."
          >
            {confirmDelete ? "Delete for good?" : "Delete"}
          </button>
        </span>
      </header>
      <div className="canvas-stage">
        {initial?.id === id && (
          <Excalidraw
            key={id}
            excalidrawAPI={setApi}
            initialData={initial.data ?? { appState: { name: title } }}
            onChange={() => save()}
            UIOptions={{ canvasActions: { loadScene: false, saveToActiveFile: false, export: false, saveAsImage: false } }}
          >
            <MainMenu>
              <MainMenu.Group title="Canvases">
                {list.slice(0, 10).map((c) => (
                  <MainMenu.Item key={c.id} onSelect={() => invoke("canvas_open", { id: c.id })}>
                    {c.id === id ? "● " : ""}
                    {c.title || "Untitled"} · {c.shapes} · {new Date(c.modified * 1000).toLocaleDateString()}
                  </MainMenu.Item>
                ))}
              </MainMenu.Group>
              <MainMenu.Separator />
              <MainMenu.DefaultItems.ClearCanvas />
              <MainMenu.DefaultItems.ToggleTheme />
              <MainMenu.DefaultItems.ChangeCanvasBackground />
              <MainMenu.DefaultItems.Help />
            </MainMenu>
          </Excalidraw>
        )}
      </div>
    </div>
  );
}
