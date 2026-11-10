# Pixie roadmap

What's done, what's open. Per-platform status lives in [CHECKLIST.md](../CHECKLIST.md); release
notes in [CHANGELOG.md](../CHANGELOG.md).

The rounds below marked 0.7.8 to 0.7.17 were local test builds; they ship together as **0.8.0**.

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
- [x] **0.7.8, fixes first:**
  - [x] Natural voice skipped words it didn't know ("opened", "GitHub", every number). sherpa-onnx
    2.x dropped its espeak fallback, so Pixie phonemizes itself (`crates/voice/src/say.rs`:
    numbers and symbols to words, the lexicon, endings, compounds, acronyms, known pieces) and
    the sidecar speaks the phonemes. Checked: 0 skipped words where 7 were skipped before.
  - [x] Voice too quiet (about -24 dB RMS): leveled to -16 dB with a soft limiter, plus Settings →
    Voice → Loudness (Quiet / Normal / Loud).
  - [x] Persona switches: more ways of asking switch at once, and after a turn Pixie makes the
    switch itself when the request was clear and the model didn't.
  - [x] Connectors behind a switch in Settings → Experimental (tab, servers and tools off when off;
    on for people who already added one). Chrome moved to Settings → Apps.
  - [x] Update checks retry in 15 minutes when GitHub answers with an error (seen: 504s).
- [x] **0.7.8: multi-step computer use** that keeps seeing the screen: see below. Done except a
  real run on Claude (no Claude key on the dev Mac).
- [x] **0.7.9: context and cost** (cache-friendly prompts, honest usage numbers, smaller screen text): see below.
  Claude's cache marks are unit-tested but not yet run against Claude (no key on the dev Mac).
- [x] **0.7.9: answer the way you asked**: Settings → General → Typed questions get typed answers,
  and Show typed answers in the notch.
- [x] **0.7.10: opencode as Pixie's brain** (Settings → Models → On this computer): see below.
  Free models not yet measured: opencode's free quota was used up on the dev Mac all day.
- [ ] Claude Code and Codex subscriptions as Pixie's brain: **not allowed as things stand** (terms
  checked 2026-11-09, see below). Don't build without Anthropic's / OpenAI's approval.
- [x] **0.7.11, fixes and coding:**
  - [x] Long questions kept only the last sentence: on-device speech restarts its transcript after
    a pause (and can end mid-hold); the helper now keeps every piece and listens on.
  - [x] "(Tools used for this answer…)" spoken and shown after answers: that history note was
    copied by the model; removed, and old answers are scrubbed.
  - [x] The notch lingered after answers: it now closes 0.8 s after the last word (1.5 s when
    nothing was spoken), and no longer waits when speech ended before the model call did.
  - [x] Deleted jobs stayed in the Jobs window until reopened.
  - [x] Switching provider could keep a model the new provider doesn't have (an opencode id on
    DeepSeek): spoken model names are matched against the provider's live list, choices it
    doesn't have are dropped on a switch, and a "model doesn't exist" error resets to the
    default and asks again once.
  - [x] Dictate shortcut as a toggle (press to start, press to stop), or hold as before.
  - [x] Pixie sees its jobs: a short list in every turn and `pixie action=jobs` with plan,
    progress, report, errors and which coding agent did the work (recorded on the job).
  - [x] Coding tasks go to the agent with usage left: Codex's 5-hour and weekly limits are read
    from its own logs, Claude Code's are set in Settings → Coding (plenty / low / out), and
    opencode's free models take over when both are out. The brief names the agent's folder
    (a job once got output/output/).
- [x] **(18) Type as you speak: live text while talking or dictating** (0.7.14: dictation types into the focused field as you speak, diffing each new transcript against what's typed and replacing the tail; the tidied text replaces it at the end; stops if focus moves; the notch already showed the live transcript while talking) (like macOS dictation or
  Claude's voice mode): words appear in the focused field (or the notch's text box) as they're
  spoken, so they can see where it's going and fix it before it's sent; corrections from the
  recognizer replace the last words in place. Plan: stream partials into the field through the
  helper (Accessibility value set, or a marked-text insertion where AX isn't writable), commit on
  release; a setting to keep today's "type it when done".
- [x] **0.7.12: explanations that feel live:**
  - [x] Narrated drawing: marks written inside the spoken sentences (`[[point e12]]`,
    `[[circle "Save"]]`, `[[arrow e3 e7]]`, `[[3d heart]]`) appear as their sentence starts
    playing (`src/narrate.rs`). Checked on DeepSeek Flash: 8 sentences, marks 5 to 7 s apart.
    Not perfect yet: the model sometimes names a thing by a word that isn't on screen (a mark
    that misses is skipped and logged).
  - [x] Fuller explanations when asked to explain or teach (4 to 8 sentences); quick answers stay short.
  - [x] Dictation hands-free: done as a toggle on the dictate shortcut (fn+ctrl) in 0.7.11, not a double-tap.
  - [x] 3D explanations (Experimental): a Three.js window with labelled shapes and arrows you can
    spin (`src/scene3d.rs`, `views/Scene3D.tsx`); `[[3d name]]` turns to a part as it's named.
  - [x] Several displays, first part: one line about the screens only when there are several,
    `look display=other|window|all|<name>`; see below for what's left.
  0.7.15: the notch setting, spanning windows, live agents on their app's screen, the desktops
  line, Windows/Linux `at`. Only a real second-screen test is left.
