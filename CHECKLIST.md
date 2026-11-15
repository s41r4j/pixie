# Pixie on Mac, Windows and Linux

What works where. The Mac is the main platform; Windows and Linux are previews.

| Mark | Meaning |
|---|---|
| ✅ | Works and has been used on a real machine |
| 🧪 | Built and compiles for this platform; not yet tried on a real machine |
| 🟡 | Partly: see the note |
| ❌ | Not on this platform yet |
| n/a | Doesn't apply |

On Windows and Linux, the Mac's Swift helper is replaced by native code in
`apps/desktop/src-tauri/src/native/`, which answers the same commands.

## The notch and talking

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Notch companion (panel up top) | ✅ | 🧪 | 🧪 | Windows/Linux: a pill at the top centre (no notch). Wayland may not let apps place windows. |
| Chat tab | ✅ | 🧪 | 🧪 | |
| Agents tab | ✅ | 🧪 | 🧪 | |
| Hover to expand the answer | ✅ | 🧪 | 🧪 | |
| Typed questions | ✅ | 🧪 | 🧪 | |
| Push-to-talk shortcut | ✅ | 🧪 | ❌ | Mac ⌃⌥, Windows Ctrl+Alt. Linux can't read held modifier keys yet. |
| Dictation shortcut | ✅ | 🧪 | ❌ | Mac fn+⌃. Windows has no readable fn key, so it's Ctrl+Win. |
| Dictation as a toggle (press to start, press to stop) | 🧪 | 🧪 | ❌ | 0.8.0. |
| fn alone as a shortcut waits for a hold (0.3 s), so the Mac's fn-fn Dictation doesn't wake Pixie | 🧪 | n/a | n/a | 0.8.1. Unit-tested (double press, hold, toggle stop). Windows and Linux can't see fn. |
| Rests when idle: the buddy stops redrawing on a still pointer, the shortcut poll halves its rate, fast events go to one window | 🧪 | 🧪 | 🧪 | 0.8.1. Built and unit-tested (poll pacing); the frame loop and event routing are shared code. Needs a day on battery to confirm the saving. |
| Type as you speak, Experimental (dictated words appear while talking, then the tidied text) | 🧪 | ❌ | ❌ | 0.8.0. Live wherever there's a text cursor (Accessibility, or real keys for terminals like Warp); no cursor, clipboard. Checked by self-test in TextEdit, Terminal and Warp; not yet confirmed by a real dictation. |
| Long spoken questions kept in full | 🧪 | 🧪 | ❌ | 0.8.0: pieces kept across recognizer restarts. |
| Voice input (speech to text) | ✅ | 🧪 | ❌ | Mac: Apple Speech, on-device. Windows: Windows speech recognition (needs Settings → Privacy → Speech → Online speech recognition). |
| Pixie's own speech to text (streaming, on-device, English): dictation types as you speak; Settings → Voice | 🧪 | 🧪 | 🧪 | 0.8.1. Mac: recognition checked on recorded speech through the app's audio path (words every ~0.3 s, final text 20 ms after stopping), microphone opened and read, model load ~1 s; live typing into a real text field and real spoken dictation not yet tried. Windows and Linux: compiles; never run. Linux needs xdotool to type. |
| Live transcript while talking | ✅ | 🧪 | ❌ | |
| Mic level waveform | ✅ | 🧪 | ❌ | |
| Spoken answers: system voice | ✅ | 🧪 | 🧪 | Windows voices; Linux uses espeak-ng or pico2wave if installed. |
| Spoken answers: natural voice (Kokoro) | ✅ | ❌ | ❌ | The voice runtime ships for macOS only so far. Since 0.8.0 Pixie works out the sounds itself, so words missing from Kokoro's word list (and numbers, times, prices) are spoken, not skipped. |
| Natural voice loudness (Quiet / Normal / Loud) | ✅ | n/a | n/a | Leveled to speech loudness with a limiter (0.8.0). |
| Words light up as Pixie speaks | ✅ | 🧪 | 🧪 | Windows/Linux time words by length. |
| Escape stops Pixie | ✅ | 🧪 | ❌ | |
| Sounds (starts, finishes, nudges) | ✅ | 🧪 | 🧪 | |
| Ask Pixie about itself ("what can you do?", "what's my shortcut?") | ✅ | 🧪 | 🧪 | Answers from its live setup: version, shortcuts, voice, model, permissions, apps, connectors, memory, updates, usage. |
| Change Pixie's settings by asking ("talk slower", "use the Emma voice", "turn on the canvas") | ✅ | 🧪 | 🧪 | Several at once, instantly, no clicking through Settings. Keys, permissions and sign-ins stay with you. |
| "Quit" / "exit" / "close Pixie" | ✅ | 🧪 | 🧪 | Says bye and quits. Any other phrasing goes through the model. |
| "Stop working" / "stop everything" | ✅ | 🧪 | 🧪 | Stops every running job and live agent. "Never mind" / "stop" just stops talking. |
| Personalities (Pix, Inos, Xtrem, Eon) with their own faces | ✅ | 🧪 | 🧪 | Style of replies only; same voice, tools and care. Jobs' documents stay plain. Switching by voice checked on the Mac in 0.8.0 ("talk like the nerdy one", "go back to normal Pixie", inside longer requests). |

