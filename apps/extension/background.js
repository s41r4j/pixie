// openclicky browser bridge (MV3 service worker).
// App ⇄ native host (com.openclicky.browser) ⇄ here. Requests: {id, operation, tabId?, ...args}.
// Replies: {type: "result", id, result} or {type: "result", id, error}.

const HOST = "com.openclicky.browser";
const VERSION = chrome.runtime.getManifest().version;
// Same timeout table as the reference protocol (the model's prompt depends on it).
const LIMITS = { load: 8000, saving: 3000, batch: 30000, single: 20000, batchSteps: 20, wait: 10000, waitMax: 20000, cursorGlide: 240 };
const BATCHABLE = new Set(["open", "navigate", "click", "fill", "type", "press", "scroll", "wait_for", "back", "point"]);
const UNCHANGED = "Nothing on the page changed; your last snapshot still applies.";

let port = null;
const lastSnapshot = new Map(); // tabId -> lines[]

function connect() {
  if (port) return;
  try {
    port = chrome.runtime.connectNative(HOST);
  } catch (e) {
    port = null;
    return;
  }
  port.onMessage.addListener(onRequest);
  port.onDisconnect.addListener(() => {
    port = null;
    setBadge(false);
    setTimeout(connect, 4000);
  });
  port.postMessage({ type: "hello", protocol: 1, version: VERSION });
  setBadge(true);
}

function setBadge(on) {
  chrome.action.setBadgeText({ text: on ? "" : "off" });
  chrome.action.setBadgeBackgroundColor({ color: "#5C5E6A" });
  chrome.action.setTitle({ title: on ? "openclicky is connected" : "openclicky isn't running" });
}

chrome.runtime.onStartup.addListener(connect);
chrome.runtime.onInstalled.addListener(connect);
chrome.alarms.create("openclicky-keepalive", { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener(() => connect());
chrome.action.onClicked.addListener(() => connect());
connect();

async function onRequest(msg) {
  if (!msg || msg.id === undefined) return;
  const limit = msg.operation === "batch" ? LIMITS.batch : LIMITS.single;
  try {
    const result = await withTimeout(run(msg), limit, `${msg.operation} took longer than ${limit / 1000} s`);
    port?.postMessage({ type: "result", id: msg.id, result });
  } catch (e) {
    port?.postMessage({ type: "result", id: msg.id, error: String(e?.message ?? e) });
  }
}

function withTimeout(p, ms, why) {
  return Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error(why)), ms))]);
}

function safeUrl(url) {
  let u;
  try {
    u = new URL(/^https?:\/\//.test(url) ? url : `https://${url}`);
  } catch {
    throw new Error(`Not a web address: ${url}`);
  }
  if (!/^https?:$/.test(u.protocol)) throw new Error("Only http(s) pages.");
  if (u.username || u.password) throw new Error("No credentials in URLs.");
  return u.href;
}

async function currentTab() {
  const [t] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (!t) throw new Error("No browser tab is open.");
  return t;
}

async function tabFor(msg) {
  if (msg.tabId) return chrome.tabs.get(msg.tabId);
  return currentTab();
}

function waitForLoad(tabId, ms = LIMITS.load) {
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      chrome.tabs.onUpdated.removeListener(listener);
      resolve();
    };
    const listener = (id, info) => {
      if (id === tabId && info.status === "complete") setTimeout(finish, 250);
    };
    chrome.tabs.onUpdated.addListener(listener);
    chrome.tabs.get(tabId).then((t) => t.status === "complete" && setTimeout(finish, 300)).catch(finish);
    setTimeout(finish, ms);
  });
}

async function groupTab(tabId) {
  try {
    const { groupId } = await chrome.storage.session.get("groupId");
    let gid = groupId;
    if (gid !== undefined) {
      try {
        await chrome.tabGroups.get(gid);
      } catch {
        gid = undefined;
      }
    }
    gid = await chrome.tabs.group(gid === undefined ? { tabIds: [tabId] } : { tabIds: [tabId], groupId: gid });
    await chrome.tabGroups.update(gid, { title: ":) openclicky", color: "blue" });
    await chrome.storage.session.set({ groupId: gid });
  } catch {
    /* groups are a nicety */
  }
}

async function inPage(tabId, func, args = []) {
  const [r] = await chrome.scripting.executeScript({ target: { tabId }, func, args, world: "ISOLATED" });
  if (r?.result?.__error) throw new Error(r.result.__error);
  return r?.result;
}

// ---- page-side functions (serialised into the page; must be self-contained) -----------------

