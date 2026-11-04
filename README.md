# openclicky

An open-source, macOS-first AI companion that lives in the menu bar / notch, hears you, sees
your screen, draws on it, and runs background jobs. Tauri 2 + Rust + React + TypeScript.

> Status: **M1 (shell)**. Tray app, notch panel, hold-to-talk hotkey, permissions wizard,
> resource slots and sound bus. Voice (M2) is next. See `docs/M1.md`.

## Run

Requirements: macOS 14+, Rust (stable), Node 20+.

```bash
npm install
npm run dev            # tauri dev: Vite on :1420 + the Rust app
```

Hold **⌃ control + ⌥ option** to open the panel (you'll hear the launch sound). Release and
it stays open; tap the chord again or press **esc** to close it. Clicking the menu-bar icon
toggles it too.

Other commands:

```bash
npm run check          # cargo check --workspace && frontend build
npm test               # cargo test --workspace
npm run placeholders   # regenerate any missing media in resources/ (never overwrites)
npm run app:build      # .app + .dmg for this machine's arch
npm run app:build:universal   # needs: rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

## Layout

```
apps/desktop/         Tauri app (src-tauri/ = Rust shell, src/ = React panels)
apps/extension/       MV3 browser extension (M7)
crates/core/          data model, events, resource slots
crates/voice/         audio bus now; STT/TTS/VAD/AEC traits in M2
crates/llm/           provider trait + Anthropic (M2)
crates/tools/         agent tools (M5)
crates/capture/       ScreenCaptureKit + annotations (M3)
crates/mcp/           connector host (M6)
crates/native-host/   Chrome native messaging host (M7)
resources/            media by slot name, see resources/README.md
skills/  docs/
```

## Licence

MIT. Bundled fonts are OFL (see `resources/fonts/OFL-*.txt`).