- [x] **(17) Self-healing, implementation:** see "Next: self-healing" below. 0.7.13 did the log file,
  crash notes (6), last-good copies (2), the problems list (7), the checkup (4), model errors
  noted and explained (part of 3) and the report (8). Left: watchdogs and stuck states (1),
  revoked permissions noticed mid-use and retired models replaced by the newest of the family (rest of 3),
  rolling back a bad update (5). 0.7.14 did those (`watchdog.rs`, `rollback.rs`, `myself::successor`).
  Not covered: the Chrome bridge (it's started by Chrome, not Pixie) and rollback on Windows and
  Linux.
- [x] **0.7.13 fixes:** marks on menu bar icons landed about a quarter too far left (the icons
  weren't in the control list, the model read positions off the smaller screenshot as points):
  the helper lists menu bar status items (`extras`), mark numbers are screenshot pixels and are
  converted. The 3D view lagged (a CSS transition on labels moved every frame); removed, plus
  1.5x pixel ratio and Lambert materials. Live check of the menu bar marks still to do (the Mac
  was locked).
- [ ] **opencode inside Pixie hits the free limit when opencode on its own doesn't** (seen
  2026-11-09: `opencode run` answered on three free models in 3.5 to 5.3 s; Pixie's opencode got
  "429: free usage limit" on every one, and Settings showed "add key"). Pixie runs opencode with
  its own config and data folders, so it doesn't have the sign-in in
  `~/.local/share/opencode/auth.json` and counts as anonymous. Options to decide together: use
  their opencode sign-in (read-only, no copy kept), or tell them to sign in from Settings. Item 3
  stays open until then.
- [x] **(16) Speed and smoothness pass:** every animation smooth and quick (the notch opening on hover
  and closing the instant the pointer leaves, the talk shortcut reacting at once, clicks and
  sounds without delay), with as little CPU, GPU and RAM as possible: measure frame times and
  hotkey-to-listening latency first, then fix what's slow.
  0.7.15: measured with `--hold-talk-test` (the helper holds ctrl+option): keys seen in 22 to
  35 ms, mic on 190 ms later, most of it making the speech recognizer each time. Now kept and
  prewarmed with the audio input: 58 ms (143 ms first press). Hover open 90 → 40 ms, close
  350 + 340 → 120 + 220 ms, notch transitions 420 → 260 ms. Idle: about 0.1% CPU. Still to try:
  animating the notch with transforms instead of width and height (cheaper per frame), and
  creating the overlay window only when a mark is drawn (one fewer web view, about 45 MB).
- [x] (8) macOS lists Pixie under Login Items → "Allow in the Background" (as it does for VS Code, Codex,
  Claude). Pixie installs no login item or launch agent; the entry is macOS noting a menu-bar app
  that runs with no window. Make sure Quit ends every Pixie process. 0.7.14: agents' commands and
  coding agents run in their own process groups, ended on Quit and by the helper if Pixie dies.

## Next: several displays and desktops

**Today (checked in the code 2026-11-09).** The notch rests on the screen with the camera notch
and moves to the mouse's screen when you talk; the buddy follows the mouse across screens; the
screenshot, OCR and control list cover only the mouse's screen (controls of the front window are
cut off at that screen's edge); pointing and drawing can go to any screen. The model is told only
"this display spans x…y": it doesn't know other screens exist. Spaces (several desktops on one
screen): ScreenCaptureKit and Accessibility only see the current Space, which is right; nothing
breaks, but "the window on my other desktop" can't be seen.

**Rule: aware when it helps, silent when it doesn't.** No extra screenshots, no extra OCR, no
per-display state kept between turns. Everything below is either one short line or happens only
when the request asks for it.

**Done in 0.7.12:** the screens line (only with more than one screen, per turn, no capture),
`look display=pointer|window|other|left|right|built-in|external|all|<name>|<number>` (one
capture per screen named; "all" at most 3), the helper capturing the screen at a given point and
reporting the front window's frame. Pixie still looks at the pointer's screen by default (where
"this" usually is); the line tells the model when the front window is elsewhere. **Left:** where
the notch appears (setting), controls of a window that spans two screens, live agents staying on
their app's screen, Spaces lines, Windows/Linux `at`, a test on a real second screen.

