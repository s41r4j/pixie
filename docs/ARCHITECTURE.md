# Pixie architecture (0.8)

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
3. Prompt (`crates/core/src/prompt.rs`, golden-tested), ordered for prompt caching: tools →
   system prompt with only what stays the same (identity → persona → mode → policy → stable
   environment → plan/note → the job → final contract) → history → the newest user message, which
   starts with `prompt::context`: RIGHT NOW (time, front app, window) and, for voice, the screen
   description (`Capture::describe_for`: the 30 controls and 40 text lines that match the request
   or sit nearest the pointer; `look full=true` for all). Questions with no screen words skip the
   screen (`screen::about_screen`). On Claude, `cache_control` marks the last tool, the system
   prompt and the end of history, plus automatic caching for the turn's own steps
   (`AnthropicProvider::cache_marks`). Screenshot (1280 px wide) attached when the model takes
   images and screen sharing is on. Usage records uncached, cache-read and cache-write tokens and
   prices each at its own rate (`usage.rs`). Jobs cap a tool result at 20k characters and stub
   results older than the last 6 steps from step 15, every 10 steps (`agent.rs`).
4. Streamed answer → cut at sentence ends → spoken while still streaming, by Kokoro (when
   installed; it starts loading as soon as they start talking) or the system voice. For Kokoro,
   Pixie writes numbers and symbols out and turns words into phonemes itself
   (`openclicky_voice::say`, with the model's lexicon plus fallbacks), and the sidecar levels the
   audio to speech loudness;
   `speak_word` events light up words in the notch.
5. Tools during the turn: `highlight`, `draw` (overlay), `mac_step` (AX), `note`/`note_answer`
   (the paper card), `start_job`, web, device, browser, apps, connectors, `canvas` (Excalidraw,
   used on its own when it helps; Settings → General → Drawing), `memory`, `pixie` and `look`.
   ≤ 12 steps. Computer use over several steps (`screen.rs`): every `mac_step` action ends with the
   helper's `screen_state` (Accessibility only: app, window, focused field, controls, text), which
   replaces `last_capture` so clicks snap to the newest screen. Control ids are Pixie's, not the
   helper's: `screen::stabilize` keeps the same id for the same control (kind + label + frame,
   or the only one of its kind and label) for the whole request and reports ids that went away;
   on the Mac an id is pressed at its current position (`press_point`).
6. Today's chat carries on all day (last 12 turns in full, older ones summarised). The notch collapses
   7 s after speech ends unless hovered.

## opencode as the model

`src/opencode.rs`. When Settings picks opencode, `opencode::run_agent` replaces Pixie's tool
loop: one `opencode serve` (own `XDG_CONFIG_HOME` and `OPENCODE_DB`, random password, a shell
watcher that ends it with Pixie, stopped after 10 idle minutes), one session per turn or job run
with agent `pixie` (opencode's tools denied), Pixie's system prompt as the session's
instructions entry, earlier turns and the screenshot in the prompt. Pixie's tools are served
back over a localhost HTTP MCP endpoint with a bearer token; each call is routed by opencode's
session id to that turn's `ToolCtx` and run by `openclicky_tools::execute`. Text streams from
`/api/event`; usage goes to usage.jsonl. `OpencodeProvider` answers plain text calls
(summaries, memory) through `/api/experimental/generate/text`.

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
- `settings.experimental` holds per-feature switches: `connectors`, and `3d` (`scene3d.rs`: the
  `scene3d` tool, a `scene` window running `views/Scene3D.tsx` with Three.js, lazy-loaded, drawing
  only while something moves). The Excalidraw canvas in `canvas.rs`, files in
  `<app_data>/canvases/`, graduated in 0.7.6 and replaced the old board.
- Narrated drawing (`narrate.rs`): marks inside the answer text (`[[circle e12]]`,
  `[[arrow e3 e7]]`, `[[3d heart]]`) are cut out of the stream in `speech.rs`, replaced by
  private-use stand-ins so the sentence splitter knows their sentence, and run on that
  utterance's `speak_start` (at once when nothing is spoken). They go through
  `overlay::highlight` / `overlay::draw`, so they snap to the same controls as the tools.
- Self-healing (`heal.rs`): the logger tees to `<app_data>/logs/pixie.log`; a panic hook writes
  `logs/crash.txt`, picked up on the next launch (a toast, and a problems entry); settings and
  memory are written with `save_safely` (temp file + rename + `.last-good` copy) and read with
  `load_safely` (a damaged file is set aside as `.damaged` and the copy restored);
  `heal::note` keeps `issues.md`; `myself::checkup` and `open_report` back the `pixie` actions
  checkup / issues / report and Settings → About → Health. Reports open
  `github.com/s41r4j/pixie/issues/new?title=&body=` in their browser, redacted (`heal::redact`);
  nothing is posted by Pixie.
- Watchdogs (`watchdog.rs`): every 30 s pings the helper (twice, 8 s), checks the voice server
  for sentences waiting 40 s with no output (`kokoro::hung`) and pings each connector (`ping`,
  silence only counts); a hung part is restarted, three in five minutes and it's left alone
  (`judge`). Every 5 s: the hotkey poller's heartbeat (`hotkey::since_tick`, a replacement poller
  takes over by generation) and notch modes nothing drives any more (`stuck_after`). Helper
  errors that mean a permission is off go through `watchdog::helper_error`: toast, open the
  pane, poll for it to come back.
- Rollback (`rollback.rs`, macOS): `keep_current` copies the bundle to `<app_data>/previous`
  before `update.install`; `launches.json` counts launches that end within a minute without
  Quit; the third in a row on the version an update brought swaps the copy back from a detached
  shell after Pixie exits, and `skip-version` keeps the updater from offering it again.
- Agents' processes (`openclicky_tools::children`): `bash` and coding agents spawn in their own
  process group, listed in `<app_data>/agents.pids`; dropped guards end a stopped command's whole
  group, Quit ends all, and the helper ends the listed groups when its stdin closes.
- Several screens (0.8.0): `panel::pinned_screen` / `home_screen` place the notch by the
  `notch_screen` setting; `AX.snapshot` widens its screen filter to the front window's frame when
  the window crosses the edge; `look` in Live mode defaults to `display=window`; the capture lists
  the front app's off-screen windows (`AX.elsewhere`), used only by `session::desktops_line`.
- Type as you speak (`LiveTyper` in the helper): `listen_start live_type` checks a text field has
  focus; each partial is diffed against what's typed (backspaces, then unicode key events from a
  private event source so held dictate keys don't apply); `live_type_finish` swaps in the tidied
  text; a focus change stops it.
- Several screens (`screen.rs`): `displays()` lists named screens (a main-thread hop, no
  capture); `screens_line` adds one RIGHT NOW line only when there's more than one;
  `look display=…` resolves a screen and the helper's `capture` takes an `at` point.
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
