# Changelog

## 0.8.1 · unreleased
Pixie's name everywhere.

- **Clicks land on what Pixie meant.** Asked to message someone in WhatsApp, Pixie opened other chats: a click was moved to the nearest control even when that wasn't under the point, a contact scrolled out of the list was clicked where it used to be (below the screen), and Pixie reported the label it expected rather than what it hit. Now a click by control id goes to that exact control as it is right now (scrolled into view first when it's out of its list), and is refused when something covers it or it has changed; a click at a point stays at that point; every click says what it actually landed on; and controls out of view are marked as such for the model.
- **The Mac's own Dictation stays the Mac's.** With a Pixie shortcut of fn on its own, pressing fn twice for the Mac's Dictation also woke Pixie each time. A shortcut of fn alone now only counts once you've held fn for a moment (0.3 s): quick presses (fn twice, the emoji or input-source press) are left to macOS, with no notch and no microphone. Shortcuts like fn+ctrl and ctrl+option work as before.
- **Pixie asks you before anything risky.** Before it presses Delete, Send, Pay, Move to Trash, Empty Trash, Don't Save and the like, sends a message with Return in a messaging app, presses cmd+delete, or types a command such as `rm -rf`, `DROP TABLE` or `git push --force`, a system dialog asks you: **Allow** or **Don't** (the default). No answer in a minute counts as Don't. The model can't skip or answer it, in voice turns and in live agents alike.
- **Pixie on every system.** The program is now `pixie` (`pixie.exe` on Windows, `/usr/bin/pixie` on Linux) and the app ID is `pixie.s41r4j.in`, with the helper, the voice and the browser bridge named to match. On the first launch your data folder (settings, keys, memory, chats, canvases, jobs and the natural voice) moves to the new name by itself, in an instant; a link stays at the old place so nothing saved before is lost, and an older version put back by a rollback still finds it.
- **Mac: permissions once more.** macOS keeps permissions per app ID, so Accessibility, Screen Recording, Microphone, Speech Recognition and access to apps like Mail or Calendar are asked for again once. Pixie says so at launch; Settings → Permissions has every switch.
- **Chrome extension:** press Reload on Pixie's extension in `chrome://extensions` once. The old browser registration is removed.
- **Credits where they're due.** Settings → About → **Built with** thanks the open-source projects Pixie stands on (Excalidraw for the canvas, Mermaid, three.js, React, Tauri, sherpa-onnx and the Kokoro voice, and the fonts), and **Open-source licences** opens the licence of every package Pixie ships with, as their licences ask. The repo now has its `LICENSE` (MIT) and `THIRD_PARTY_NOTICES.md`.
- **A new README** with screenshots, getting started, things to try, privacy and credits; build details moved to `docs/DEVELOPMENT.md`.
- **Pixie's own speech to text: words appear as you speak.** Dictation used the system's recogniser and put the text in when you stopped. Pixie now has its own engine, the same on Mac, Windows and Linux: a streaming model that runs on your computer (sherpa-onnx; English; about 73 MB, downloaded once in Settings → Voice) and types each word into the field as you say it, changing only the end that changed. Punctuation and capitals are tidied when you stop. It's separate from the Mac's own Dictation. Once downloaded it's used for dictation (and for talking to Pixie on Linux, which had no voice input at all); Settings → Voice chooses between it and the system recogniser, and "Type as I speak" turns the live typing off.
- **The canvas lays things out for you.** Ask for an org chart, a mind map, a timeline, an architecture in layers, a cycle, pros and cons, a 2×2, a bar, line, pie or scatter chart from real numbers, a table, sticky notes, a code block or an app wireframe, and Pixie places every box itself (before, the model had to work out each position, and mostly drew flowcharts). Everything is still ordinary shapes you can edit. Pixie can now change a drawing instead of redrawing it (move, resize, recolour, relabel, remove; arrows follow), and while it explains, the canvas glides to the part each sentence is about. Arrows that point straight down no longer come out slanted.
- **3D explanations that look and move like the real thing.** New shapes: tubes through points (vessels, wires, DNA), profiles spun round (bottles, bells), outlines pushed up (floor plans, wings), gears, pyramids, domes and discs. Objects can be parts of other objects, be tilted, be glass, metal or glowing, and be repeated hundreds of times (atoms, cells) in one draw call. They spin, orbit, pulse and bob, and dots flow along arrows. Soft light and shadows. **Explode** pulls the parts apart, **Cut** slices the model open, and Pixie can do both while it talks. It stays smooth: a frame is drawn only when something changed, ambient motion runs at 30 frames a second, nothing is drawn while the window is hidden, and quality steps down by itself on a slow computer.
- **Updating keeps everything.** Checked on a copy of a real 0.8.0 data folder: settings, chats, memory, canvases, keys, connectors and voices all carry over to the new app ID (694 files; the only ones that changed are the browser-extension files and the refreshed price list).
- **Lighter on the battery.** The buddy's window redrew every frame all day, even with the pointer still: it now rests once the buddy has caught up and wakes when the pointer moves. Pointer positions, the microphone level and spoken-word marks used to be sent to every Pixie window; each now goes only to the window that draws it. The shortcut listener halves its rate after three seconds with no key held and the pointer still (the first key or movement brings it straight back), and Settings stops refreshing while it's hidden.
- **Quicker and smoother.** The screenshot, the text recognition and the reading of controls now run side by side instead of one after another. The listening waveform moves its own bars instead of redrawing the whole notch 30 times a second. The notch, buddy and shelf no longer load Settings (their code is about a quarter smaller), and release builds are optimised across the whole program.
- **A log that explains a slow answer.** Each look at the screen logs how long its parts took, and each answer logs when the model was asked, when each tool started and ended, and when it finished.
- **Housekeeping.** The log starts a new file when it's full while Pixie runs (before, only at launch). Quitting Pixie no longer logs "helper exited" as a problem. Google calls reuse one connection.
- **Report a problem on Windows** now removes your user folder from the draft, like on the Mac.

