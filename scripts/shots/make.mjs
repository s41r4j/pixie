// The website and README pictures: a stand-in desktop (scenes.html) with Pixie's real overlay
// drawn over it (the app's browser preview), at 2x.
//
//   npm run dev -w apps/desktop          (the preview, on port 1420)
//   node scripts/shots/make.mjs <chrome-headless-shell> [scene…]
//
// Writes site/shots/<name>.png for buddy, ask and do.
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const [bin, ...wanted] = process.argv.slice(2);
if (!bin) throw new Error("usage: node scripts/shots/make.mjs <chrome-headless-shell> [scene…]");
const scenes = wanted.length ? wanted : ["buddy", "ask", "do"];
const W = 1280, H = 800, SCALE = 2;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const port = 9400 + Math.floor(Math.random() * 400);
const chrome = spawn(bin, ["--headless", `--remote-debugging-port=${port}`, "--allow-file-access-from-files", "--hide-scrollbars", `--window-size=${W},${H}`, "about:blank"], { stdio: "ignore" });
let target;
for (let i = 0; i < 60 && !target; i++) {
  await sleep(150);
  try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page"); } catch {}
}
const ws = new WebSocket(target.webSocketDebuggerUrl);
let id = 0;
const waiting = new Map();
const send = (method, params = {}) => new Promise((r) => { waiting.set(++id, r); ws.send(JSON.stringify({ id, method, params })); });
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && waiting.has(d.id)) waiting.get(d.id)(d.result);
  if (d.method === "Runtime.exceptionThrown") console.log("page error:", d.params.exceptionDetails.exception?.description);
};
await new Promise((r) => (ws.onopen = r));
await send("Runtime.enable");
await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: SCALE, mobile: false });
const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result?.value;
const shoot = async () => Buffer.from((await send("Page.captureScreenshot", { format: "png" })).data, "base64");

for (const scene of scenes) {
  // 1. The desktop, and where its marks go.
  const url = pathToFileURL(path.join(here, "scenes.html")).href + `?scene=${scene}`;
  await send("Page.navigate", { url });
  await sleep(1200);
  const marks = await evaluate("new Promise(r => { const t = () => window.SCENE ? r(window.SCENE) : setTimeout(t, 50); t(); })");
  const desk = await shoot();
  // 2. The real overlay over it.
  await send("Page.navigate", { url: "http://localhost:1420/?preview=overlay&scripted=1" });
  await sleep(1500);
  await evaluate(`document.body.style.setProperty("background", "url(data:image/png;base64,${desk.toString("base64")}) 0 0 / ${W}px ${H}px no-repeat", "important")`);
  // The buddy starts beside the pointer, low on the screen.
  await evaluate(`window.__previewEmit("buddy.cursor", { x: ${W * 0.62}, y: ${H * 0.74} }, 0)`);
  let t = 300;
  for (const [kind, payload, after] of marks) {
    t += after;
    await evaluate(`window.__previewEmit(${JSON.stringify(kind)}, ${JSON.stringify(payload)}, ${t})`);
  }
  await sleep(t + 2600);
  const out = path.join(root, "site/shots", `${scene}.png`);
  fs.writeFileSync(out, await shoot());
  console.log("wrote", path.relative(root, out));
}
chrome.kill();
process.exit(0);