## Seeing and doing

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Screenshot for a question | ✅ | 🧪 | 🧪 | Windows: GDI capture. Linux: grim, gnome-screenshot, spectacle, scrot or ImageMagick. |
| Reading text on screen (OCR) | ✅ | 🧪 | 🟡 | Mac Vision, Windows OCR. Linux needs tesseract installed. |
| Knowing the buttons on screen (UI tree) | ✅ | 🧪 | ❌ | Mac Accessibility, Windows UI Automation. Linux AT-SPI not done. |
| Which app is in front | ✅ | 🧪 | 🟡 | Linux needs xdotool (X11). |
| Buddy pointer next to the cursor | ✅ | 🧪 | 🧪 | |
| Pointing at things | ✅ | 🧪 | 🧪 | |
| Drawing marks (circle, box, underline, arrow) | ✅ | 🧪 | 🧪 | |
| Drawing while talking (each mark appears as its sentence is spoken) | ✅ | 🧪 | 🧪 | 0.8.0. Checked on the Mac with DeepSeek Flash: marks 5 to 7 s apart, in step with the voice. |
| Fuller explanations when asked to explain or teach (4 to 8 sentences) | ✅ | 🧪 | 🧪 | 0.8.0. |
| 3D explanations (Experimental): tubes, spun and extruded shapes, gears, parts, copies, materials, motion, Explode, Cut | 🧪 | 🧪 | 🧪 | 0.8.1 rewrite. Checked in the browser preview on this Mac's GPU: two sample scenes about 1 ms a frame (15 to 23 draw calls), exploded and cut views look right. Not yet driven by the model in the app. |
| Menu bar icons (Wi-Fi, battery, clock, status items) in the control list | 🧪 | ❌ | ❌ | 0.8.0. Not yet tried live (the Mac was locked). |
| Self-healing: log file, crash notes, last-good settings and memory, problems list | 🧪 | 🧪 | 🧪 | 0.8.0. Unit-tested; checkup run in the real app. |
| Checkup ("check yourself", Settings → About) | ✅ | 🧪 | 🧪 | 0.8.0. |
| Watchdogs: a hung helper, voice or connector is restarted, three strikes then left alone | 🧪 | 🧪 | 🧪 | 0.8.0. Mac: a frozen helper and a frozen voice server were caught and replaced in the real app. Connectors and the three-strike stop are unit-tested. |
| Stuck notch and shortcut listener reset | 🧪 | 🧪 | 🧪 | 0.8.0. Not yet seen stuck in real use. |
| A permission turned off while running: named, its page opened, noticed when back | 🧪 | ❌ | ❌ | 0.8.0. Unit-tested; not tried by revoking a permission. |
| Retired model replaced by the newest of its family | 🧪 | 🧪 | 🧪 | 0.8.0. Unit-tested. |
| Rolling back an update that keeps closing at launch | 🧪 | ❌ | ❌ | 0.8.0. Mac: the swap, reopen and skipped version checked in the real app with a copy of the same build; not yet through a real update. |
| Faster: mic on 58 ms after the talk keys (143 ms the first time), quicker notch hover and animations | 🧪 | 🧪 | 🧪 | 0.8.0. Mic times measured in the real app with a simulated key press (`--hold-talk-test`); the hover feel needs a real pointer. |
| Notch on a chosen screen (Settings → General → Screens) | 🧪 | 🧪 | 🧪 | 0.8.0. Setting checked in the preview; this Mac has one screen, so the move isn't tried yet. |
| A window across two screens keeps all its controls | 🧪 | ❌ | ❌ | 0.8.0. Needs a second screen to try. |
| Live agents stay on their app's screen | 🧪 | 🧪 | 🧪 | 0.8.0. Needs a second screen to try. |
| Other desktops: the front app's windows elsewhere, when asked | 🧪 | ❌ | ❌ | 0.8.0. Unit-tested. |
| Quit (or a crash) ends agents' commands and coding agents | 🧪 | 🧪 | 🧪 | 0.8.0. Mac: a job's 400 s command ended when Pixie was killed with kill -9. Windows ends them on Quit only. |
| Report a problem (a filled-in GitHub issue you submit) | 🧪 | 🧪 | 🧪 | 0.8.0. Draft and redaction unit-tested; not opened in a browser in testing. |
| New app ID (`pixie.s41r4j.in`) and program name (`pixie`): the old data folder moves over, linked at the old path | 🧪 | 🧪 | 🧪 | 0.8.1. Move, link and "both have data" unit-tested on the Mac; the bundle checked (names, signature). Not yet tried as a real update from 0.8.0. Windows links with a directory junction. |
| Credits and open-source licences (Settings → About; THIRD_PARTY_NOTICES.md in the app) | 🧪 | 🧪 | 🧪 | 0.8.1. Notices generated from the shipped dependencies (`npm run notices`); the file is in the bundle. Opens in TextEdit / Notepad / the default app. |
| Several screens: one line about them, look at another screen | 🧪 | 🧪 | ❌ | 0.8.0. Unit-tested; the dev Mac has one screen. Windows captures the pointer's screen only for now. |
| Excalidraw canvas | ✅ | 🧪 | 🧪 | Editable diagrams in a canvas window; shapes, arrows, text or Mermaid; saves locally; exports .excalidraw, PNG, SVG to Downloads. Pixie can open, list and delete canvases; the window hides on a click outside; Show Canvas in the menu bar menu. Replaced the old board in 0.7.6. |
| Canvas: laid out for you (tree, mind map, timeline, layers, cycle, compare, quadrant, charts, table, notes, code, wireframe), edit by id, glide to a part while speaking | 🧪 | 🧪 | 🧪 | 0.8.1. Layouts unit-tested (no overlaps, arrows join real shapes); every kind and the edit action rendered and checked in the preview. `[[canvas id]]` while speaking not yet tried in the app. |
| Drawing on its own when it helps (Only when I ask / When it helps / Often) | 🧪 | 🧪 | 🧪 | Settings → General → Drawing, or "draw more". |
| Click and type for you | ✅ | 🧪 | 🟡 | Windows SendInput. Linux xdotool, X11 only. |
| Several steps in one request (fresh screen after every action, ids stable for the request, look in voice) | ✅ | 🧪 | 🟡 | 0.8.0. Checked on the Mac with DeepSeek Flash in Finder and System Settings; not yet on Claude. Linux has no control list, so only clicks by position. |
| Exact clicks: an id goes to the live control (scrolled into view, refused if covered or changed); points aren't moved; each click says what it hit | 🧪 | 🧪 | 🧪 | 0.8.1. Unit-tested (out-of-view rows marked, not clicked by name); the helper's live checks type-check but weren't run against real apps here (no Accessibility for the test shell). Windows and Linux: out-of-view refused, no live check yet. |
| Asks before risky actions (Delete, Send, Pay, Return in a chat, cmd+delete, `rm -rf`…): a dialog, Don't by default | 🧪 | 🧪 | 🧪 | 0.8.1. Rules unit-tested. Mac: osascript dialog; Windows: a message box; Linux: zenity or kdialog, and without either it doesn't do it. |
| Press a control by name, open menus | ✅ | 🧪 | ❌ | Needs the UI tree. |
| Keyboard shortcuts (key combos) | ✅ | 🧪 | 🟡 | On Windows ⌘ means Ctrl. Linux via xdotool. |
| Scroll | ✅ | 🧪 | 🟡 | |
| Paste text into the focused field | ✅ | 🧪 | 🟡 | Clipboard + paste, clipboard put back after. |
| Open apps, files and links | ✅ | 🧪 | 🧪 | Windows ShellExecute, Linux xdg-open / gtk-launch. |
| Live agents (do it on your screen) | ✅ | 🧪 | 🟡 | Linux: position clicks only. |
| Hidden from screenshots and screen sharing | ✅ | 🧪 | ❌ | Windows 10 2004+ (WDA_EXCLUDEFROMCAPTURE). Linux has no such API. |
| Above full-screen apps, on every Space/desktop | ✅ | 🟡 | 🟡 | Always on top; not over exclusive full-screen games. |

