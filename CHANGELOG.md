# Changelog

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
