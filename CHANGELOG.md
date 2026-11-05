# Changelog

## 0.5.0 — 2026-11-05
- **Notch tabs**: hover the notch for **Chat** (this conversation's questions and answers, type a follow-up, start a new chat) and **Agents** (every job with its plan, progress, questions you can answer in place, and stop). The edge job cards are gone.
- **Voice shows no text**: while you talk and Pixie answers, nothing is drawn under the notch; it only widens. Click-to-talk stops from the right ear.
- **Live agents**: a second kind of agent that does the task on your real screen (clicks, types, keys, menus, drags, copies) while explaining each step out loud; esc stops it. Background agents keep working on their own.
- **Clipboard**: `clipboard` copies results for you or reads what you copied; `look` lets live agents see the screen.
- **Buddy modes**: always on, idle (only appears while Pixie listens, thinks, talks, points or works) or off.
- **Settings, redone**: grouped sidebar, search (⌘F) that jumps to and flashes the setting, pixel switches, a Models page with provider tabs and a filterable live model list (newest first, context size, price per million tokens, V/A picks for voice and agents).
- **Prices pulled automatically** from the public model price list (Anthropic, OpenAI, DeepSeek), refreshed daily; Usage costs need no typing.
- **Fix**: jobs no longer fail with "tool_use ids were found without tool_result blocks"; tool calls are always answered and history is repaired before every request, so a stuck job continues when you message it.
- Board diagrams draw again (their stroke animation was lost in 0.4.1); labels wrap and shrink to fit inside their boxes, notes and arrow labels move off anything they'd cover, the view fits the drawing, and a click anywhere puts the board away (no ✕).
- **Precise pointing**: highlight and draw take a `target` (an element id or the exact on-screen text) and land exactly on it; coordinates that hit nothing listed fall back to the label, and requests that match nothing on screen are refused instead of pointing at empty space.
- **Onboarding**: your name (skippable), a live mic test with a level meter and the words you said, the natural voice downloading in the background from the first screen with a real progress bar, and a tour of the shortcuts.
- **Shortcuts you can change**: talk and dictate chords (presets including fn, or record your own) in Settings → General; the notch's hints follow them.
- **Chats history**: every conversation is saved; the Jobs window toggles between Jobs and Chats (search, copy, delete), and the notch's Chat tab has a History link.
- **Sounds page**: every sound with what it's for, play, its own volume, on/off, and your own file (or back to default); overall volume.
- **Models**: official names next to alias ids (DeepSeek's `deepseek-flash` is V4.1-Flash), context windows from the official lists, an In use switch, and one **fallback** model from any provider that answers when the main one fails.
- **Usage**: spent-so-far and tokens-so-far totals, ranges from today to all time, filters by provider and voice/agents; alias models are priced through their official names.
- About lists the developer (s41r4j on GitHub and X) and project links.
- The natural voice runtime is now the universal build, so it installs on Intel Macs too.

## 0.4.1 — 2026-11-05
- Voice turns stay in the notch's ears (listening, thinking, speaking) and never drop down or show text.
- The buddy is a round pixel `:)` badge, not a mouse arrow; it draws marks itself like a pencil and drags highlights out as a marching-ants selection.
- Settings → **Models**: Claude, OpenAI and DeepSeek with live model lists for voice and jobs; OpenAI provider (Chat Completions).
- Settings → **Usage**: tokens and estimated cost per model and per day, editable prices.
- Settings → **Coding**: default agent, per-agent model, Codex sandbox, Claude Code permissions, default folder.
- Menu-bar icon is the pixel `:)` again (the pack's template was a microphone).

## 0.4.0 — 2026-11-05
- **Pixie**: openclicky is now Pixie, in the monochrome pixel design system (IBM Plex Mono, VT323, Press Start 2P; dithering, hard shadows, chamfered corners).
- **Buddy pointer**: rides beside the cursor, flies on an arc to what it's talking about, shows what it's saying.
- **Clicks for you**: `mac_step` click / double_click / right_click / drag through the buddy, with the screen state returned after each step; "pointer only" setting.
- **Coding with local agents**: new `code` tool for jobs runs Codex CLI, Claude Code or opencode non-interactively.
- **Drag-me permission guide**: a card with a native draggable app tile for the Accessibility and Screen Recording lists.
- General settings tab; site rebranded.

## 0.3.0 — 2026-11-05
- **Board**: visual explanations are sketched on a clean paper canvas (`draw` with `board: true`) instead of over the screen; the notch shrinks to one line while marks or the board are up.
- **Hover to peek**: resting the pointer on the notch opens quick actions, running jobs and the last answer (native hover, no click needed).
- **Precise pointing**: screenshots are 1:1 with screen points, the overlay uses the window's real frame, and targets snap to the nearest detected control or text line.
- **Smoother**: WebKit background throttling disabled, so animations and timers never wait for a click; marks persist through the answer and fade on their own.
- **Neutral design**: graphite, white and one blue accent with the system font.
- **Permissions that stick**: builds are signed with a stable designated requirement; Settings can reset a stale grant.
- Website (`index.html`, GitHub Pages).

## 0.2.0 — 2026-11-05
- Voice loop: hold ⌃⌥, talk, hear the answer while it streams; DeepSeek or Claude.
- Natural offline voice (Kokoro via sherpa-onnx).
- Jobs with plan boxes, live `note.md`, questions, report blocks and edge cards.
- Browser extension + native host; MCP connectors; skills.
- Dictation with fn⌃; design system v2 assets; single instance.

## 0.1.0 — 2026-11-04
- Menu-bar app, notch panel, hold-to-talk hotkey, permissions wizard, resource slots and sound bus.