function pageSnapshot() {
  const SEL =
    'a[href],button,input:not([type=hidden]),select,textarea,summary,[role=button],[role=link],[role=tab],[role=checkbox],[role=radio],[role=menuitem],[role=option],[role=switch],[role=combobox],[role=textbox],[contenteditable=""],[contenteditable="true"],h1,h2,h3,[role=dialog],[role=alert]';
  window.__ocN = window.__ocN || 0;
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return false;
    const s = getComputedStyle(el);
    return s.visibility !== "hidden" && s.display !== "none" && s.opacity !== "0";
  };
  const roleOf = (el) => {
    const r = el.getAttribute("role");
    if (r) return r;
    const t = el.tagName.toLowerCase();
    if (t === "a") return "link";
    if (/^h[1-3]$/.test(t)) return "heading";
    if (t === "select") return "combobox";
    if (t === "textarea") return "textbox";
    if (t === "summary") return "button";
    if (t === "input") {
      const ty = (el.getAttribute("type") || "text").toLowerCase();
      if (["checkbox", "radio"].includes(ty)) return ty;
      if (["button", "submit", "reset", "image"].includes(ty)) return "button";
      if (ty === "search") return "searchbox";
      return "textbox";
    }
    if (el.isContentEditable) return "textbox";
    return t;
  };
  const nameOf = (el) => {
    const clean = (s) => (s || "").replace(/\s+/g, " ").trim();
    let n = clean(el.getAttribute("aria-label"));
    if (!n && el.labels && el.labels.length) n = clean(el.labels[0].innerText);
    if (!n && el.getAttribute("aria-labelledby")) n = clean(document.getElementById(el.getAttribute("aria-labelledby"))?.innerText);
    if (!n && !["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName)) n = clean(el.innerText);
    if (!n) n = clean(el.getAttribute("placeholder") || el.getAttribute("title") || el.getAttribute("alt") || (el.type === "submit" ? el.value : ""));
    if (!n) n = clean(el.querySelector("img")?.getAttribute("alt"));
    return n.length > 80 ? n.slice(0, 79) + "…" : n;
  };
  const lines = [];
  for (const el of document.querySelectorAll(SEL)) {
    if (lines.length >= 400) break;
    if (!visible(el)) continue;
    const role = roleOf(el);
    const name = nameOf(el);
    if (role === "heading") {
      if (name) lines.push(`heading: ${name}`);
      continue;
    }
    if (!name && !["textbox", "searchbox", "combobox", "checkbox", "radio"].includes(role)) continue;
    let ref = el.getAttribute("data-oc-ref");
    if (!ref) {
      ref = `e${++window.__ocN}`;
      el.setAttribute("data-oc-ref", ref);
    }
    const st = [];
    if (document.activeElement === el) st.push("focused");
    if (el.checked || el.getAttribute("aria-checked") === "true") st.push("checked");
    if (el.disabled || el.getAttribute("aria-disabled") === "true") st.push("disabled");
    if (el.required) st.push("required");
    if (el.getAttribute("aria-expanded") === "true") st.push("expanded");
    if (el.getAttribute("aria-selected") === "true") st.push("selected");
    let val = "";
    if ((el.tagName === "INPUT" || el.tagName === "TEXTAREA") && el.type !== "password" && el.value) val = ` = "${el.value.slice(0, 60)}"`;
    if (el.type === "password") st.push("password");
    if (el.tagName === "SELECT") val = ` = "${el.options[el.selectedIndex]?.text ?? ""}"`;
    lines.push(`${role}: ${name}${val} [${ref}]${st.length ? " " + st.join(" ") : ""}`);
  }
  return { title: document.title, url: location.href, lines, scroll: Math.round((scrollY / Math.max(1, document.documentElement.scrollHeight - innerHeight)) * 100) };
}

function pageExtract() {
  const text = (document.body?.innerText || "").replace(/\n{3,}/g, "\n\n").slice(0, 40000);
  const tables = [...document.querySelectorAll("table")].slice(0, 20).map((t) =>
    [...t.rows].slice(0, 200).map((r) => [...r.cells].map((c) => c.innerText.trim().replace(/\s+/g, " ")).join(" | ")).join("\n"),
  );
  return { title: document.title, url: location.href, text, tables: tables.join("\n\n").slice(0, 60000) };
}

function pageAct(action, ref, text, submit, glide) {
  const el = ref ? document.querySelector(`[data-oc-ref="${ref}"]`) : document.activeElement;
  if (ref && !el) return { __error: `No element ${ref}. Take a new snapshot.` };
  const cursor = () => {
    let c = document.getElementById("__openclicky_cursor");
    if (!c) {
      c = document.createElement("div");
      c.id = "__openclicky_cursor";
      c.style.cssText =
        "position:fixed;z-index:2147483647;width:18px;height:18px;border-radius:50%;pointer-events:none;left:50vw;top:-20px;" +
        "background:radial-gradient(circle at 35% 30%,#fff 0 8%,#8EC5FF 30%,#5AA8F5 60%,#221898);box-shadow:0 0 0 2px #fff,0 0 16px rgba(90,168,245,.8);" +
        `transition:left ${glide}ms ease,top ${glide}ms ease,opacity 400ms ease;`;
      document.documentElement.appendChild(c);
    }
    return c;
  };
  if (el && action !== "press") {
    el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    const r = el.getBoundingClientRect();
    const c = cursor();
    c.style.opacity = "1";
    c.style.left = `${r.left + r.width / 2 - 9}px`;
    c.style.top = `${r.top + r.height / 2 - 9}px`;
    clearTimeout(window.__ocCursorT);
    window.__ocCursorT = setTimeout(() => (c.style.opacity = "0"), 2500);
  }
  if (action === "point") return { ok: true };
  if (action === "click") {
    el.focus?.();
    el.click();
    return { ok: true };
  }
  if (action === "fill") {
    if (el.type === "password") return { __error: "That's a password field; I'll let them type it." };
    el.focus();
    if (el.isContentEditable) {
      document.execCommand("selectAll", false);
      document.execCommand("insertText", false, text);
    } else if (el.tagName === "SELECT") {
      const opt = [...el.options].find((o) => o.text.trim().toLowerCase() === text.toLowerCase() || o.value === text);
      if (!opt) return { __error: `No option "${text}". Options: ${[...el.options].map((o) => o.text.trim()).join(", ")}` };
      el.value = opt.value;
    } else {
      const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, "value").set.call(el, text);
    }
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
    if (submit) {
      el.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, bubbles: true }));
      if (el.form) el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit();
    }
    return { ok: true };
  }
  if (action === "press") {
    const target = document.activeElement || document.body;
    const key = text || "Enter";
    for (const type of ["keydown", "keypress", "keyup"])
      target.dispatchEvent(new KeyboardEvent(type, { key, code: key, keyCode: key === "Enter" ? 13 : key === "Escape" ? 27 : key === "Tab" ? 9 : 0, bubbles: true }));
    if (key === "Enter" && target.form) target.form.requestSubmit ? target.form.requestSubmit() : target.form.submit();
    return { ok: true };
  }
  return { __error: `unknown action ${action}` };
}