**Done in 0.7.15:** Settings → General → Notch on (follow / built-in / a name; `panel::pinned_screen`),
the front window's controls read across the screen edge (the snapshot widens to the window
when it reaches past the screen), live agents' `look` defaulting to the window's screen, the
desktops line (`session::desktops_line`, from the helper's `elsewhere`: the front app's windows
not on screen, only when they ask), and `at` on Windows and Linux. **Left:** a test on a real
second screen (mixed Retina scale factors, mirrored screens, unplugging mid-turn).

**Plan.**
- **One line of awareness, only with more than one screen:** "Screens: Built-in 1710×1107 (left,
  notch), LG 2560×1440 (right). Pointer on LG; front window on Built-in." Built from
  `NSScreen.screens` at turn start (microseconds, no capture). With one screen, nothing is added.
  It goes in RIGHT NOW (per turn), not the cached system prompt.
- **Which screen to look at:** the front window's screen (what they're working in), else the
  pointer's. "Other / left / right / laptop / external / second screen" picks that one. Still one
  screenshot per question.
- **`look display=pointer|other|<name>|all`:** "all" captures each screen, only when asked (it
  costs a screenshot per screen); the others are one capture.
- **Controls follow the chosen screen:** read the front window's controls wherever it is, not cut at
  the mouse's screen edge.
- **Across screens:** global coordinates already span all screens; the buddy flies across, the
  overlay opens on the target's screen and closes on the old one (one overlay window at a time,
  so no extra memory); check scale factors on mixed Retina / non-Retina setups.
- **Where the notch appears** (Settings → General): the screen I'm using (default) / always the
  built-in screen / always <name>. Screens without a notch get the same bar at top centre.
- **Live agents** stay on the screen of the app they work in, even if the mouse moves away.
- **Desktops (Spaces):** stay with the current desktop (that's what's on screen). If the front app
  has windows on another desktop, say so in one line when they ask about "my other desktop";
  switching desktops for them only on request (ctrl+arrow through mac_step key).
- **Edge cases:** mirrored screens count as one; a screen unplugged mid-turn falls back to the
  pointer's and says so; full-screen apps; Sidecar.
- **Test:** needs a second display on the dev Mac; plus unit tests for the screen picker.

## Next: self-healing

**Already heals:** the helper restarts when it exits; model calls retry twice, then the fallback
model; the voice reloads on demand; update checks retry in 15 minutes; connectors have Retry.

**Plan, in this order.**
1. **Watchdogs and three strikes.** Ping the helper, the voice server, connector servers and the
   Chrome bridge every 30 s; restart one that hangs, not only one that exits. Three failures in
   five minutes switches that feature off and says so once, with the fix. No restart loops.
   **Stuck states:** a notch mode with no events for too long (thinking, listening, an overlay that
   stays) resets to idle; the hotkey poller restarts if it stops ticking.
2. **Last-good copies.** settings.json, memory.json and the chats keep a last-good copy on every
   successful save; a file that won't load is restored from it, and Pixie says so instead of
   quietly starting fresh.
3. **Permissions and models.** A revoked permission is noticed when needed: say which one, open its
   pane, check again on return. Model errors are classified (bad key, no credit, rate limit,
   retired model): switch to the fallback and say so once; a retired model id is replaced by the
   newest of the same family from the live list, and they're told.
4. **"Pixie, check yourself"** (also Settings → About → Checkup): helper, mic, speech, screen
   capture, Accessibility, one tiny model call, the voice, connectors, disk space; fixes what it
   can, lists the rest.
5. **Rolling back a bad update (agreed 2026-11-09).** Keep the previous version (~60 MB). Three
   crashes within a minute of starting after an update puts the previous version back and pauses
   updates until the next release.
6. **Crash notes, local only (agreed).** A panic hook writes a crash note to the log; the next launch
   says "Pixie closed unexpectedly, I restarted" with Show log. Nothing is ever sent anywhere.

7. **Understand everyday failures, work around them, and report them.** Errors that aren't code
   bugs (a wrong model id after a provider switch, a model retired, a connector token expired,
   a tool that keeps failing): detect, apply the workaround (reset to the default, retry once,
   switch to the fallback), tell them in one line, and keep a "to fix" note in
   `<app_data>/issues.md` (what happened, when, the workaround, the log lines). The model-id case
   is done in 0.7.11.
8. **Report an issue for them.** "Report this" (or Pixie offering when a workaround was needed):
   draft a GitHub issue (title, steps, version, platform, the log lines with personal paths and
   keys removed), ask which browser they're signed in to GitHub in, open
   github.com/s41r4j/pixie/issues/new there with the draft filled in, and post only after they say
   yes on the draft (posting is outward-facing). Cheap: a template plus the browser tool, no model
   call beyond writing the summary.

