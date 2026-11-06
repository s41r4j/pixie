# Pixie roadmap

What's done, what's open. Per-platform status lives in [CHECKLIST.md](../CHECKLIST.md); release
notes in [CHANGELOG.md](../CHANGELOG.md).

## Done (0.7.x)

- **Windows and Linux**: a native layer (`apps/desktop/src-tauri/src/native/`) answers the Swift
  helper's commands: capture, OCR, UI tree (Windows), input, speech in and out. Agents' tools
  (shell, grep, images, copy, open) no longer call Mac programs. CI checks both on every push;
  the release builds an NSIS installer, an AppImage and a .deb.
- **CHECKLIST.md**: every feature and setting, per platform.
- **Auto-update** from GitHub Releases: signed `latest.json`, checks on launch and every 6 h,
  installs when Pixie is idle, Restart now / Check now in Settings → About and the tray. The
  signing secrets are set in the repo.
- **Settings → Experimental**, with one switch per feature; tools behind an off switch aren't
  offered to the model.
- **Excalidraw canvas** (standard since 0.7.6, the old board removed): shapes, arrows, text or Mermaid in an editable canvas
  window; open, list, read, clear and delete (to the Trash) by voice; exports to Downloads; hides
  on a click outside.
- **Typing into the right field**: `mac_step type` can click the field first and reports where
  the text went.
- **Lighter on RAM**: the natural voice loads on demand and unloads after 5 quiet minutes; Settings
  and the canvas are created when opened and freed when closed (~730 MB → ~215 MB idle).
- **Pixie knows itself** (`pixie` tool, `src/myself.rs`): answers about its live setup, changes its
  own settings in one call, opens Settings pages, checks for updates, quits, stops all jobs, starts
  a new chat. Short commands ("quit", "stop working", "never mind", "new chat") skip the model.
- **Show Canvas** in the menu bar menu; Pixie draws on its own when it helps (Settings → General →
  Drawing: only when asked / when it helps / often).
- **Personalities** (0.7.6): Pix (default, unchanged), Inos, Xtrem (with Spicy) and Eon, each with
  its own face in the notch, the buddy and Settings; style of replies only.
- **Memory** (`src/memory.rs`, `src/chats.rs`, `src/session.rs`): one chat per day (reset hour in
  Settings → Memory, midnight by default) that survives gaps and restarts; a running summary of
  turns older than the last 12; long-term notes in `memory.json` from "remember…", from quiet
  chats (Automatic) and from jobs' `LEARNED` lines; the top 15 in every prompt, the rest through
  the `memory` tool's search; Settings → Memory to see and edit them.

## Open

- [ ] Try the Windows build on a real PC (voice, OCR, UI Automation, clicks) and the Linux AppImage,
  then turn 🧪 into ✅ in CHECKLIST.md.
- [ ] Watch the first CI run of the Windows/Linux checks and the Linux build after pushing.
- [ ] Hotkeys on Windows checked on a real machine; Linux hotkeys and voice input.
- [ ] Open at login (`tauri-plugin-autostart`).
- [ ] Update channel: stable / beta from prerelease tags.
- [ ] Canvas: the buddy pointer moving to each shape while it draws; a `skills/excalidraw`
  playbook.
- [ ] Personalities: a "Pick your Pixie" step in the welcome window; check tone and length against
  the same 10 questions per personality.
- [ ] Kokoro (natural voice) on Windows; the Chrome bridge on Windows (registry + named pipe).
- [ ] Read PDF / Word / RTF and make PDFs on Windows and Linux.
- [ ] Memory, later: a short weekly tidy-up pass (merge near-duplicates, drop stale learned notes);
  semantic search if keyword search proves too weak; let jobs read and write notes mid-job.
- [ ] **Next (0.7.8): multi-step computer use** that keeps seeing the screen: see below.
- [ ] **Next: context and cost** (cache-friendly prompts, honest usage numbers, smaller screen text): see below.
- [ ] Local coding CLIs as Pixie's brain (Codex, Claude Code, opencode): see below.

## Next fix: multi-step computer use that keeps seeing the screen