function pageScroll(direction, amount) {
  const dy = (direction === "up" ? -1 : 1) * (amount || 0.8) * innerHeight;
  if (direction === "top") scrollTo(0, 0);
  else if (direction === "bottom") scrollTo(0, document.documentElement.scrollHeight);
  else scrollBy({ top: dy, behavior: "instant" });
  return { ok: true };
}

function pageHasText(text) {
  return (document.body?.innerText || "").toLowerCase().includes(text.toLowerCase());
}

// ---- operations ------------------------------------------------------------------------------

function render(snap) {
  return `# ${snap.title}\n${snap.url}  (scrolled ${snap.scroll}%)\n${snap.lines.join("\n")}`;
}

async function snapshot(tabId, diff) {
  const snap = await inPage(tabId, pageSnapshot);
  const prev = lastSnapshot.get(tabId);
  lastSnapshot.set(tabId, snap.lines);
  if (!diff || !prev) return render(snap);
  const before = new Set(prev);
  const after = new Set(snap.lines);
  const added = snap.lines.filter((l) => !before.has(l));
  const removed = prev.filter((l) => !after.has(l));
  if (!added.length && !removed.length) return UNCHANGED;
  if (added.length + removed.length > snap.lines.length * 0.6) return render(snap);
  return `# ${snap.title}\n${snap.url}\n${removed.map((l) => `- ${l}`).join("\n")}${removed.length && added.length ? "\n" : ""}${added.map((l) => `+ ${l}`).join("\n")}`;
}

async function settle(tabId) {
  await new Promise((r) => setTimeout(r, 450));
  await waitForLoad(tabId, LIMITS.load);
}

