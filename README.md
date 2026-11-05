# :) openclicky

An open-source, macOS-first companion that lives in your notch. Hold **⌃ control + ⌥ option**
and talk: it hears you, looks at your screen, answers out loud, points at things, draws on the
screen, does steps in your apps, and hands bigger work to background jobs that write a live
note. Tauri 2 + Rust + React + a small Swift helper. Claude or DeepSeek as the brain.

## Run it

Requirements: macOS 14+, Xcode command line tools (for `swiftc`), Rust stable, Node 20+.

```bash
cp .env.example .env          # put your key in: DEEPSEEK=sk-…  (or ANTHROPIC_API_KEY=…)
npm install
npm run app:build             # → target/release/bundle/macos/openclicky.app (+ .dmg)
open target/release/bundle/macos/openclicky.app
```

Development: `npm run dev` (Vite on :1420 + the app with hot reload).

On first launch a welcome window walks through permissions (Microphone, Speech Recognition,
Screen Recording, Accessibility) and the model key. macOS ties permissions to the app's
signature; ad-hoc builds get a new one each build, so after rebuilding you may need to switch
openclicky off and on again in System Settings → Privacy & Security.

## Using it

| Do this | What happens |
|---|---|
| Hold **⌃⌥** and talk, let go | The notch opens with a live waveform and your words; it answers out loud, the words light up as they're spoken |
| "Where's the share button?" | A gel pointer flies out of the notch, rings the button and labels it |
| "Circle the total" | Hand-drawn marks on the screen that fade away |
| "Click Export for me" | `mac_step` presses it through Accessibility |
| "Make me a one-page PDF about tea" | A job card appears on the right edge with a 3–5 box plan; the note fills live; the PDF lands where you asked |
| A job asks something | The card turns amber with tap answers; or answer in the note window |
| Tap **⌃⌥** (or click the menu-bar `:)`) | A text box to type instead of talk |
| Hold **fn⌃** and talk | Dictation: cleaned-up text is typed into whatever field has focus |
| **esc** | Stops speech, the turn and closes the notch |

## Keys and models

Keys are read from (first wins): Settings → AI, the process environment,
`~/Library/Application Support/com.openclicky.app/.env`, then the repo `.env` in dev builds.
`ANTHROPIC_API_KEY` → Claude (`claude-opus-5-5`, the default provider); `DEEPSEEK` /
`DEEPSEEK_API_KEY` → DeepSeek's Anthropic-compatible endpoint (`deepseek-flash`, which also
takes screenshots). See `.env.example` for model overrides.

## Browser and connectors

- **Chrome/Brave/Edge/Arc**: the app registers its native messaging host on launch. Load the
  extension once: `chrome://extensions` → Developer mode → Load unpacked →
  `~/Library/Application Support/com.openclicky.app/extension`. Then "what tabs do I have open",
  "fill this form with…", "compare these two pages" work. Details: `apps/extension/README.md`.
- **Apps (MCP)**: add servers to `~/Library/Application Support/com.openclicky.app/mcp.json`
  (Claude Desktop format, examples inside) and restart. Their tools show up as
  `mcp__<server>__<tool>`.
- **Playbooks**: `skills/<name>/SKILL.md`; add your own in the support folder's `skills/`.

## Media

Every sound, icon, video and font is loaded by slot name from `resources/manifest.json`
(see `resources/README.md`). `npm run assets:local -- --from DIR` builds a private local pack
in Application Support that overrides the bundled media on this Mac only; never commit
third-party media.

## Commands

```bash
npm run check          # cargo check --workspace && frontend build
npm test               # cargo test --workspace
npm run app:build      # .app + .dmg for this machine
npm run app:build:universal   # needs: rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run placeholders   # regenerate missing placeholder media (never overwrites)
xcrun swift scripts/make-logo.swift   # re-render the :) icons
```

UI preview without the app (mocked IPC): `npm run dev -w apps/desktop`, then open
`http://localhost:1420/?preview=panel&mode=answer` (views: panel, overlay, shelf, note,
settings; see `apps/desktop/src/preview.ts`).

## Layout

```
apps/desktop/         Tauri app: src-tauri/ (Rust shell), src/ (React views), helper/ (Swift)
apps/extension/       MV3 browser extension
crates/core/          data model, events, resource slots, note DSL, report contract, prompts
crates/llm/           Provider trait + Anthropic Messages client (Claude, DeepSeek)
crates/tools/         BUILD-SPEC §5 tools, path denylist, never-overwrite, the tool loop
crates/voice/         sound bus (UI sounds)
crates/mcp/           MCP stdio client
crates/native-host/   Chrome native messaging host
crates/capture/       (capture lives in the Swift helper; see docs/ARCHITECTURE.md)
resources/  skills/  docs/  scripts/
```

Architecture, decisions and what's left: `docs/ARCHITECTURE.md`.

## Licence

MIT. Bundled fonts are OFL (see `resources/fonts/OFL-*.txt`).
