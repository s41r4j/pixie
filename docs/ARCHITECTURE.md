# Pixie architecture (0.7)

Pixie's code and binary are still called `openclicky` (identifier `com.openclicky.app`).

```
                 ┌──────────────── openclicky.app (Tauri, Rust) ────────────────┐
 hold ⌃⌥ ──────▶ │ hotkey poll (20 ms) ─▶ session (voice loop) ─▶ llm::Provider ─│──▶ Claude / OpenAI /
                 │        │                    │  tools::run (tool loop)          │     DeepSeek
                 │        ▼                    ▼                                  │
                 │  panel · overlay · shelf · guide (made at launch)             │
                 │  settings · note · canvas (made when opened, freed on close)  │
                 │        ▲                    │                                  │
                 │   events (openclicky://event)     jobs (workspace, note.md)    │
                 └─────────┬───────────────────────────┬──────────────┬─────────┘
                    JSON lines (stdio)          unix socket         stdio JSON-RPC
                           ▼                           ▼                ▼
                 openclicky-helper (Swift)    native host ⇄ extension   MCP servers
                 Speech · AVAudioEngine ·     (same binary, Chrome      (mcp.json)
                 ScreenCaptureKit+Vision ·     native messaging)
                 AVSpeechSynthesizer · AX ·
                 WebKit PDF · PDFKit text

 openclicky-voice (C, sherpa-onnx Kokoro): natural voice, loaded on demand, unloaded when quiet
 Windows / Linux: no Swift helper; src-tauri/src/native/ answers the same commands in-process
   (Windows: GDI capture, Windows OCR, UI Automation, SendInput, WinRT speech in and out;
    Linux: grim/scrot, tesseract, xdotool, espeak-ng)
```

## Voice turn

1. **⌃⌥ down**: interrupt anything in flight (barge-in), open the notch (`agent-launch`), start
   `listen_start` (on-device Speech when available, mic level meter) and, in parallel, a
   screenshot of the screen under the pointer with OCR lines and the frontmost window's
   controls (`e1`, `e2`… with frames). Our windows are excluded from capture.
2. **⌃⌥ up**: `listen_stop` → final transcript. Empty → "didn't catch that".
3. Prompt = identity → mode → policy → environment → plan/note → screen description → final
   contract (`crates/core/src/prompt.rs`, golden-tested). Screenshot attached when the provider
   takes images and screen sharing is on.
4. Streamed answer → cut at sentence ends → spoken while still streaming, by Kokoro (when
   installed; it starts loading as soon as they start talking) or the system voice;
   `speak_word` events light up words in the notch.
5. Tools during the turn: `highlight`, `draw` (overlay), `mac_step` (AX), `note`/`note_answer`
   (the paper card), `start_job`, web, device, browser, apps, connectors, `canvas` (Excalidraw,
   used on its own when it helps; Settings → General → Drawing), `memory` and `pixie`. ≤ 8 steps.
6. Today's chat carries on all day (last 12 turns in full, older ones summarised). The notch collapses
   7 s after speech ends unless hovered.

## Jobs

`<app_data>/jobs/<id>/{note.md, output/, tmp/, inbox/, AGENTS.md, state.json}`. The job runs
the same tool loop with the job prompt (≤ 60 steps, deeper effort), emits plan/progress/note
events, can `ask_user` (card turns amber, sound, toast), and ends with the report blocks
(`DONE_TITLE`, `NEXT_ACTIONS`, `ARTIFACTS`, `LEARNED`, parsed by `crates/core/src/report.rs`).
`LEARNED` lines go to `<app_data>/memory.md` and come back in the environment block. Follow-ups
from the note window continue the same conversation.

## Hard rules in code

- Path denylist (`.ssh`, `.gnupg`, `Library/Keychains`, `.aws`, `.config/gh`, `.env*`), checked
  on the literal and symlink-resolved path, before any read; `bash` commands naming them are
  refused (`crates/tools/src/guard.rs`).
- `device_save_file` never overwrites (` 2`, ` 3`, …). `write`/`edit` only inside the job
  folder or temp.
- `bash`: job cwd, 300 s default timeout, 50 KB output cap.
- Background jobs can't click around (`mac_step` only `open`); password fields are never typed
  into by the extension; secure input pauses screen capture.

## Updates and experiments

- `updates.rs`: tauri-plugin-updater against `releases/latest/download/latest.json` (minisign
  public key in `tauri.conf.json`; CI signs with the repo secrets). Checks 25 s after launch and
  every 6 h; installs (macOS/Linux) or stages (Windows) the download, restarts only when idle.
- `settings.experimental` holds per-feature switches (none right now; the Excalidraw canvas in
  `canvas.rs`, files in `<app_data>/canvases/`, graduated in 0.7.6 and replaced the old board).
- Personalities: `settings.persona` adds a STYLE block after the identity in `prompt.rs` (Pix adds
  nothing; jobs never get one); faces are `brand-face-*` icons, read by `lib/persona.ts`.

## Decisions (simplest behaviour-preserving option; what's next)

| Topic | Choice | Next |
|---|---|---|
| Native APIs | Swift helper sidecar over JSON lines on macOS; in-process native layer on Windows/Linux | none |
| STT / TTS | Apple Speech on-device; Kokoro (local) or the system voice for answers | Streaming cloud STT/TTS adapters |
| Models | Anthropic Messages for Claude and DeepSeek, an OpenAI client, live model lists with prices | none |
| Hotkey | Polling `CGEventSourceFlagsState` (no permission needed) | Event tap to swallow keys |
| Notch window | Transparent borderless NSWindow over the menu bar; click-through except the drawn shape (`hit.rs`) | NSPanel subclass |
| Web search | DuckDuckGo HTML, Bing fallback (no key) | Pluggable search API |
| Browser | `chrome.scripting` + DOM refs; the app binary doubles as native host | CDP for trusted input; Windows registration |
| Connectors | MCP stdio servers from `mcp.json`, plus a gallery | HTTP transport + OAuth sign-in UI |
| Memory | Webviews only for windows in use; voice model unloads after 5 quiet minutes | none |

See [ROADMAP.md](ROADMAP.md) for what's open.

## Tests (`npm test`)

note DSL parser · report parser · prompt goldens per mode · path denylist + never-overwrite ·
tool loop against a scripted provider · SSE accumulator · provider config · MCP client against a
fake stdio server · native-messaging framing · notes ops · web parsing · hotkey chords ·
sentence splitter · dotenv · skills loader · resource slots · audio bus.
