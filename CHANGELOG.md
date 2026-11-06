# Changelog

## 0.7.7 · unreleased
- The menu bar menu has a separator above Settings, so Type a Question, Show Jobs and Show Canvas sit in their own group.

## 0.7.6 · unreleased
- **The canvas is standard**: the Excalidraw canvas is no longer an experiment, and the old board (the sketch that floated over the screen) is gone. On-screen marks (circles, boxes, underlines, arrows) stay.
- **Draws on its own**: when a picture would explain it better (how things connect, a flow, steps, a comparison, a timeline), Pixie draws it on the canvas without being asked. Settings → General → **Drawing**: Only when I ask, When it helps (the default) or Often; or say "draw more" / "only draw when I ask".
- **Four personalities**: Settings → General → **Personality**. **Pix** `:)` is Pixie as it always was (nothing added). **Inos** `8)` is intelligent, nerdy and curious. **Xtrem** `XD` is chaotic, witty and mischievous (jokes about the situation, never about you), with a **Spicy** switch for a little swearing. **Eon** `;)` is calm, concise and precise. Each has its own pixel face in the notch, on the buddy and in Settings. They change only the style of replies: same voice, same tools, same care before sending or deleting, and documents that agents write keep the plain style. Switch by asking: "be Eon", "switch to Xtrem", "turn on spicy"; "switch to …" and "be …" work instantly, without asking the model.
- Show Canvas is always in the menu bar menu. Settings → Experimental is empty for now.

## 0.7.5 · unreleased
- **One chat all day**: today's conversation carries on after quiet gaps and after Pixie restarts, until you press New chat (or say "new chat" / "start over") or the day changes. The new day starts at midnight by default; pick another hour in Settings → Memory. After a long gap Pixie knows how long it's been, so an old "this" isn't mixed up with what's on screen now. Older turns are folded into a short running summary in the background, so long days stay fast and cheap.
- **Long-term memory**: Pixie keeps short notes about you across days: who you are, how you like things, your people, projects and setup. Say "remember that…" or "forget…", or let it learn from a chat once the chat has gone quiet (Automatic). The most useful notes go with every question, and Pixie looks up the rest when they matter. Notes stay on your computer (`memory.json`); passwords, keys and card numbers are refused. The notes jobs used to keep in `memory.md` come over on their own.
- **Connectors, redone**: GitHub, Slack and Brave Search used npm packages that have been retired and are no longer maintained; they now run on GitHub's own hosted server, the maintained `slack-mcp-server` (marked as a community server; posting stays off) and Brave's own server. Linear and Jira & Confluence use their current endpoints. Connectors you added before move over on their own, keeping their tokens. New: **Sentry** and **Context7** (up-to-date library docs, no key). The page now has a summary (connected, tools, needing attention, Node.js), sections by purpose, how each one signs in, a clear status, the reason in plain words when one can't connect (with Retry and the server's own message), the list of tools each one gives Pixie, and **Add your own server** instead of editing `mcp.json` by hand. Connectors also start properly on Windows (`npx.cmd`).
- **No em dashes**: the website, the docs and Pixie's own texts don't use them any more, and Pixie is told not to write them.
- **Settings → Memory**: Automatic / Only when I ask / Off, the hour a new chat starts, and every note: add, edit (click it), forget, search, or forget everything. Pixie can switch these by voice too ("turn memory off", "start a new chat at 4am").

## 0.7.4 · unreleased
- **Pixie knows itself and sets itself up**: ask "what can you do?", "what's my talk shortcut?", "which model are you using?" and Pixie answers from how it's actually set up right now (version, shortcuts, voice, model, permissions, apps, connectors, experiments, updates, usage). Say "talk slower", "use the Emma voice", "stop speaking answers", "turn on the canvas", "call me Sam", "don't click for me", "switch to Claude" or "turn off the Slack connector" and it changes the setting instantly, several at once, without clicking through Settings; an open Settings window updates to match. It can also open any Settings page, check for updates and restart into one. API keys, permissions and sign-ins stay with you on the page.
- **Quit and stop by asking**: say or type "quit", "exit", "close Pixie" or "turn yourself off" and Pixie says bye and quits; "stop working" / "stop everything" stops every running job and live agent; "never mind" / "stop" just stops. The short ones are handled instantly without asking the model; anything phrased differently ("I'm done with you for today") goes through the pixie tool.
- "Check for updates" no longer says it couldn't reach GitHub when the newest release simply has no update file yet; it says Pixie is up to date.

