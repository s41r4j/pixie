# :) Pixie

Pixie (formerly openclicky) is an open-source, macOS-first companion that lives in your notch. Hold **⌃ control + ⌥ option**
and talk: it hears you, looks at your screen, answers out loud, points at things, draws on the
screen with its own little pointer, clicks for you when you ask, and hands bigger work to
background jobs (including coding through Codex CLI, Claude Code or opencode) that write a live
note. Tauri 2 + Rust + React + a small Swift helper. Claude, OpenAI or DeepSeek as the brain.

Website: https://pixie.s41r4j.in · Download: [latest release](https://github.com/s41r4j/pixie/releases/latest)

## Run it

Requirements: macOS 14+, Xcode command line tools (for `swiftc`), Rust stable, Node 20+.

```bash
cp .env.example .env          # put your key in: DEEPSEEK=sk-…  (or ANTHROPIC_API_KEY=…)
npm install
npm run app:build             # → target/release/bundle/macos/Pixie.app (+ .dmg)
npm run app:install           # copy to /Applications and open
```

Development: `npm run dev` (Vite on :1420 + the app with hot reload).

On first launch a welcome window walks through permissions (Microphone, Speech Recognition,
Screen Recording, Accessibility) and the model key. Screen Recording only applies to a fresh
process: after switching it on, use **relaunch Pixie** in the window.

macOS ties permissions to the app's signature. Default builds are ad-hoc signed, which changes
every build, so after a rebuild remove the old Pixie rows in System Settings → Privacy &
Security (the − button) and allow the new one. To keep permissions across rebuilds, create a
local signing identity once and build with it:

```bash
sh scripts/create-signing-identity.sh
APPLE_SIGNING_IDENTITY="Pixie Local Signing" npm run app:build
```

## The pointer

Pixie has a round `:)` buddy that rides just off your cursor. When it talks about something it
flies there and draws (circle, underline, arrow, a marching-ants selection). Ask it to *do*
something and it clicks, double-clicks, right-clicks, types, drags or copies to the clipboard,
one step at a time, checking the screen after each step. Settings → General → Buddy: **Always
on**, **Idle mode** (only while Pixie is busy) or **Off**; **Let Pixie click and type for me**
(off = pointer only).

## Agents

Bigger work runs as an agent, tracked in the notch's **Agents** tab (hover the notch):

- **Background** agents work on their own (research, documents, files, code) and write a live
  `note.md`.
- **Live** agents do the task on your real screen while explaining each step out loud; esc
  stops them.

## Coding with local agents

Background jobs can hand code to the coding agents already on your Mac — **Codex CLI**,
**Claude Code** or **opencode** (whichever is installed; T3 Code users have these underneath).
Ask for an app, a script or a fix and the job runs the agent in your repo or its output folder,
streams its progress to the job card, and checks the result.

## The notch

On a Mac with a notch, Pixie rests invisibly over it. Voice turns stay in the notch's "ears":
it widens while listening, thinking and speaking, and never shows the answer as text. **Hover
the notch** for two tabs: **Chat** (this conversation; type a follow-up or start a new chat) and
**Agents** (plans, progress, questions to answer, stop). Hold ⌃⌥ any time to talk; it opens on
whichever screen you're using.

## Voice

Answers are spoken by **Kokoro**, a natural neural voice that runs offline (sherpa-onnx). Get it
from Settings → Voice → Download natural voice (~350 MB, from the k2-fsa/sherpa-onnx GitHub
releases, stored in `~/Library/Application Support/com.openclicky.app/voices`). Pick from eight
voices and set the speed there; the system voice is the fallback.

## Using it

| Do this | What happens |
|---|---|
| Hold **⌃⌥** and talk, let go | The notch opens with a live waveform and your words; it answers out loud, the words light up as they're spoken |
| "Where's the share button?" | The buddy flies to the button and circles it |
| "Circle the total" | Hand-drawn marks on the screen that fade away |
| "Click Export for me" | `mac_step` presses it through Accessibility |
| "Make me a one-page PDF about tea" | A background agent starts; hover the notch → Agents for its plan; the note fills live |
| "Rename these slides for me" | A live agent does it on screen, explaining each step |
| An agent asks something | The Agents tab shows the question with tap answers |
| Tap **⌃⌥** (or click the menu-bar `:)`) | A text box to type instead of talk |
| Hold **fn⌃** and talk | Dictation: cleaned-up text is typed into whatever field has focus |
| **esc** | Stops speech, the turn and closes the notch |

## Keys and models

The app's internal identifier stays `com.openclicky.app` (so permissions and data carry over);
everything you see says Pixie.

Keys are read from (first wins): Settings → Models, the process environment,
`~/Library/Application Support/com.openclicky.app/.env`, then the repo `.env` in dev builds.
`ANTHROPIC_API_KEY` → Claude (`claude-opus-5-5`, the default provider); `DEEPSEEK` /
`DEEPSEEK_API_KEY` → DeepSeek's Anthropic-compatible endpoint (`deepseek-flash`, which also
takes screenshots); `OPENAI_API_KEY` → OpenAI. Settings → Models lists each provider's models
live (newest first, with context size and price) and lets you pick one for voice and one for
agents. Settings → Usage prices every call from the public model price list (OpenRouter's
catalogue, cached in `prices.json`, refreshed daily). See `.env.example` for overrides.

## Releases

Push a tag like `v0.5.0` and `.github/workflows/release.yml` builds a universal DMG + zip from
source and publishes a GitHub release; the website's Download buttons point at
`releases/latest/download/Pixie-macOS-universal.dmg`. Optional secrets for a signed, notarized
build: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD`, `APPLE_TEAM_ID`.

The website lives in `site/`: Home (with the interactive demo), Features, Download, Docs and
Changelog, sharing `site/assets/site.css` and `site/assets/site.js`. `npm run site` builds it into
`_site/` with `scripts/build-site.sh` and serves it on http://localhost:8000; `pages.yml` deploys the
same build (Settings → Pages → Source: GitHub Actions).

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
from Pixie's Pixel Design System v1 (monochrome pixel style, IBM Plex Mono / VT323 /
Press Start 2P under OFL; the source pack lives outside the repo). `npm run assets:local -- --from DIR` builds a private local pack
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
site/                 the website (pages, assets, screenshots; built by scripts/build-site.sh)
resources/  skills/  docs/  scripts/
```

Architecture, decisions and what's left: `docs/ARCHITECTURE.md`.

## Licence

MIT. Bundled fonts are OFL (see `resources/fonts/OFL-*.txt`).