async function run(msg) {
  const op = msg.operation;
  switch (op) {
    case "tabs": {
      const cur = await currentTab().catch(() => null);
      const tabs = await chrome.tabs.query({});
      return {
        text: tabs
          .slice(0, 500)
          .map((t) => `${t.id}${t.id === cur?.id ? " (current)" : ""}: ${t.title} — ${t.url}`)
          .join("\n"),
      };
    }
    case "open": {
      const url = safeUrl(msg.url);
      const tab = await chrome.tabs.create({ url, active: msg.background ? false : true });
      await groupTab(tab.id);
      await waitForLoad(tab.id);
      return { tabId: tab.id, text: `Opened tab ${tab.id}.\n` + (await snapshot(tab.id, false)) };
    }
    case "navigate": {
      const tab = await tabFor(msg);
      await chrome.tabs.update(tab.id, { url: safeUrl(msg.url) });
      await settle(tab.id);
      return { tabId: tab.id, text: await snapshot(tab.id, false) };
    }
    case "back": {
      const tab = await tabFor(msg);
      await chrome.tabs.goBack(tab.id);
      await settle(tab.id);
      return { tabId: tab.id, text: await snapshot(tab.id, true) };
    }
    case "close": {
      const tab = await tabFor(msg);
      if (tab.active && !msg.force && !(await isOurs(tab))) throw new Error("That's their own tab; I only close tabs I opened.");
      await chrome.tabs.remove(tab.id);
      return { text: `Closed tab ${tab.id}.` };
    }
    case "snapshot": {
      const tab = await tabFor(msg);
      return { tabId: tab.id, text: await snapshot(tab.id, false) };
    }
    case "extract": {
      const tab = await tabFor(msg);
      const r = await inPage(tab.id, pageExtract);
      return { tabId: tab.id, text: `# ${r.title}\n${r.url}\n\n${r.text}${r.tables ? `\n\nTables:\n${r.tables}` : ""}` };
    }
    case "click":
    case "point":
    case "fill":
    case "type":
    case "press": {
      const tab = await tabFor(msg);
      const action = op === "type" ? "fill" : op;
      await inPage(tab.id, pageAct, [action, msg.ref ?? null, msg.text ?? msg.key ?? "", !!msg.submit, LIMITS.cursorGlide]);
      if (op === "point") return { tabId: tab.id, text: `Pointing at ${msg.ref}.` };
      await settle(tab.id);
      return { tabId: tab.id, text: await snapshot(tab.id, true) };
    }
    case "scroll": {
      const tab = await tabFor(msg);
      await inPage(tab.id, pageScroll, [msg.direction ?? "down", msg.amount ?? 0.8]);
      await new Promise((r) => setTimeout(r, 300));
      return { tabId: tab.id, text: await snapshot(tab.id, true) };
    }
    case "wait_for": {
      const tab = await tabFor(msg);
      const until = Date.now() + Math.min(msg.timeout ?? LIMITS.wait, LIMITS.waitMax);
      while (Date.now() < until) {
        if (await inPage(tab.id, pageHasText, [msg.text ?? ""])) return { tabId: tab.id, text: `"${msg.text}" is on the page.` };
        await new Promise((r) => setTimeout(r, 400));
      }
      throw new Error(`"${msg.text}" didn't appear.`);
    }
    case "screenshot": {
      const tab = await tabFor(msg);
      await chrome.tabs.update(tab.id, { active: true });
      await new Promise((r) => setTimeout(r, 250));
      const url = await chrome.tabs.captureVisibleTab(tab.windowId, { format: "png" });
      return { tabId: tab.id, screenshot: url.split(",")[1], text: `Screenshot of ${tab.title}.` };
    }
    case "batch": {
      const steps = (msg.steps ?? []).slice(0, LIMITS.batchSteps);
      const out = [];
      let tabId = msg.tabId;
      for (const [i, s] of steps.entries()) {
        if (!BATCHABLE.has(s.operation)) throw new Error(`Step ${i + 1}: ${s.operation} can't be batched.`);
        try {
          const r = await run({ ...s, tabId: s.tabId ?? tabId });
          tabId = r.tabId ?? tabId;
          out.push(`${i + 1}. ${s.operation}: ok`);
          if (i === steps.length - 1) out.push(r.text ?? "");
        } catch (e) {
          out.push(`${i + 1}. ${s.operation}: ${e.message}. Stopped here.`);
          break;
        }
      }
      return { tabId, text: out.join("\n") };
    }
    default:
      throw new Error(`Unknown operation "${op}".`);
  }
}

async function isOurs(tab) {
  const { groupId } = await chrome.storage.session.get("groupId");
  return groupId !== undefined && tab.groupId === groupId;
}
