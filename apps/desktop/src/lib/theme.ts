import { api, type Theme } from "./bridge";

// Group → CSS var prefix. `colors.ink` → `--ink`, `draw.sky` → `--draw-sky`, `radius.md` → `--r-md`.
// `brand` tokens are kept in theme.json for the brand work but not applied to the UI yet.
const PREFIX: Record<string, string> = { colors: "", draw: "draw-", paper: "paper-", radius: "r-", motion: "m-" };

/** Apply theme.json as CSS custom properties. styles.css carries the same defaults. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement.style;
  for (const [group, prefix] of Object.entries(PREFIX)) {
    const values = theme[group];
    if (!values || typeof values !== "object") continue;
    for (const [name, value] of Object.entries(values)) root.setProperty(`--${prefix}${name}`, value);
  }
}

export function loadTheme(): void {
  api.theme().then(applyTheme).catch(() => console.warn("theme.json unavailable; using CSS defaults"));
}
