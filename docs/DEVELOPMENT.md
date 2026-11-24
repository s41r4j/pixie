# Developing Pixie

Building, signing, releasing and the repo layout. What Pixie does is in the [README](../README.md);
how it works inside is in [ARCHITECTURE.md](ARCHITECTURE.md).

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

## Keys and models (developer details)

Keys are read from (first wins): Settings → Models, the process environment,
`~/Library/Application Support/pixie.s41r4j.in/.env`, then the repo `.env` in dev builds.
`ANTHROPIC_API_KEY` → Claude (`claude-opus-5-5`, the default provider); `DEEPSEEK` /
`DEEPSEEK_API_KEY` → DeepSeek's Anthropic-compatible endpoint (`deepseek-flash`, which also
takes screenshots); `OPENAI_API_KEY` → OpenAI. Settings → Models lists each provider's models
live (newest first, with context size and price) and lets you pick one for voice and one for
agents. Settings → Usage prices every call from the public model price list (OpenRouter's
catalogue, cached in `prices.json`, refreshed daily), with tokens read from the provider's prompt
cache counted and priced separately. Prompts are built for caching: the system prompt holds only
what stays the same, and the time, front app and screen lead the newest message. See
`.env.example` for overrides.

No API key? Install [opencode](https://opencode.ai) and pick **Settings → Models → On this
computer**: Pixie runs on opencode's free models (or any provider you signed in to there), with
opencode's own tools off and Pixie's tools given to it.

Typed questions can get typed answers: Settings → General → **Typed questions get typed
answers**, and **Show typed answers in the notch** to read them there.

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
espeak-ng). **[CHECKLIST.md](../CHECKLIST.md)** ticks every feature and setting for each platform. Optional secrets for
a signed, notarized Mac build: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`.

**Self-signed releases.** Without a Developer ID, releases are signed with Pixie's own
self-signed certificate ("Pixie Self-Signed", made once with openssl, valid to 2046). macOS ties
Screen Recording, Accessibility and the microphone to the certificate, so they stay on across
updates; an ad-hoc signature changes with every build and turns them off. The repo secrets are
`PIXIE_CODESIGN_P12` (the .p12, base64) and `PIXIE_CODESIGN_PASSWORD`; the build puts the
certificate in a keychain of its own because Tauri only imports Apple-issued certificates itself.
The .p12 and its password are kept with the updater key, never in the repo. Losing them means one
more round of permissions for everyone, not a broken update. First opens still need right-click →
Open, as before; only a Developer ID removes that.

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

The pictures of Pixie at work (`site/shots/buddy.png`, `ask.png`, `do.png`, also used in the README)
are drawn by Pixie's real overlay over stand-in desktops in `scripts/shots/scenes.html`: start the
preview with `npm run dev -w apps/desktop`, then run
`node scripts/shots/make.mjs <path to chrome-headless-shell> [buddy|ask|do]`.

## Browser and connectors

- **Chrome/Brave/Edge/Arc**: the app registers its native messaging host on launch. Load the
  extension once: `chrome://extensions` → Developer mode → Load unpacked →
  `~/Library/Application Support/pixie.s41r4j.in/extension`. Then "what tabs do I have open",
  "fill this form with…", "compare these two pages" work. Details: `apps/extension/README.md`.
- **Connectors (MCP)**: turn them on in Settings → Experimental → Connectors (off by default since
  0.8.0 unless you'd already added one; off means no connector servers run). Then pick them in
  Settings → Connectors, or add servers to
  `~/Library/Application Support/pixie.s41r4j.in/mcp.json` (Claude Desktop format, examples
  inside). Their tools show up as `mcp__<server>__<tool>`.
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

Architecture and decisions: [ARCHITECTURE.md](ARCHITECTURE.md). What's open: [ROADMAP.md](ROADMAP.md).
Third-party licences: `npm run notices` rewrites [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) after a dependency change.