**Symptom.** Asked to do something that takes several steps ("open Settings, go to Wi-Fi, turn
it off"), Pixie does the first step and then acts as if it can't see: it stops, repeats itself,
or clicks the wrong thing. Found on 2026-11-07 while reading the code; four causes, all
confirmed:

1. **Voice turns can't look again.** `defs::for_mode(Mode::Voice)` has no `look` tool. After a
   click, `buddy::act` returns only the app, the window, the focused field and 15 control labels
   (`Pointer.afterState()` in the Swift helper). After `type`, `key`, `menu`, `scroll`, `open` or
   `press`, `host.rs` returns the helper's raw JSON with nothing about the screen. From step two
   the model is guessing.
2. **Element ids are renumbered under it.** The screen description lists controls as `e1`, `e2`…;
   every `AX.snapshot` (the one in `afterState`, and every new capture) clears `AX.cache` and
   numbers again from `e1`. The model still holds the first list, so "click e12" on step two can
   land on a different control.
3. **Clicks snap to the old screen.** `buddy::act` and `overlay::locate` snap targets to
   `AppState.last_capture`, set once when the voice turn starts (`session.rs`). After the window
   changes, a click is pulled toward where a control used to be. `look` (live agents) calls
   `screen::capture` but never updates `last_capture`, so live agents snap to whatever the last
   voice turn saw.
4. **No vision on DeepSeek.** The screenshot is only attached when
   `provider.supports_images()`; on DeepSeek the model works from OCR lines and the control list
   only, which is much weaker for "what changed after I clicked".

Also by design: a voice turn stops after 8 steps (`AgentOptions { max_steps: 8 }`) and the voice
prompt hands anything over 2–3 steps to a live agent.

**Plan.**
- Add `look` to voice mode (and Takeover), with a line in the voice prompt: after an action, look
  before the next one when the screen may have changed.
- Every `mac_step` action returns a fresh, compact screen state, not only clicks: app, window,
  focused field, and the controls with **ids and frames** (top 30 by distance to where it acted),
  plus changed text lines. One shape for all actions, built in Rust from a new helper call
  `screen_state` (AX snapshot + optional OCR of the front window only, so it stays fast).
- **Stable ids.** Make ids stable for the life of a turn: the helper keys controls by
  role + label + rounded frame and keeps a turn-scoped map, reusing an id when the same control
  is still there and issuing new ones (`e61`…) for new controls. A `turn` counter resets the map
  when a new voice turn or job step starts. The state returned after each action says which ids
  are gone.
- **Snap against the latest screen.** Every capture, `look` and post-action state replaces
  `last_capture`; snapping ignores a capture older than the last action.
- **Vision.** When the model can't take images, say so in Settings → Models next to the voice
  model ("can't see screenshots; computer use works from on-screen text") and suggest a vision
  model for live agents; let live agents use the agents model, which can differ.
- Raise voice `max_steps` to 12 for turns that are clearly a short task, and keep the live-agent
  handoff for longer ones.

**Test.** A scripted 4-step task on a fixture window (open a menu, pick an item, type into the
field that appears, press Save) run against a fake model that follows the returned ids, plus a
real run in System Settings and Finder on Claude and on DeepSeek. Pass when every step targets the
right control and the turn ends with the task done.

## Next fix: context and cost

**What's sent today (measured 2026-11-07).** One voice step sends roughly 5,000–12,000 input
tokens, and every step of a turn sends it again:

| Part | Size per step |
|---|---|
| System prompt (voice) | ~6.2k chars ≈ 1,600 tokens |
| Built-in tool definitions (13 voice tools) | ~5.3k chars ≈ 1,400 tokens |
| Extra tools: `pixie`, `memory`, `canvas`, apps, connectors | ≈ 1,500+ tokens (connectors can add many more) |
| Screen description: OCR lines + up to 60 controls | 1,000–8,000 tokens, the biggest and most variable part |
| History (last 12 turns) + day summary + 15 memory notes | ≈ 1,000 tokens |
| Screenshot image (vision models only) | ≈ 1,000–1,600 tokens |

**What's already fine.** Context doesn't pile up over a day: history is capped at 12 turns plus a
running summary, long-term memory sends at most 15 notes, and tool results (fetched pages,
connector output) aren't kept in history.

**What's wrong.**
1. **Usage under-reports.** `usage.rs` records `input_tokens` from the Anthropic-style response,
   which excludes `cache_read_input_tokens` and `cache_creation_input_tokens`. On DeepSeek
   (automatic prefix caching) the log shows ~200–500 input tokens per step instead of thousands,
   so Settings → Usage shows less than is actually billed (cache reads are cheap, not free).
2. **No prompt caching on Claude.** No `cache_control` breakpoints are set, so Claude bills every
   step at the full input price.
3. **The cache breaks early on DeepSeek too.** The system prompt has the time to the minute and
   the screen description in the middle (`ENVIRONMENT`, then `WHAT'S ON THEIR SCREEN`), so the
   reusable prefix ends there; history after it is re-billed every turn.
4. **Jobs grow fast.** A job runs up to 60 steps and every tool result stays in its transcript,
   so late steps resend everything before them.

**Plan.**
- **Honest numbers.** Parse `cache_read_input_tokens` / `cache_creation_input_tokens` (Anthropic
  and DeepSeek's Anthropic endpoint) and OpenAI's `prompt_tokens_details.cached_tokens`; store
  them in `usage.jsonl`; price cache reads and writes at their own rates; show "cached" in
  Settings → Usage.
- **Cache-friendly order.** Fixed first, changing last: tools → identity, persona, mode, policy,
  skills, memory (sorted, stable) → history → a final user message that carries the time, the
  front app and the screen description. Add `cache_control` breakpoints after the tools, after
  the fixed system part and after the history for Claude. Expect most of each step to become a
  cache read on both providers.
- **Smaller screen text.** By default send the 30 controls and 40 text lines nearest the pointer
  or the thing they asked about, with a line saying how many more there are; `look` (with an
  optional `full=true`) returns the rest when needed. Skip the screen entirely for questions
  that clearly aren't about it ("what's the capital of…").
- **Fewer tool bytes.** Shorter tool descriptions (the long teaching text moves to the `about`
  action and playbooks); connectors' tools offered only when connected and, past ~20, only the
  ones whose server matches the request (a cheap name/keyword filter).
- **Jobs.** After step 15, replace tool results older than the last 6 steps with a one-line
  stub ("read file x, 120 lines"), keeping the plan and note current; cap any single tool result
  at ~20k characters in the transcript.

**Test.** Record input / cached / output tokens for the same 10 voice questions and one 20-step
job before and after, on DeepSeek and Claude; ship when the billed tokens per step drop by at
least half with no change in answers.

## Next idea: Codex, Claude Code and opencode as Pixie's model

**Why.** Today Pixie needs an API key (Claude, OpenAI or DeepSeek). Many people already pay for a
Codex (ChatGPT), Claude Code (Claude Pro/Max) or opencode subscription, or want it free: opencode
ships free models. If Pixie could run on a CLI already installed and signed in on their computer,
they'd use Pixie with no key and, with opencode, no cost.

**What's there (checked 2026-11-07 on the dev Mac).** opencode 2.2.5, Codex CLI 0.166.0 and Claude
Code 2.1.319 are installed. `opencode models` lists free models such as
`opencode/deepseek-v4.1-flash-free`, `opencode/glm-5.3-flash-free`,
`opencode/nemotron-3.5-super-vl-free` and `opencode/mimo-v2.6-flash-free`, besides providers the
user has connected (DeepSeek, Cloudflare Workers AI…). Headless entry points:

| CLI | Headless mode | Streaming | Images | Their tools off | Sign-in |
|---|---|---|---|---|---|
| opencode | `opencode serve` (local HTTP API + SSE; also `opencode run`, `opencode acp`) | yes | file parts | agent config with tools disabled | `opencode auth`, free models need none |
| Claude Code | `claude -p --input-format stream-json --output-format stream-json`, one process kept alive | yes | image blocks | `--tools ""` / `--disallowedTools`, `--system-prompt` | Claude subscription login |
| Codex | `codex exec --json`, or its app-server / `codex mcp-server` | yes | `-i` | read-only sandbox, no shell | ChatGPT login |

**The hard part is Pixie's tools.** A direct API takes tool definitions (highlight, mac_step,
draw, canvas, note, memory, pixie…); these CLIs are agents with their own tools. Plan:
1. **Pixie as an MCP server.** Add a small local MCP server (stdio or a localhost port with a
   per-run token) exposing Pixie's tools; `crates/mcp` already has the client side and
   `crates/native-host` shows the framing. Each tool call runs through `AppHost::call` exactly as
   it does today.
2. **A `local` provider** in `crates/llm` implementing `Provider` over the CLI: keep one CLI warm
   (opencode: `opencode serve` on a random port; Claude Code: a persistent stream-json process;
   Codex: app-server), pass Pixie's system prompt and the screenshot, register Pixie's MCP server,
   switch the CLI's own shell/edit tools off, and stream text deltas back as `AgentEvent::Text`
   so speech starts on the first sentence. The CLI runs the tool loop; Pixie reports tool starts
   and ends from the MCP side.
3. **Settings → Models → "On this computer"**: detect each CLI (`coders::detect` already does),
   show signed-in or not, list models (`opencode models`, Claude Code's model aliases, Codex's
   models) with **Free** badges, and let them pick one for voice and one for jobs. The Usage page
   shows $0 / "subscription" for these.
4. **Order:** opencode first (free models, cleanest HTTP API; ~2–3 days with testing), then Claude
   Code, then Codex.

**Check before shipping.**
- Terms: whether Anthropic's and OpenAI's current terms allow a consumer subscription to drive
  another app this way. Confirm per provider; don't ship a mode that breaks them.
- Voice latency: time to first spoken word versus a direct API. A warm process is a must; measure
  and show the expected speed next to each model.
- Tool use: free models may call Pixie's tools badly. Test the voice flows (point, click, type into
  the right field, canvas, settings) per model and mark the good ones.
- Version churn: these CLIs change often. Pin tested versions, check `--version`, and fall back
  to a clear "update opencode" message.
- Privacy: screenshots go to whichever model the CLI uses. Show that in Models.

## Rules that stay

- Never commit `.env`, `work/`, `heyclicky/`; never reuse HeyClicky's proprietary assets.
- Keep the binary `openclicky` and the identifier `com.openclicky.app`.
- Pixie never sends mail; it only drafts.
- The updater's private key and password never go in the repo.