## Memory

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| One chat all day (carries on after gaps and restarts) | 🧪 | 🧪 | 🧪 | Ends with New chat ("new chat", "start over") or at the day's reset hour (midnight by default). |
| Running summary of older turns | 🧪 | 🧪 | 🧪 | The last 12 turns go in full; older ones as a short summary. |
| Long-term notes ("remember that…", "forget…") | 🧪 | 🧪 | 🧪 | Local `memory.json`; passwords and keys refused. |
| Learning from quiet chats (Automatic) | 🧪 | 🧪 | 🧪 | A cheap background pass a few minutes after a chat goes quiet. |

## Agents and tools

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Background agents | ✅ | 🧪 | 🧪 | |
| Explanation notes window | ✅ | 🧪 | 🧪 | |
| Shell tool | ✅ | 🧪 | 🧪 | Windows: Git Bash when installed, otherwise PowerShell. |
| Files: read, write, edit, find | ✅ | 🧪 | 🧪 | |
| Search inside files (grep) | ✅ | 🧪 | 🧪 | Windows/Linux use a built-in search. |
| Find files on the computer | ✅ | 🟡 | 🟡 | Mac Spotlight; elsewhere a walk of Desktop, Documents, Downloads. |
| Look at images (any format) | ✅ | 🧪 | 🧪 | |
| Read PDF / Word / RTF | ✅ | ❌ | ❌ | Copied into the job folder for the shell instead. |
| Make PDFs from HTML | ✅ | ❌ | ❌ | Uses WebKit through the Mac helper. |
| Save to the computer (never overwrites) | ✅ | 🧪 | 🧪 | |
| Private paths blocked (keys, .env, credentials) | ✅ | 🧪 | 🧪 | Windows adds Credentials, Protect and Vault. |
| Local coding agents (Codex, Claude Code, opencode) | ✅ | 🧪 | 🧪 | Found on PATH, Windows also in %APPDATA%\npm. |
| Playbooks (skills) | ✅ | 🧪 | 🧪 | |
| Web search and fetch | ✅ | 🧪 | 🧪 | |
| Clipboard tool | ✅ | 🧪 | 🧪 | |

