# :) Pixie

Pixie (formerly openclicky) is an open-source, macOS-first companion that lives in your notch
(with Windows and Linux previews: see [CHECKLIST.md](CHECKLIST.md)). Hold **⌃ control + ⌥ option**
and talk: it hears you, looks at your screen, answers out loud, points at things, draws on the
screen with its own little pointer, clicks for you when you ask, and hands bigger work to
background jobs (including coding through Codex CLI, Claude Code or opencode) that write a live
note. It remembers you (one chat all day, plus long-term notes kept on your computer), you can
set it up by asking ("talk slower", "switch to Eon", "quit"), draws explanations on an editable
Excalidraw canvas, comes in four personalities (Pix, Inos, Xtrem, Eon), and updates itself from
GitHub. Tauri 2 + Rust + React + a small Swift helper. Claude, OpenAI or DeepSeek as the brain.

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

Background jobs can hand code to the coding agents already on your Mac: **Codex CLI**,
**Claude Code** or **opencode** (whichever is installed; T3 Code users have these underneath).
Ask for an app, a script or a fix and the job runs the agent in your repo or its output folder,
streams its progress to the job card, and checks the result.

## The notch

On a Mac with a notch, Pixie rests invisibly over it. Voice turns stay in the notch's "ears":
it widens while listening, thinking and speaking, and never shows the answer as text. **Hover
the notch** for two tabs: **Chat** (today's conversation; type a follow-up or start a new chat) and
**Agents** (plans, progress, questions to answer, stop). Hold ⌃⌥ any time to talk; it opens on
whichever screen you're using.

## Voice

Answers are spoken by **Kokoro**, a natural neural voice that runs offline (sherpa-onnx). Get it
from Settings → Voice → Download natural voice (~350 MB, from the k2-fsa/sherpa-onnx GitHub
releases, stored in `~/Library/Application Support/com.openclicky.app/voices`). Pick from eight
voices and set the speed there; the system voice is the fallback. The model loads when you start
talking (about a second) and unloads after five quiet minutes, so idle Pixie stays near 215 MB.

## Memory

- **Today's chat** carries on after quiet gaps and restarts until **New chat** (or saying "new
  chat") or the day's reset hour (midnight by default; Settings → Memory). The last 12 turns go to
  the model in full, older ones as a running summary.
- **Long-term notes** (`memory.json` in the support folder): "remember that…" / "forget…"; on
  Automatic, Pixie also learns lasting facts from a chat once it has been quiet a few minutes, and
  jobs add their `LEARNED` lines. The 15 most useful go into every prompt; the rest are found with
  the `memory` tool. Passwords, keys and card numbers are refused. Settings → Memory lists, edits
  and deletes them, and switches memory to Automatic, Only when I ask, or Off.

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
| "What can you do?" / "What's my talk shortcut?" | Answers from its live setup (the `pixie` tool) |
| "Talk slower and use the Emma voice" | Changes its own settings at once, no clicking through Settings |
| "Remember that my manager is Priya" | A long-term note; "forget…" removes it |
| "Quit" / "Stop working" / "Never mind" / "New chat" | Quits / stops every job / stops talking / starts a fresh chat, instantly |
| "Draw how OAuth works" | An Excalidraw canvas comes up with the diagram; edit it, keep it, export it. Show Canvas in the menu brings it back |
| "Switch to Eon" / "Be Xtrem" | Changes the personality (Pix, Inos, Xtrem, Eon) and the face in the notch |

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

`.github/workflows/build.yml` builds Pixie for **macOS** (universal `.dmg` + `.zip`),
**Windows** (x64 NSIS `-setup.exe`) and **Linux** (x64 `.AppImage` + `.deb`) on every push to
`main` that touches the app; the files are attached to the run as artifacts. Run it by hand from
Actions → Build, or push a tag like `v0.5.1`, to also publish a GitHub release; the website's
Download buttons point at `releases/latest/download/Pixie-macOS-universal.dmg`,
`…/Pixie-Windows-x64-setup.exe` and `…/Pixie-Linux-x64.AppImage`.
Windows and Linux are previews. On the Mac a Swift helper does the screen, voice and input work;
on Windows and Linux `apps/desktop/src-tauri/src/native/` answers the same commands (Windows OCR,
UI Automation, SendInput and speech recognition; Linux uses grim/scrot, tesseract, xdotool and
espeak-ng). **[CHECKLIST.md](CHECKLIST.md)** ticks every feature and setting for each platform. Optional secrets for
a signed, notarized Mac build: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`.

**Auto update.** Installed copies check `releases/latest/download/latest.json` (written by the
release job) and install signed updates themselves (`src-tauri/src/updates.rs`). The signing key
pair is made once with `npx tauri signer generate`; the public key is in `tauri.conf.json`
(`plugins.updater.pubkey`), and the private key and its password go in the repo secrets
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Without them, builds still
work but carry no update packages. Keep the private key safe: losing it means installed copies
can't verify new versions.

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
apps/desktop/         Tauri app: src-tauri/ (Rust shell; native/ for Windows and Linux),
                      src/ (React views), helper/ (Swift), voice/ (Kokoro sidecar)
apps/extension/       MV3 browser extension
crates/core/          data model, events, resource slots, note DSL, report contract, prompts
crates/llm/           Provider trait + Anthropic Messages client (Claude, DeepSeek)
crates/tools/         BUILD-SPEC §5 tools, path denylist, never-overwrite, the tool loop
crates/voice/         sound bus (UI sounds)
crates/mcp/           MCP stdio client
crates/native-host/   Chrome native messaging host
site/                 the website (pages, assets, screenshots; built by scripts/build-site.sh)
resources/  skills/  docs/  scripts/
```

Architecture and decisions: `docs/ARCHITECTURE.md`. What's open: `docs/ROADMAP.md`.

## Licence

MIT. Bundled fonts are OFL (see `resources/fonts/OFL-*.txt`).