**Never:** Pixie changing its own code or settings on its own, telemetry, endless retries,
posting an issue without their yes on the exact text.

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
4. **No vision on some models.** The screenshot is only attached when
   `provider.supports_images()`. Corrected 2026-11-09: DeepSeek's Flash models do see images
   (tested: it named the colour of an attached image, and input tokens rose with it); V4-Pro and
   the older chat / reasoner models are text-only and work from OCR lines and the control list.

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

**Done in 0.7.8.** `screen::multi_step::four_steps_land_on_the_right_controls` runs that fixture
through the real agent loop (the helper's numbering shifts every step; the moved search field
keeps its id; gone ids are refused). Real runs on DeepSeek Flash in the installed app: Finder
(open, Downloads, read the first item) and System Settings (open, look, About by id, point at the
Mac's name by an id issued after the click), both right. Not yet run on Claude.

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

**Done in 0.7.9 (measured 2026-11-09, DeepSeek V4.1 Flash, installed app, same 10 questions).**
Uncached input per step 2,772 → 1,374 (half); the whole prompt 7,540 → 6,017 tokens; billed
input per step (cache reads at the live list's 0.018 vs 0.051 $/M) 4,455 → 3,013, 68% of before;
cost per step 70% of before; the same answers. The rest is the fixed part, system prompt and
tools (about 4,600 tokens a step, all cache reads), which DeepSeek's listed cache price still bills
at 35% of input. The baseline was kind to the old layout: the screen didn't change between
questions, so DeepSeek cached the old screen text too; with a changing screen the old layout
re-billed 6,000+ tokens a step. On Claude (cache reads 0.05×, and no caching at all before) the
same structure should land near a quarter of the old per-step cost; not measured, no key. Next
step if more is needed: fewer or shorter tool schemas for voice (draw's schema, the four device_*
tools as one).

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

**Done in 0.7.10 (opencode).** `src/opencode.rs`. `opencode serve` (2.2.5) starts on demand
with its own config folder (`XDG_CONFIG_HOME`), database (`OPENCODE_DB`) and a random password,
under a small shell that ends it when Pixie goes (force-quit included); stopped after 10 quiet
minutes (~400 MB). An agent `pixie` with every opencode tool denied and only `pixie_*` allowed;
Pixie's system prompt is the session's instructions entry, earlier turns and the screenshot go
in the prompt (data: URI). Pixie's tools are served by a small HTTP MCP endpoint in the app
(localhost, bearer token); opencode's `_meta` session id routes each call to its turn's
`ToolCtx`. Text streams from `/api/event` (`session.text.delta`); `session.usage.updated` goes
to usage.jsonl (free models priced $0). Voice turns use the model's least-thinking variant.
Keys Pixie already has are passed to opencode so its DeepSeek / Claude / OpenAI models work too.
Measured on the installed app through opencode with `deepseek/deepseek-flash` (free models were
rate-limited: "Free usage limit reached" on every free model all day): first spoken words after
1.3 to 2.3 s from a cold start (server ready in ~0.4 s) for plain questions, 3 to 9 s with a tool
call (highlight, pixie about), against 0.7 to 2.2 s on the DeepSeek API directly; answers the
same. A background job ran end to end on opencode (13 steps through Pixie's tools, note and
report right). Still to do: time and tool quality on 2 or 3 free models once the quota allows
(still "Free usage limit reached" later the same day); a live agent on opencode.

**Terms (checked 2026-11-09, official pages).**
- **Claude Code** ([legal and compliance](https://code.claude.com/docs/en/legal-and-compliance)):
  subscription sign-in (OAuth) "is intended exclusively for" subscribers "to support ordinary
  use of Claude Code and other native Anthropic applications"; developers "should use API key
  authentication"; "Anthropic does not permit third-party developers to offer Claude.ai login
  into their own applications, or to route requests through Free, Pro, Max, or Ultra plan
  credentials on behalf of their users." Pixie answering every voice turn through the user's
  signed-in Claude Code is routing Pixie's requests through their plan, so **no**. What stays
  fine: the existing `code` tool, which hands a coding task to their own unmodified Claude Code
  (ordinary Claude Code use), and Claude through an API key. If we want it, ask Anthropic.
- **Codex** ([auth](https://learn.chatgpt.com/docs/auth)): ChatGPT sign-in is documented for
  the ChatGPT app, Codex CLI and IDE extension; "Use API key authentication for programmatic
  Codex CLI workflows". Nothing permits third-party apps to drive Codex on a ChatGPT plan, and
  secondary sources say paid or hosted apps must request access for "Sign in with ChatGPT".
  So **no**, unless OpenAI approves Pixie. The `code` tool handing coding tasks to Codex CLI stays.

**Check before shipping (kept for the record).**
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