## 0.8.0 · unreleased
Pixie looks after itself, works across several screens, draws while it explains (in 3D too), costs less to run, can run with no API key, and dictation can type as you speak. Everything from 0.7.8 to 0.7.17 ships here (those versions were only built locally).

**Self-healing**
- **Watchdogs.** Every 30 s Pixie checks that the helper, the natural voice and each connector still answer, and restarts one that hangs, not only one that quits. Three restarts of one part in five minutes and it's left alone (the voice falls back to the Mac's, a connector is paused until you press Retry) and you're told once. No restart loops.
- **Stuck states.** A notch left on Thinking after the answer ended, or on Listening after the keys were let go, goes back to rest; a stuck shortcut listener is replaced.
- **A permission turned off while Pixie runs** is noticed when something needs it: Pixie says which one, opens its page in System Settings, and says so when it's back on (restarting its helper for Screen Recording).
- **Retired models** are replaced by the newest of the same family from the provider's live list ("claude-sonnet-4-5" becomes the newest Sonnet), and you're told. Only when there's no family match does it go back to the default.
- **Rolling back a bad update (Mac).** Before an update replaces Pixie, the running version is kept (about 60 MB). If the new version closes within a minute of starting three times in a row, Pixie puts the previous one back, opens it, and skips that version until the next release.
- **Settings and memory can't be lost to a damaged file.** Every save keeps a last-good copy; a file that won't read is set aside and the copy put back, and Pixie tells you.
- **A log file** at `<support folder>/logs/pixie.log` (Settings → About → Show log), with millisecond times. **Crash notes**, local only: the next launch says so once and keeps it for a report. Nothing is sent anywhere.
- **A "to fix" list** (`issues.md` in the support folder): problems Pixie worked around (a model error, the helper restarting, a damaged file, a crash), counted when they repeat.
- **"Pixie, check yourself"** (or Settings → About → Run a checkup): the helper, permissions, one tiny model call, the voice, connectors, disk space and today's problems. A helper that doesn't answer is restarted on the spot.
- **"Report this"** (or Settings → About → Report a problem): opens a GitHub issue in the browser you name, already filled in with what happened, your version and the relevant log lines, with personal paths, keys and email addresses removed. You read it and press Submit; Pixie never posts.
- **Quit ends every Pixie process.** Each command or coding agent a job starts runs in its own process group: Quit ends them, and if Pixie is force-quit or crashes, its helper ends them the moment it notices. Stopping a job ends everything its command started, not only the top process. (The "Allow in the Background" entry in System Settings is macOS noting a menu-bar app; Pixie installs no login item.)

