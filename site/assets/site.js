(() => {
const REPO = "s41r4j/pixie", DMG = "Pixie-macOS-universal.dmg";
const $ = (s, r = document) => r.querySelector(s), $$ = (s, r = document) => [...r.querySelectorAll(s)];
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

/* ---------- theme + menu ---------- */
const root = document.documentElement;
if (localStorage.getItem("pixie-theme") === "light") root.classList.add("light");
$("#theme").onclick = () => { root.classList.toggle("light"); localStorage.setItem("pixie-theme", root.classList.contains("light") ? "light" : "dark"); };
$("#burger").onclick = () => $("#nav").classList.toggle("open");
$$("#nav a").forEach(a => a.addEventListener("click", () => $("#nav").classList.remove("open")));

/* ---------- the website buddy: follows the pointer, explains what you hover ---------- */
const wb = $("#wb"), say = $(".say", wb);
let mx = -100, my = -100, bx = -100, by = -100, seen = false, greeted = false;
addEventListener("pointermove", e => {
  if (e.pointerType !== "mouse") return;
  mx = e.clientX; my = e.clientY;
  if (!seen) { seen = true; bx = mx + 26; by = my + 28; wb.classList.add("on"); greet(); }
}, { passive: true });
document.addEventListener("pointerleave", () => wb.classList.remove("on"));
document.addEventListener("pointerenter", () => seen && wb.classList.add("on"));
addEventListener("pointerdown", () => { wb.classList.add("click"); setTimeout(() => wb.classList.remove("click"), 140); });
let tipEl = null, tipTimer = 0;
function show(text) { say.textContent = text; wb.classList.add("tip"); }
function hide() { wb.classList.remove("tip"); }
function greet() {
  if (greeted || sessionStorage.getItem("pixie-hi")) return;
  greeted = true; sessionStorage.setItem("pixie-hi", "1");
  setTimeout(() => { if (!tipEl) { show("Hi, I'm Pixie. Hover things and I'll tell you about them."); setTimeout(() => !tipEl && hide(), 3400); } }, 500);
}
document.addEventListener("pointerover", e => {
  const el = e.target.closest("[data-tip]");
  if (el === tipEl) return;
  tipEl = el; clearTimeout(tipTimer);
  if (el) tipTimer = setTimeout(() => show(el.dataset.tip), 260); else hide();
});
(function loop() {
  // Rides down-right of the pointer, with a little space, like in the app.
  const tx = mx + 26, ty = my + 28;
  bx += (tx - bx) * (reduce ? 1 : .22); by += (ty - by) * (reduce ? 1 : .22);
  wb.classList.toggle("flip", bx > innerWidth - 300);
  wb.style.transform = `translate(${bx - 11}px, ${by - 11}px)`;
  requestAnimationFrame(loop);
})();

if ($("#stage")) (() => {
/* ---------- the demo: five scenes that play like a video ---------- */
const stage = $("#stage"), sn = $("#sn"), ear = $("#ear"), sub = $("#sub"), cur = $("#cur"), bud = $("#bud"), ink = $("#ink");
const SCENES = [
  { t: "Ask", d: "Hold ⌃⌥ and talk", ms: 8200, run: sAsk },
  { t: "Show", d: "It points and draws", ms: 8200, run: sShow },
  { t: "Do", d: "It clicks and types", ms: 11000, run: sDo },
  { t: "Explain", d: "It sketches a board", ms: 9800, run: sExplain },
  { t: "Delegate", d: "Agents take the job", ms: 13500, run: sDelegate },
];
const ctl = $("#dctl");
SCENES.forEach((s, i) => {
  const b = document.createElement("button");
  b.className = "dtab"; b.innerHTML = `<b>0${i + 1} ${s.t}</b><span>${s.d}</span><i class="pg"></i>`;
  b.onclick = () => play(i, true);
  ctl.append(b);
});
const pbtn = document.createElement("button"); pbtn.className = "dplay"; pbtn.textContent = "❚❚"; pbtn.setAttribute("aria-label", "Pause");
ctl.append(pbtn);
let gen = 0, idx = 0, auto = true, inView = false, started = false;
pbtn.onclick = () => { auto = !auto; pbtn.textContent = auto ? "❚❚" : "▶"; if (auto) play(idx); };
const W = () => stage.clientWidth, H = () => stage.clientHeight;
const sleep = (ms, g) => new Promise((ok, no) => setTimeout(() => g === gen ? ok() : no("stop"), reduce ? Math.min(ms, 60) : ms));
// positions are in stage pixels
let cx = 0, cy = 0, ux = 0, uy = 0, budFree = true;
function center(el) { const r = el.getBoundingClientRect(), s = stage.getBoundingClientRect(); return { x: r.left - s.left + r.width / 2, y: r.top - s.top + r.height / 2, w: r.width, h: r.height, l: r.left - s.left, t: r.top - s.top }; }
function place() {
  cur.style.transform = `translate(${cx}px, ${cy}px)`;
  if (budFree) { ux += (cx + 24 - ux) * .25; uy += (cy + 26 - uy) * .25; }
  bud.style.transform = `translate(${ux - bud.offsetWidth / 2}px, ${uy - bud.offsetHeight / 2}px)`;
}
(function tick() { place(); requestAnimationFrame(tick); })();
function tween(ms, g, f) {
  return new Promise((ok, no) => {
    const t0 = performance.now();
    const step = now => { if (g !== gen) return no("stop"); const k = reduce ? 1 : Math.min(1, (now - t0) / ms); f(1 - Math.pow(1 - k, 3)); k < 1 ? requestAnimationFrame(step) : ok(); };
    requestAnimationFrame(step);
  });
}
const moveCursor = (x, y, ms, g) => { const x0 = cx, y0 = cy; return tween(ms, g, e => { cx = x0 + (x - x0) * e; cy = y0 + (y - y0) * e; }); };
function flyBuddy(x, y, ms, g) {
  budFree = false; const x0 = ux, y0 = uy, mx2 = (x0 + x) / 2, my2 = Math.min(y0, y) - H() * .12;
  return tween(ms, g, e => { const u = 1 - e; ux = u * u * x0 + 2 * u * e * mx2 + e * e * x; uy = u * u * y0 + 2 * u * e * my2 + e * e * y; });
}
function ripple(x, y) { const r = document.createElement("i"); r.className = "rip"; r.style.left = x + "px"; r.style.top = y + "px"; stage.append(r); setTimeout(() => r.remove(), 500); }
function notch(s, html = "") { sn.dataset.s = s; ear.innerHTML = html; }
const WAVE = `<span class="wave">${"<i></i>".repeat(11)}</span>`, DOTS = `<span class="dots"><i></i><i></i><i></i></span>`;
function subtitle(who, text) { if (!text) return sub.classList.remove("on"); sub.innerHTML = `<em>${who}</em>${text}`; sub.classList.add("on"); }
function reset() {
  notch("rest"); subtitle(); $("#dlg").classList.remove("on"); $("#fname").textContent = ""; $("#brd").classList.remove("on"); ink.innerHTML = "";
  $("#brdsvg").innerHTML = ""; budFree = true; cx = W() * .78; cy = H() * .62;
  ["ag1p", "ag2p"].forEach(id => $$("#" + id + " i").forEach(i => i.className = ""));
}
async function listen(g, words) {
  notch("listen", WAVE); subtitle("You", `“${words}”`); await sleep(2100, g);
  notch("think", DOTS); await sleep(800, g);
}
async function sAsk(g) {
  await moveCursor(W() * .45, H() * .55, 900, g);
  await listen(g, "What does this chart show?");
  notch("speak", WAVE); subtitle("Pixie", "“Revenue grew every month, up 18% in November. Mostly new customers.”"); await sleep(3600, g);
  notch("rest"); subtitle(); await sleep(400, g);
}
function circleAround(r, g) {
  const p = document.createElementNS("http://www.w3.org/2000/svg", "ellipse");
  p.setAttribute("cx", r.x); p.setAttribute("cy", r.y); p.setAttribute("rx", r.w / 2 + 14); p.setAttribute("ry", r.h / 2 + 10); p.setAttribute("pathLength", 1); p.setAttribute("class", "c");
  ink.append(p); requestAnimationFrame(() => requestAnimationFrame(() => g === gen && p.classList.add("d")));
}
async function sShow(g) {
  await moveCursor(W() * .5, H() * .6, 700, g);
  await listen(g, "Where's the export button?");
  const e = center($("#t-export"));
  await flyBuddy(e.x + e.w / 2 + 16, e.y + e.h / 2 + 14, 650, g);
  circleAround(e, g); notch("speak", WAVE); subtitle("Pixie", "“Top right of the toolbar. I've circled it.”"); await sleep(1500, g);
  const c = center($("#chart"));
  const sel = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  Object.entries({ x: c.l + c.w * .58, y: c.t - 4, width: c.w * .42, height: c.h + 8, class: "ants" }).forEach(([k, v]) => sel.setAttribute(k, v));
  await flyBuddy(c.l + c.w + 12, c.t + c.h + 12, 600, g); ink.append(sel);
  subtitle("Pixie", "“And these two months are where the jump happened.”"); await sleep(2300, g);
  notch("rest"); subtitle(); budFree = true; await sleep(300, g);
}
async function sDo(g) {
  await moveCursor(W() * .5, H() * .58, 700, g);
  await listen(g, "Export this as a PDF called Q3 Review.");
  notch("speak", WAVE); subtitle("Pixie", "“On it.”");
  const e = center($("#t-export"));
  await flyBuddy(e.x, e.y, 650, g); ripple(e.x, e.y); await sleep(350, g);
  $("#dlg").classList.add("on"); await sleep(400, g);
  const f = center($("#fname")); await flyBuddy(f.l + 20, f.y, 500, g); ripple(f.l + 20, f.y);
  const name = "Q3 Review.pdf";
  for (let i = 1; i <= name.length; i++) { $("#fname").textContent = name.slice(0, i); await sleep(70, g); }
  const s = center($("#d-save")); await flyBuddy(s.x, s.y, 500, g); ripple(s.x, s.y); await sleep(300, g);
  $("#dlg").classList.remove("on"); budFree = true; subtitle();
  notch("toast", `<span class="toast">■ Saved Q3 Review.pdf</span>`); await sleep(2600, g); notch("rest");
}
async function sExplain(g) {
  await moveCursor(W() * .5, H() * .55, 600, g);
  await listen(g, "How does our sales funnel work?");
  $("#brd").classList.add("on"); notch("speak", WAVE);
  subtitle("Pixie", "“Visitors become leads, leads become trials, and about a fifth of trials buy.”");
  const svg = $("#brdsvg"), NS = "http://www.w3.org/2000/svg";
  const parts = [["rect", { x: 60, y: 250, width: 200, height: 90 }, "Visitors", 160, 302], ["path", { d: "M275 295 L395 295 M380 284 L395 295 L380 306" }], ["rect", { x: 410, y: 250, width: 200, height: 90 }, "Leads", 510, 302],
    ["path", { d: "M625 295 L745 295 M730 284 L745 295 L730 306" }], ["rect", { x: 760, y: 250, width: 190, height: 90 }, "Trials", 855, 302], ["path", { d: "M855 355 L855 455 M844 440 L855 455 L866 440" }], ["rect", { x: 760, y: 470, width: 190, height: 90 }, "Customers", 855, 522], ["text", {}, "~20% of trials buy", 600, 520]];
  for (const [tag, at, label, lx, ly] of parts) {
    if (tag !== "text") {
      const el = document.createElementNS(NS, tag); Object.entries(at).forEach(([k, v]) => el.setAttribute(k, v)); el.setAttribute("pathLength", 1); el.setAttribute("class", "s");
      if (tag === "path") el.style.fill = "none"; svg.append(el); requestAnimationFrame(() => requestAnimationFrame(() => el.classList.add("d")));
    }
    if (label) { const t = document.createElementNS(NS, "text"); t.setAttribute("x", lx); t.setAttribute("y", ly); t.setAttribute("text-anchor", "middle"); t.textContent = label; svg.append(t); setTimeout(() => t.classList.add("d"), 350); }
    await sleep(560, g);
  }
  await sleep(2400, g); subtitle("Click", "anywhere and the board goes away"); ripple(cx, cy); await sleep(500, g);
  $("#brd").classList.remove("on"); notch("rest"); subtitle(); await sleep(500, g);
}
function plan(id, st) { $$("#" + id + " i").forEach((i, k) => i.className = k < st ? "d" : k === st ? "a" : ""); }
async function sDelegate(g) {
  await moveCursor(W() * .5, H() * .58, 600, g);
  await listen(g, "Research three CRMs and write me a comparison.");
  notch("toast", `<span class="toast">■ Started: CRM comparison</span>`); subtitle("Pixie", "“On it. I'll ping you when it's ready.”"); await sleep(2000, g);
  subtitle("You", "hover the notch"); await moveCursor(W() * .5, H() * .03, 700, g);
  notch("peek"); subtitle(); plan("ag1p", 0); plan("ag2p", 1);
  const steps = [["Reading hubspot.com", 1], ["Comparing prices", 2], ["Writing the note", 3]];
  for (const [txt, st] of steps) { $("#ag1s").textContent = txt; plan("ag1p", st); $("#ag2s").textContent = st > 1 ? "Checking it worked" : "Clicking slide 3"; if (st > 1) plan("ag2p", 2); await sleep(1500, g); }
  plan("ag1p", 4); $("#ag1s").textContent = "done"; plan("ag2p", 3); $("#ag2s").textContent = "done"; await sleep(1100, g);
  await moveCursor(W() * .5, H() * .6, 500, g); notch("toast", `<span class="toast">■ CRM comparison is ready</span>`); await sleep(2000, g); notch("rest");
}
function bars(i, ms) {
  $$(".dtab", ctl).forEach((b, k) => { b.classList.toggle("on", k === i); const p = $(".pg", b); p.style.transition = "none"; p.style.width = k < i ? "100%" : "0"; });
  const p = $(".pg", $$(".dtab", ctl)[i]); requestAnimationFrame(() => requestAnimationFrame(() => { p.style.transition = `width ${ms}ms linear`; p.style.width = "100%"; }));
}
async function play(i, user) {
  const g = ++gen; idx = i; if (user && !auto) { auto = true; pbtn.textContent = "❚❚"; }
  reset(); bars(i, SCENES[i].ms); const t0 = performance.now();
  try {
    await SCENES[i].run(g);
    const left = SCENES[i].ms - (performance.now() - t0); if (left > 0) await sleep(left, g);
    if (g === gen && auto && inView) play((i + 1) % SCENES.length);
  } catch (_) { /* superseded */ }
}
new IntersectionObserver(([e]) => {
  inView = e.isIntersecting;
  if (inView && (!started || auto)) { started = true; play(idx); } else if (!inView) gen++;
}, { threshold: .35 }).observe(stage);
$$("[data-scene]").forEach(a => a.addEventListener("click", () => setTimeout(() => play(+a.dataset.scene, true), 400)));
const deep = /^#demo-(\d)$/.exec(location.hash);
if (deep) { idx = Math.min(4, Math.max(0, +deep[1] - 1)); setTimeout(() => { stage.scrollIntoView({ block: "center" }); }, 50); }
setInterval(() => { const d = new Date(); $("#clock").textContent = d.toLocaleDateString([], { weekday: "short" }) + " " + d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }); }, 10000);

})();

