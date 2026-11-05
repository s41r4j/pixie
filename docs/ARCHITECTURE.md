# openclicky architecture (v0.2)

```
                 ┌──────────────── openclicky.app (Tauri, Rust) ────────────────┐
 hold ⌃⌥ ──────▶ │ hotkey poll (20 ms) ─▶ session (voice loop) ─▶ llm::Provider ─│──▶ Claude / DeepSeek
                 │        │                    │  tools::run (tool loop)          │     (Anthropic Messages)
                 │        ▼                    ▼                                  │
                 │  panel / overlay / shelf / note / settings  (React webviews)  │
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
4. Streamed answer → cut at sentence ends → spoken by AVSpeechSynthesizer while still streaming;
   `speak_word` events light up words in the notch.
5. Tools during the turn: `highlight`, `draw` (overlay), `mac_step` (AX), `note`/`note_answer`
   (the paper card), `start_job`, web, device, browser, connectors. ≤ 8 steps.
6. History (text only) kept for follow-ups; resets after 10 idle minutes. The notch collapses
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

## Decisions (simplest behaviour-preserving option; TODOs)

| Topic | Choice | TODO |
|---|---|---|
| Native APIs | Swift helper sidecar over JSON lines instead of a static-lib shim | — |
| STT / TTS | Apple Speech (on-device) + AVSpeechSynthesizer: free, local, low latency | Streaming cloud STT/TTS adapters (Deepgram/ElevenLabs) behind the same calls |
| Models | One Anthropic-Messages client for Claude and DeepSeek; `thinking` off for voice on DeepSeek, `effort` for Claude | OpenAI-compatible adapter |
| Hotkey | Polling `CGEventSourceFlagsState` (no permission needed) | Event tap to swallow keys |
| Notch window | Transparent borderless NSWindow over the menu bar; click-through except the drawn shape (`hit.rs`) | NSPanel subclass |
| Web search | DuckDuckGo HTML, Bing fallback (no key) | Pluggable search API |
| Browser | `chrome.scripting` + DOM refs; the app binary doubles as native host | CDP (`chrome.debugger`) for trusted input, downloads, dialogs |
| Connectors | MCP stdio servers from `mcp.json` | HTTP transport + OAuth sign-in UI |
| Polish (M8) | Onboarding, edge cards, pill, toasts, take-over via voice + pointer | Handoff (drag a region into an app), usage meter, updater, notarized build |
| `start_job` tool | Added (voice needs to hand work off); not in §5's table | Fold into the spec |

## Tests (`npm test`)

note DSL parser · report parser · prompt goldens per mode · path denylist + never-overwrite ·
tool loop against a scripted provider · SSE accumulator · provider config · MCP client against a
fake stdio server · native-messaging framing · notes ops · web parsing · hotkey chords ·
sentence splitter · dotenv · skills loader · resource slots · audio bus.