**Explanations**
- **Drawing in step with the voice.** Pixie writes its pointing and marks inside its sentences, and each one appears the moment that sentence starts playing: "This is the Apple menu" points at the Apple menu as you hear it. Works for pointing, circles, boxes, underlines, ticks, crosses and arrows between two things, in answers and in live agents' spoken steps. With a typed answer, marks show as the text comes in.
- **Fuller explanations** when you ask Pixie to explain, teach or walk you through something: 4 to 8 short sentences that build on each other, most with a mark on screen. Quick questions stay short.
- **3D explanations** (Settings → Experimental → 3D explanations): for things easier to grasp in 3D (the heart, a molecule, an engine, the planets) Pixie builds a model of labelled shapes and arrows in its own window. Spin, zoom and move it; as Pixie explains, the view turns to each part and lights it up as it's named. The window only draws while something moves, and closing it frees its memory.
- **Marks land where they should.** Menu bar icons (Wi-Fi, battery, the clock, other apps' status items) are in the list of controls with their exact frames (and can be pressed); before, marks on them came out about a quarter too far left. Numbers in a mark are read as screenshot pixels and converted, and Pixie is told never to invent a position.

**Computer use over several steps**
- Pixie can **look** at the screen again during a voice turn, not only at the start.
- **Every action reports the screen after it** (click, press, type, key, menu, open, scroll): the front app and window, the focused field, the 30 controls nearest where it acted with ids and frames, the ids that went away and the text that's new.
- **Control ids stay the same for the whole request**, so "e12" is the same button after a click; ids that disappear are refused with a clear message. **Clicks snap to the newest screen**, never to the one from before the last action.
- A voice turn can take up to 12 steps (was 8); longer jobs still go to a live agent.

**Several screens and desktops**
- **Notch on** (Settings → General → Screens, shown with more than one screen): the screen you're using, the built-in screen, or a screen by name. A screen without a camera notch gets the same bar at the top centre. Or say "keep the notch on the LG".
- Pixie knows which screens there are and where they sit. It looks only at the pointer's screen unless you ask about another ("what's on my other screen?", "both screens"), then takes one screenshot per screen. With one screen nothing changes.
- **A window across two screens** keeps all its controls, and **live agents stay on their app's screen** even when you move the mouse elsewhere. Windows and Linux capture the screen Pixie was asked about.
- **Desktops (Spaces):** ask about "my other desktop" and Pixie knows which of the front app's windows are on another desktop or minimized; it can switch desktops when you ask.

**Voice and dictation**
- **No more skipped words.** The natural voice used to drop any word missing from its word list ("opened", "GitHub", every number and price). Pixie now works out the sounds itself: numbers, times, prices, percentages, years, units and symbols become words first ("$12.50" is "twelve dollars fifty cents"), then each word is looked up, with endings, compounds, acronyms and known pieces.
- **Louder and clearer.** Speech is leveled to normal speech loudness (about 8 dB louder) with a limiter so peaks never crack. Settings → Voice → Loudness: Quiet, Normal or Loud; "speak louder" works by voice.
- **Long questions are heard in full.** Apple's on-device recognition starts over after a pause, and Pixie kept only the last sentence; every piece is kept now.
- **Dictate hands-free:** Settings → General → "Press to start, press to stop" makes fn+ctrl a toggle. Or say "make dictation a toggle".
- **See what Pixie hears.** While you dictate, the notch shows the words so far.
- **Type as you speak** (Settings → Experimental, off unless you turn it on): words appear as you talk wherever there's a text cursor, through Accessibility in most Mac apps and by key presses (the real keys for your layout) in terminals like Warp. When you let go, the tidied text (punctuation, no "um") replaces what you said. With no text cursor (the desktop, a list), the words go on the clipboard and the notch says so. Off: dictation types the text when you stop, as before.

**Faster and smoother**
- **The microphone comes on sooner:** 58 ms from the talk keys instead of about 190 (143 ms instead of 276 for the first press after launch).
- **The notch follows your pointer at once** (opens after 40 ms, closes after 120 ms) and **slides out of the camera housing and back in**, staying black; its content fades in after it opens and out before it closes.
- **The notch goes away right after the answer:** 0.8 s after the last word (1.5 s if nothing was spoken).
- The 3D view no longer lags (labels trailed the view and stuttered).

**Answer the way you asked**
- **Typed questions get typed answers** (Settings → General, off by default): talk and Pixie talks back; type and it writes back, without a word out loud.
- **Show typed answers in the notch** (on by default): the notch drops down with the whole answer, stays while you hover and closes with Esc. Off: typed answers wait in the notch's Chat tab.

**Context and cost**
- **Honest usage.** Settings → Usage counts tokens read from the provider's prompt cache and prices them at their own rates; before, cached tokens weren't counted at all.
- **Cache-friendly prompts.** The system prompt holds only what stays the same; the time, the front app and the screen go with your newest message. On Claude, the tools, system prompt and earlier conversation are marked for caching.
- **Smaller screen text:** the 30 controls and 40 text lines that match your words or sit nearest the pointer (all of it when you ask to read or summarize); questions clearly not about the screen skip it; screenshots go out 1280 px wide. Tool descriptions are about a fifth smaller, and with more than 20 connector tools only the services your request names are offered.
- **Long jobs stay lean:** one tool result is capped at about 20,000 characters, and older results are folded into one-line stubs from step 15.
- Measured on DeepSeek Flash with the same 10 voice questions: uncached input per step halved (2,772 to 1,374 tokens) and cost per step down 30%, same answers.

**Models**
- **No API key needed: opencode as Pixie's model.** Settings → Models → **On this computer**: if opencode is installed, pick its models for talking and for agents. Free models are marked **Free** and cost $0; models you've signed in to in opencode work too. Pixie starts opencode only when needed, with its own settings (yours are untouched), and stops it after ten quiet minutes. Or say "use opencode".
- **Switching provider keeps the right models.** Model names you say are matched against the provider's real list, a switch drops choices the new provider doesn't have, and if a provider says a model doesn't exist, Pixie goes back to its default and tells you.
- Settings → Models warns when the talk or agent model can't see screenshots, and suggests one that can.

**Jobs and coding**
- **Pixie knows its jobs:** every question comes with a short list of your recent jobs, and Pixie can look up any job's plan, progress, report and note ("how's my job going?").
- **Coding goes to the agent with usage left:** your default if it has usage, else Codex, then Claude Code, then opencode's free models. Codex's limits are read from its own logs; for Claude Code, set what's left in Settings → Coding.
- **Jobs you ask for get started**, also the second time in a day, and deleting a job removes it from the Jobs window at once.

**Personalities and connectors**
- Switching personality is reliable: "talk like the nerdy one", "go back to normal Pixie" and close mishearings switch at once, and the face always matches.
- **Connectors moved behind a switch** in Settings → Experimental: off, the page is hidden, no connector servers run and their tools aren't offered. If you'd already added connectors, it starts on. The Chrome extension moved to Settings → Apps.

**Fixes**
- Pixie knows its own version, so "what version are you?" can't get a made-up answer.
- No more "(Tools used for this answer…)" after answers; old answers are cleaned up.
- When a check can't fetch the update file, Pixie tries again in 15 minutes instead of six hours.

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
- **A new look**: the monochrome pixel design system (IBM Plex Mono, VT323, Press Start 2P; dithering, hard shadows, chamfered corners).
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