if ($("#reel")) (() => {
/* ---------- screenshot reel: drag, arrows, gentle auto-scroll, lightbox ---------- */
const reel = $("#reel");
let down = false, sx = 0, sl = 0, moved = 0, hover = false;
reel.addEventListener("pointerdown", e => { down = true; moved = 0; sx = e.clientX; sl = reel.scrollLeft; });
addEventListener("pointermove", e => { if (!down) return; const dx = e.clientX - sx; moved = Math.max(moved, Math.abs(dx)); if (moved > 4) reel.classList.add("drag"); reel.scrollLeft = sl - dx; });
addEventListener("pointerup", () => { down = false; setTimeout(() => reel.classList.remove("drag"), 0); });
reel.addEventListener("pointerenter", () => hover = true); reel.addEventListener("pointerleave", () => hover = false);
const stepBy = d => reel.scrollBy({ left: d * (reel.querySelector(".shot").offsetWidth + 18), behavior: "smooth" });
$("#rprev").onclick = () => stepBy(-1); $("#rnext").onclick = () => stepBy(1);
setInterval(() => { if (hover || down || reduce || document.hidden) return; if (reel.scrollLeft + reel.clientWidth >= reel.scrollWidth - 4) reel.scrollTo({ left: 0, behavior: "smooth" }); else stepBy(1); }, 4200);
const lb = $("#lb");
lb.onclick = null;
$$(".shot", reel).forEach(f => f.addEventListener("click", () => { if (moved > 4) return; const img = $("img", f); $("img", lb).src = img.src; $("img", lb).alt = img.alt; $("figcaption span", lb).textContent = $("figcaption", f).firstChild.textContent; lb.showModal(); }));
lb.addEventListener("click", () => lb.close());

})();

