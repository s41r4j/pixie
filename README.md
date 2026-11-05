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
Screen Recording, Accessibility) and the model key. Screen Recording only applies to a fresh
process: after switching it on, use **relaunch openclicky** in the window.

macOS ties permissions to the app's signature. Default builds are ad-hoc signed, which changes
every build, so after a rebuild remove the old openclicky rows in System Settings → Privacy &
Security (the − button) and allow the new one. To keep permissions across rebuilds, create a
local signing identity once and build with it:

```bash
sh scripts/create-signing-identity.sh
APPLE_SIGNING_IDENTITY="openclicky Local Signing" npm run app:build
```

## The notch

On a Mac with a notch, openclicky rests invisibly over it. **Hover the notch** to peek: Talk
(click to talk, click Done), Type, Jobs and Settings, the jobs that are running or need you,
and your last answer. Hold ⌃⌥ any time to talk; it opens on whichever screen you're using.

## Voice

Answers are spoken by **Kokoro**, a natural neural voice that runs offline (sherpa-onnx). Get it
from Settings → Voice → Download natural voice (~350 MB, from the k2-fsa/sherpa-onnx GitHub
releases, stored in `~/Library/Application Support/com.openclicky.app/voices`). Pick from eight
voices and set the speed there; the system voice is the fallback.

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
(see `resources/README.md`). The shipped icons, sounds, mic/system glyphs and brand colours come
from the OpenClicky Design System v2 in `design/` (see its `reference.md` and
`REPLACEMENT_MAP.csv`). `npm run assets:local -- --from DIR` builds a private local pack
in Application Support that overrides the bundled media on this Mac only; never commit
third-party media.

## Commands

```bash
npm run check          # cargo check --workspace && frontend build
npm test               # cargo test --workspace
npm run app:build      # .app + .dmg for this machine
npm run app:build:universal   # needs: rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run placeholders   # regenerate missing placeholder media (never overwrites)
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
