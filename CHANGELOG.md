# Changelog

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
