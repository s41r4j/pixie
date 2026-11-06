# Changelog

## 0.7.7 · 2026-11-07
The first release that updates itself, and the first with Windows and Linux builds. Everything from 0.7.0 to 0.7.7 ships here (those versions were only built locally).

**New platforms**
- **Windows**: the Mac helper's work has a Windows twin built in: voice input through Windows speech recognition (with the mic level), Windows voices for answers, screenshots, Windows OCR, the screen's buttons through UI Automation, and clicking, typing, keys, scrolling and drag. Shortcuts read Ctrl / Alt / Win, and dictation is Ctrl+Win (Windows can't see fn). Permissions links straight to the Microphone and Speech privacy pages. Agents use Git Bash when it's installed and PowerShell otherwise, and connectors start properly (`npx.cmd`).
- **Linux preview**: an AppImage and a .deb. Chat, agents, screenshots (grim, scrot…), tesseract OCR, xdotool clicks and espeak-ng speech; no voice input yet.
- **CHECKLIST.md** ticks every feature and setting for Mac, Windows and Linux, and CI checks all three on every push.

**Updates itself**
- Pixie checks GitHub on launch and every 6 hours, downloads a new version quietly, checks its signature, and switches over when you're not using it (nothing running, the notch idle, no window open, 10 quiet minutes). Settings → About has Check now / Restart now; the menu bar menu has Check for Updates / Restart to Update. You can turn it off. This is the last version you download by hand.

**Canvas**
- Diagrams and explanations land on a real Excalidraw canvas in its own window instead of a sketch floating over the screen (the old board is gone; on-screen marks stay). Shapes, arrows and text, or a Mermaid flowchart laid out for you. Move and edit anything, rename it, save it as .excalidraw, PNG or SVG (to Downloads, never overwriting).
- **Draws on its own** when a picture explains it better: Settings → General → **Drawing**: Only when I ask, When it helps (default) or Often; or say "draw more".
- The window comes up in front of you, also over a full-screen app, and hides when you click outside it. **Show Canvas** in the menu bar menu brings it back. Pixie can open, list, read back, clear and delete canvases when you ask; Delete in the canvas sends one to the Trash.

**Memory**
- **One chat all day**: today's conversation carries on after quiet gaps and restarts until you press New chat (or say "new chat") or the day changes (midnight by default; pick the hour in Settings → Memory). Older turns are folded into a short running summary so long days stay quick.
- **Long-term notes**: say "remember that…" or "forget…", or let Pixie learn lasting facts from a chat once it's gone quiet. The most useful notes go with every question. Notes stay on your computer (`memory.json`); passwords, keys and card numbers are refused.
- **Settings → Memory**: Automatic / Only when I ask / Off, the hour a new chat starts, and every note to add, edit, search or forget.

**Pixie knows itself**
- Ask "what can you do?", "what's my talk shortcut?", "which model are you on?" and Pixie answers from how it's actually set up. Say "talk slower", "use the Emma voice", "stop speaking answers", "call me Sam", "don't click for me", "switch to Claude" or "turn off the Slack connector" and it changes the setting at once, several at a time; an open Settings window updates to match. API keys, permissions and sign-ins stay with you on the page.
- "Quit" / "exit" / "close Pixie" quits; "stop working" stops every job and live agent; "never mind" stops talking; "new chat" starts fresh. These short ones work instantly, without the model.

**Four personalities**
- Settings → General → **Personality**: **Pix** `:)` (default, Pixie as it always was), **Inos** `8)` intelligent and nerdy, **Xtrem** `XD` chaotic and witty with a **Spicy** switch, **Eon** `;)` calm and concise. Each has its own pixel face in the notch, on the buddy and in Settings. Only the style of replies changes: same voice, same tools, same care. "Switch to Eon" works instantly.

**Connectors**
- GitHub, Slack and Brave Search moved off retired npm packages to GitHub's own hosted server, the maintained `slack-mcp-server` (a community server; posting stays off) and Brave's own server. Linear and Jira & Confluence use their current endpoints. Connectors you added before move over on their own, keeping their tokens. New: **Sentry** and **Context7** (library docs, no key).
- The page has a summary, sections by purpose, how each one signs in, a clear status, the reason in plain words when one can't connect (with Retry and the server's own message), the tools each gives Pixie, and **Add your own server**.

**Smaller things**
- **Lighter**: idle Pixie went from about 730 MB to about 215 MB. The natural voice loads when you start talking (about a second) and unloads after 5 quiet minutes; Settings and the canvas are freed when closed.
- **Typing into the right field**: Pixie clicks into the field first and is told where the text went, so split windows no longer get it in the wrong pane.
- No em dashes in the website, docs or Pixie's own texts. The menu bar menu groups Type a Question, Show Jobs and Show Canvas. The Download page's Windows link no longer turns into a grey box. "Check for updates" says you're up to date when the newest release has no update file.