## Apps and connectors

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Calendar | ✅ | n/a | n/a | macOS Calendar (EventKit). Use Google on Windows/Linux. |
| Reminders | ✅ | n/a | n/a | |
| Contacts | ✅ | n/a | n/a | |
| Apple Mail drafts | ✅ | n/a | n/a | |
| Google sign-in: Gmail drafts, Google Calendar | ✅ | 🧪 | 🧪 | |
| Connectors switch (Settings → Experimental) | ✅ | 🧪 | 🧪 | 0.8.0: off hides the page, stops the servers and their tools. On by default only if you'd added connectors before. |
| Connectors gallery (GitHub, Linear, Jira & Confluence, Sentry, Notion, Slack, Brave Search, Context7, a folder) | 🧪 | 🧪 | 🧪 | Needs Node.js. Moved to current servers in 0.7.5 (GitHub, Slack and Brave's old packages were retired); not yet connected to each service on a real machine. |
| Connector status, plain-language errors, Retry, tool lists, add your own server | 🧪 | 🧪 | 🧪 | |
| Custom MCP servers (mcp.json) | ✅ | 🧪 | 🧪 | |
| Browser control (Chrome, Brave, Edge, Arc) | ✅ | ❌ | 🧪 | Windows needs a registry entry and a named pipe; not done. |

## Models

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Claude, OpenAI, DeepSeek | ✅ | 🧪 | 🧪 | |
| opencode on this computer (free models, no key) | ✅ | 🧪 | 🧪 | 0.8.0. Checked on the Mac through opencode with DeepSeek Flash (voice, pointing, the pixie tool, a 13-step background job); free models not yet (their quota was used up). Windows untested. |
| Your own API keys | ✅ | 🧪 | 🧪 | |
| Live model lists with prices | ✅ | 🧪 | 🧪 | |
| Separate voice and agents models, fallback | ✅ | 🧪 | 🧪 | |
| Usage and cost tracking | ✅ | 🧪 | 🧪 | Counts and prices cached tokens separately (0.8.0). |
| Prompt caching (stable system prompt, Claude cache marks, smaller screen text) | ✅ | 🧪 | 🧪 | 0.8.0. Measured on DeepSeek; Claude's cache marks are unit-tested but not yet run against Claude. |

## Settings

| Page / setting | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Search in Settings | ✅ | 🧪 | 🧪 | |
| General: buddy (always / idle / off) | ✅ | 🧪 | 🧪 | |
| General: let Pixie click and type | ✅ | 🧪 | 🟡 | |
| General: look at the screen when asked | ✅ | 🧪 | 🧪 | |
| General: speak answers | ✅ | 🧪 | 🧪 | |
| General: typed questions get typed answers, shown in the notch | 🧪 | 🧪 | 🧪 | 0.8.0. |
| General: your name | ✅ | 🧪 | 🧪 | |
| General: talk / dictate shortcuts | ✅ | 🧪 | ❌ | Windows shows Ctrl / Alt / Win and drops the fn presets. |
| Voice: natural or system voice, speed, loudness | ✅ | 🟡 | 🟡 | System voice only (loudness is for the natural voice). |
| Voice: mic test, listening language | ✅ | 🧪 | ❌ | |
| Sounds: volume, custom sounds | ✅ | 🧪 | 🧪 | |
| Memory: automatic / only when asked / off, new-chat hour, notes (add, edit, forget, search) | 🧪 | 🧪 | 🧪 | |
| Models | ✅ | 🧪 | 🧪 | Warns when the talk or agent model can't see screenshots. |
| Usage | ✅ | 🧪 | 🧪 | |
| Coding | ✅ | 🧪 | 🧪 | 0.8.0: usage left per plan (Codex read from its logs; Claude Code set by hand), picks the agent with usage left. |
| Permissions | ✅ | 🧪 | 🧪 | Windows: links to Microphone and Speech privacy. Linux: which tools to install. |
| Apps (+ Chrome) | ✅ | 🟡 | 🟡 | Google only on Windows and Linux. No Chrome bridge on Windows. |
| Connectors (behind Experimental) | ✅ | 🧪 | 🧪 | |
| Experimental | ✅ | 🧪 | 🧪 | Connectors, 3D explanations. |
| About | ✅ | 🧪 | 🧪 | |
| Onboarding (welcome steps) | ✅ | 🧪 | 🧪 | |
| General: personality (Pix, Inos, Xtrem with Spicy, Eon) and drawing | 🧪 | 🧪 | 🧪 | The face changes in the notch, the buddy and Settings. |
| Experimental | ✅ | 🧪 | 🧪 | Connectors, 3D explanations. |
| About: updates (automatic, Check now, Restart now) | 🧪 | 🧪 | 🧪 | |

## The app itself

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Installer | ✅ | ✅ | 🧪 | Universal .dmg · NSIS .exe · .AppImage and .deb (CI job added, first run pending). |
| Tray / menu bar icon | ✅ | 🧪 | 🧪 | Linux needs an AppIndicator-capable desktop. |
| Reopen from the Dock opens Settings | ✅ | n/a | n/a | |
| One copy at a time | ✅ | 🧪 | 🧪 | |
| Pixel design | ✅ | 🧪 | 🧪 | |
| Auto update from GitHub Releases | 🧪 | 🧪 | 🧪 | Signed `latest.json`; checks on launch and every 6 h, installs when Pixie is idle, or on Restart now / Quit. Starts working from the first release built with the signing key. |
| Open at login | ❌ | ❌ | ❌ | |
| CI checks (clippy + tests) | ✅ | 🧪 | 🧪 | Windows and Linux jobs added to `ci.yml`. |