if ($("#dnav")) (() => {
/* ---------- feature shots open full size ---------- */
if ($("#lb")) {
  const lb = $("#lb");
  $$(".dshot").forEach(f => f.addEventListener("click", () => { const img = $("img", f); $("img", lb).src = img.src; $("figcaption span", lb).textContent = $("figcaption", f).textContent; lb.showModal(); }));
  lb.addEventListener("click", () => lb.close());
}

/* ---------- docs: nav + search ---------- */
const dnav = $("#dnav"), docs = $$(".doc");
docs.forEach(d => { const a = document.createElement("a"); a.href = "#" + d.id; a.textContent = $("h3", d).textContent; dnav.append(a); });
$("#dsearch").addEventListener("input", e => {
  const q = e.target.value.trim().toLowerCase(); let n = 0;
  docs.forEach((d, i) => { const hit = !q || d.textContent.toLowerCase().includes(q); d.hidden = !hit; dnav.children[i].hidden = !hit; n += hit; });
  $("#nores").style.display = n ? "none" : "block";
});

})();

/* ---------- live release info ---------- */
const mb = b => b > 1e6 ? (b / 1e6).toFixed(1) + " MB" : Math.round(b / 1e3) + " KB";
fetch(`https://api.github.com/repos/${REPO}/releases/latest`).then(r => r.ok ? r.json() : Promise.reject(r.status)).then(rel => {
  const dmg = rel.assets.find(a => a.name === DMG) || rel.assets.find(a => a.name.endsWith(".dmg"));
  $$("[data-version]").forEach(e => e.textContent = rel.tag_name);
  $$("[data-date]").forEach(e => e.textContent = new Date(rel.published_at).toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" }));
  if (dmg) { $$("[data-size]").forEach(e => e.textContent = mb(dmg.size)); $$("[data-dl]").forEach(a => a.href = dmg.browser_download_url); }
  if ($("[data-assets]")) $("[data-assets]").innerHTML = rel.assets.map(a => `<a href="${a.browser_download_url}"><span>${a.name}</span><span>${mb(a.size)}</span></a>`).join("");
}).catch(() => {});

if ($("#log")) (() => {
/* ---------- changelog from CHANGELOG.md ---------- */
const inl = s => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`([^`]+)`/g, "<code>$1</code>");
fetch("CHANGELOG.md").then(r => r.text()).then(md => {
  const parts = md.split(/^## /m).slice(1);
  $("#log").innerHTML = parts.map((p, i) => {
    const [head, ...rest] = p.split("\n"); const [v, date] = head.split(/\s+[—-]\s+/);
    const items = rest.filter(l => l.startsWith("- ")).map(l => `<li>${inl(l.slice(2))}</li>`).join("");
    return `<article class="rel ${i ? "old" : ""}"><div><div class="v">${v.trim()}</div><time>${date || ""}</time>${i ? "" : '<span class="tag">latest</span>'}</div><div><ul>${items}</ul><button class="more">Show ${rest.filter(l => l.startsWith("- ")).length} changes</button></div></article>`;
  }).join("");
  $$("#log .more").forEach(b => b.onclick = () => { b.closest(".rel").classList.add("open"); b.remove(); });
}).catch(() => { $("#log").innerHTML = `<p style="color:var(--muted)">See <a style="text-decoration:underline" href="https://github.com/${REPO}/blob/main/CHANGELOG.md">CHANGELOG.md</a>.</p>`; });

})();

/* ---------- nav: mark the current page ---------- */
const page = (location.pathname.split("/").pop() || "index.html").replace(/\.html$/, "") || "index";
$$("#nav a[data-page]").forEach(a => a.classList.toggle("on", a.dataset.page === page));
})();
