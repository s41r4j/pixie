// Hover tips spoken by the buddy: in Pixie's own windows, hovering anything with data-tip,
// title or aria-label shows that text in a bubble beside the buddy (unless the buddy is off).
// Native title tooltips are moved to data-tip so only the buddy shows them.
import { invoke } from "@tauri-apps/api/core";

function tipOf(el: Element): string {
  if (el.hasAttribute("title")) {
    el.setAttribute("data-tip", el.getAttribute("title") ?? "");
    el.removeAttribute("title");
  }
  return el.getAttribute("data-tip") || el.getAttribute("aria-label") || "";
}

export function installTips() {
  let current: Element | null = null;
  let timer = 0;
  let shown = false;
  const send = (text: string) => invoke("buddy_tip", { text }).catch(() => {});
  const clear = () => {
    clearTimeout(timer);
    current = null;
    if (shown) {
      shown = false;
      send("");
    }
  };
  document.addEventListener("pointerover", (e) => {
    const el = (e.target as Element | null)?.closest?.("[data-tip],[title],button[aria-label],[role=switch][aria-label],a[aria-label]") ?? null;
    if (el === current) return;
    clearTimeout(timer);
    current = el;
    if (!el) {
      timer = window.setTimeout(clear, 120);
      return;
    }
    const text = tipOf(el).trim();
    timer = window.setTimeout(() => {
      if (current !== el) return;
      shown = !!text;
      send(text);
    }, 380);
  });
  document.addEventListener("pointerleave", clear);
  window.addEventListener("blur", clear);
  document.addEventListener("pointerdown", clear);
}