## 0.7.3 · unreleased
- **Show Canvas in the menu bar menu**: right under Type a Question… and Show Jobs while the Excalidraw canvas is switched on (Settings → Experimental). It brings back the canvas you were using, or your last one.

## 0.7.2 · unreleased
- **Lighter on memory**: the natural voice (Kokoro, ~470 MB loaded) no longer sits in memory all day. It loads in about a second when you start talking or typing to Pixie, so answers aren't slower, and unloads after 5 quiet minutes. Settings is created when you open it and freed when you close it, and a canvas you've put away is freed after 2 minutes. Idle, Pixie went from about 730 MB to about 215 MB.
- Repo tidy-up: TODO.md became `docs/ROADMAP.md`, the architecture notes describe 0.7 (Windows/Linux layer, Kokoro, updates, canvas), and an unused placeholder crate is gone.
- The website's release file list shows tiny files as "<1 KB" instead of "0 KB".

## 0.7.1 · unreleased
- **Canvas opens where you are**: Pixie opens the canvas itself when you ask ("open the canvas", "open my git canvas"), and the window comes up in front of you with focus, on the current desktop and over a full-screen app. Before, it could open behind other windows where you never saw it. Settings → Experimental → Open a canvas brings back your last one.
- **Click outside to put it away**: clicking another app or the desktop hides the canvas (saved first); asking Pixie, or Open a canvas, brings it back.
- **Delete canvases**: a Delete button in the canvas (click twice), and Pixie can delete one, or all, when you ask. The files go to the Trash. Pixie can also list your canvases.
- Exports show a "Show" link instead of opening Finder (which would hide the canvas). Titles are saved with the drawing.
- **Typing into the right field**: Pixie clicks into the field before typing, and it's told which field the text went into, so a split window (two terminal panes, a chat side panel) no longer gets it in the wrong pane.
- **Website**: the Download page's Windows link no longer turns into a big grey box (it shared a style name with the illustration on the home page).

## 0.7.0 · built locally, ships with the next release
- **Windows, for real**: the Mac helper's work now has a Windows twin built in. Voice input through Windows speech recognition (with the mic level), Windows voices for answers, screenshots, Windows OCR, the screen's buttons through UI Automation, and clicking, typing, keys, scrolling and drag for live agents. Shortcuts read Ctrl / Alt / Win, and dictation is Ctrl+Win (Windows can't see fn). Permissions links straight to the Microphone and Speech privacy pages.
- **Agents on Windows**: the shell tool runs Git Bash when it's installed and PowerShell otherwise; searching files, converting images, copying folders and opening things no longer call Mac programs. Windows credential folders join the list of paths Pixie won't open.
- **Linux preview**: an AppImage and a .deb from the same pipeline. Chat, agents, screenshots (grim, scrot…), tesseract OCR, xdotool clicks and espeak-ng speech; no voice input yet. The Chrome bridge registers for Linux browsers too.
- **CHECKLIST.md**: every feature and setting, ticked for Mac, Windows and Linux.
- CI checks Windows and Linux on every push as well.
- **Updates itself**: Pixie checks GitHub on launch and every 6 hours, downloads a new version quietly, checks its signature, and switches over when you're not using it (nothing running, the notch idle, no window open, 10 quiet minutes). Settings → About shows it and has Check now / Restart now; the tray menu has Check for Updates / Restart to Update; quitting installs a waiting Windows update. You can turn it off. This is the last version you download by hand.
- **Settings → Experimental**: early features with an on/off switch each. Tools for features that are off aren't offered to the model.
- **Excalidraw canvas (experimental)**: diagrams become real Excalidraw drawings in a canvas window instead of a slate in the air. Pixie draws shapes, arrows and text (or a Mermaid flowchart, sequence or class diagram) one by one; you can move and edit everything, rename it, and save it as .excalidraw, PNG or SVG (to Downloads, never overwriting). Canvases are kept on your computer and listed in the canvas menu; Pixie can read the canvas back, including your edits,. Works offline.

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
