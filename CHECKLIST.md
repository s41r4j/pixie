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
| Voice input (speech to text) | ✅ | 🧪 | ❌ | Mac: Apple Speech, on-device. Windows: Windows speech recognition (needs Settings → Privacy → Speech → Online speech recognition). |
| Live transcript while talking | ✅ | 🧪 | ❌ | |
| Mic level waveform | ✅ | 🧪 | ❌ | |
| Spoken answers: system voice | ✅ | 🧪 | 🧪 | Windows voices; Linux uses espeak-ng or pico2wave if installed. |
| Spoken answers: natural voice (Kokoro) | ✅ | ❌ | ❌ | The voice runtime ships for macOS only so far. |
| Words light up as Pixie speaks | ✅ | 🧪 | 🧪 | Windows/Linux time words by length. |
| Escape stops Pixie | ✅ | 🧪 | ❌ | |
| Sounds (starts, finishes, nudges) | ✅ | 🧪 | 🧪 | |
| Ask Pixie about itself ("what can you do?", "what's my shortcut?") | ✅ | 🧪 | 🧪 | Answers from its live setup: version, shortcuts, voice, model, permissions, apps, connectors, memory, updates, usage. |
| Change Pixie's settings by asking ("talk slower", "use the Emma voice", "turn on the canvas") | ✅ | 🧪 | 🧪 | Several at once, instantly, no clicking through Settings. Keys, permissions and sign-ins stay with you. |
| "Quit" / "exit" / "close Pixie" | ✅ | 🧪 | 🧪 | Says bye and quits. Any other phrasing goes through the model. |
| "Stop working" / "stop everything" | ✅ | 🧪 | 🧪 | Stops every running job and live agent. "Never mind" / "stop" just stops talking. |
| Personalities (Pix, Inos, Xtrem, Eon) with their own faces | 🧪 | 🧪 | 🧪 | Style of replies only; same voice, tools and care. Jobs' documents stay plain. |

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
| Excalidraw canvas | ✅ | 🧪 | 🧪 | Editable diagrams in a canvas window; shapes, arrows, text or Mermaid; saves locally; exports .excalidraw, PNG, SVG to Downloads. Pixie can open, list and delete canvases; the window hides on a click outside; Show Canvas in the menu bar menu. Replaced the old board in 0.7.6. |
| Drawing on its own when it helps (Only when I ask / When it helps / Often) | 🧪 | 🧪 | 🧪 | Settings → General → Drawing, or "draw more". |
| Click and type for you | ✅ | 🧪 | 🟡 | Windows SendInput. Linux xdotool, X11 only. |
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
| Connectors gallery (GitHub, Linear, Jira & Confluence, Sentry, Notion, Slack, Brave Search, Context7, a folder) | 🧪 | 🧪 | 🧪 | Needs Node.js. Moved to current servers in 0.7.5 (GitHub, Slack and Brave's old packages were retired); not yet connected to each service on a real machine. |
| Connector status, plain-language errors, Retry, tool lists, add your own server | 🧪 | 🧪 | 🧪 | |
| Custom MCP servers (mcp.json) | ✅ | 🧪 | 🧪 | |
| Browser control (Chrome, Brave, Edge, Arc) | ✅ | ❌ | 🧪 | Windows needs a registry entry and a named pipe; not done. |

## Models

| Feature | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Claude, OpenAI, DeepSeek | ✅ | 🧪 | 🧪 | |
| Your own API keys | ✅ | 🧪 | 🧪 | |
| Live model lists with prices | ✅ | 🧪 | 🧪 | |
| Separate voice and agents models, fallback | ✅ | 🧪 | 🧪 | |
| Usage and cost tracking | ✅ | 🧪 | 🧪 | |

## Settings

| Page / setting | Mac | Windows | Linux | Notes |
|---|:-:|:-:|:-:|---|
| Search in Settings | ✅ | 🧪 | 🧪 | |
| General: buddy (always / idle / off) | ✅ | 🧪 | 🧪 | |
| General: let Pixie click and type | ✅ | 🧪 | 🟡 | |
| General: look at the screen when asked | ✅ | 🧪 | 🧪 | |
| General: speak answers | ✅ | 🧪 | 🧪 | |
| General: your name | ✅ | 🧪 | 🧪 | |
| General: talk / dictate shortcuts | ✅ | 🧪 | ❌ | Windows shows Ctrl / Alt / Win and drops the fn presets. |
| Voice: natural or system voice, speed | ✅ | 🟡 | 🟡 | System voice only. |
| Voice: mic test, listening language | ✅ | 🧪 | ❌ | |
| Sounds: volume, custom sounds | ✅ | 🧪 | 🧪 | |
| Memory: automatic / only when asked / off, new-chat hour, notes (add, edit, forget, search) | 🧪 | 🧪 | 🧪 | |
| Models | ✅ | 🧪 | 🧪 | |
| Usage | ✅ | 🧪 | 🧪 | |
| Coding | ✅ | 🧪 | 🧪 | |
| Permissions | ✅ | 🧪 | 🧪 | Windows: links to Microphone and Speech privacy. Linux: which tools to install. |
| Apps | ✅ | 🟡 | 🟡 | Google only. |
| Connectors (+ Chrome) | ✅ | 🟡 | 🧪 | No Chrome bridge on Windows. |
| About | ✅ | 🧪 | 🧪 | |
| Onboarding (welcome steps) | ✅ | 🧪 | 🧪 | |
| General: personality (Pix, Inos, Xtrem with Spicy, Eon) and drawing | 🧪 | 🧪 | 🧪 | The face changes in the notch, the buddy and Settings. |
| Experimental | ✅ | 🧪 | 🧪 | Empty for now: the canvas graduated. |
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