## 0.6.0 · 2026-11-05
- **Apps on your Mac** (Settings → Apps): Calendar, Reminders and Contacts through macOS's own frameworks, and Apple Mail through its scripting. Each is off until you switch it on, which asks macOS once. "What's on tomorrow?", "remind me to call Sam at 5", "what's Priya's email?", "draft a reply to the last mail from Alex". Mail drafts open in Mail for you to send; Pixie never sends.
- **Google account**: sign in with your own Google OAuth client and Pixie reads and searches Gmail, saves drafts (never sends), and reads or adds Google Calendar events, straight from Google's APIs. Tokens stay on your Mac.
- **Connectors gallery** (Settings → Connectors): GitHub, Notion, Slack, Linear, Jira & Confluence, Brave Search and a folder of your choice in one click, each with an on/off switch, live tool counts and errors. Changes apply right away, no restart; anything else still goes in `mcp.json`.
- **Hover to read**: while Pixie thinks or talks, hover the notch and it drops down to show the question, the answer word by word and the tools it used; move away and it folds back up.
- **Dock**: clicking Pixie in the Dock, Launchpad or Finder after closing its windows opens Settings again, and Pixie shows in the Dock and ⌘-Tab while Settings or a note is open.
- **Windows build (preview)** and a new build pipeline: every push builds a universal macOS `.dmg` and a Windows `-setup.exe`; tags publish both in a GitHub release. On Windows, chat, agents, models, usage and settings work; voice, screen reading and pointing are Mac-only for now.

## 0.5.0 · 2026-11-05
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

## 0.4.1 · 2026-11-05
- Voice turns stay in the notch's ears (listening, thinking, speaking) and never drop down or show text.
- The buddy is a round pixel `:)` badge, not a mouse arrow; it draws marks itself like a pencil and drags highlights out as a marching-ants selection.
- Settings → **Models**: Claude, OpenAI and DeepSeek with live model lists for voice and jobs; OpenAI provider (Chat Completions).
- Settings → **Usage**: tokens and estimated cost per model and per day, editable prices.
- Settings → **Coding**: default agent, per-agent model, Codex sandbox, Claude Code permissions, default folder.
- Menu-bar icon is the pixel `:)` again (the pack's template was a microphone).

## 0.4.0 · 2026-11-05
- **Pixie**: openclicky is now Pixie, in the monochrome pixel design system (IBM Plex Mono, VT323, Press Start 2P; dithering, hard shadows, chamfered corners).
- **Buddy pointer**: rides beside the cursor, flies on an arc to what it's talking about, shows what it's saying.
- **Clicks for you**: `mac_step` click / double_click / right_click / drag through the buddy, with the screen state returned after each step; "pointer only" setting.
- **Coding with local agents**: new `code` tool for jobs runs Codex CLI, Claude Code or opencode non-interactively.
- **Drag-me permission guide**: a card with a native draggable app tile for the Accessibility and Screen Recording lists.
- General settings tab; site rebranded.

## 0.3.0 · 2026-11-05
- **Board**: visual explanations are sketched on a clean paper canvas (`draw` with `board: true`) instead of over the screen; the notch shrinks to one line while marks or the board are up.
- **Hover to peek**: resting the pointer on the notch opens quick actions, running jobs and the last answer (native hover, no click needed).
- **Precise pointing**: screenshots are 1:1 with screen points, the overlay uses the window's real frame, and targets snap to the nearest detected control or text line.
- **Smoother**: WebKit background throttling disabled, so animations and timers never wait for a click; marks persist through the answer and fade on their own.
- **Neutral design**: graphite, white and one blue accent with the system font.
- **Permissions that stick**: builds are signed with a stable designated requirement; Settings can reset a stale grant.
- Website (`index.html`, GitHub Pages).

## 0.2.0 · 2026-11-05
- Voice loop: hold ⌃⌥, talk, hear the answer while it streams; DeepSeek or Claude.
- Natural offline voice (Kokoro via sherpa-onnx).
- Jobs with plan boxes, live `note.md`, questions, report blocks and edge cards.
- Browser extension + native host; MCP connectors; skills.
- Dictation with fn⌃; design system v2 assets; single instance.

## 0.1.0 · 2026-11-04
- Menu-bar app, notch panel, hold-to-talk hotkey, permissions wizard, resource slots and sound bus.
