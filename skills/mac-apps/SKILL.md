---
name: mac-apps
description: Work with Mail, Calendar, Reminders, Notes, Finder, Music and Safari through their own scripting (osascript).
---
# Mac apps through their own scripting

Apps' own AppleScript dictionaries are fine to use; scripting clicks/keys through System
Events is not. Run with `osascript -e '…'` from bash. The first time, macOS asks them to
allow openclicky to control the app: say so if a call fails with "not authorized".

- **Reminders**: `tell application "Reminders" to make new reminder in list "Reminders" with properties {name:"Call Sam", due date:date "Friday 9:00"}`
- **Calendar** (read): `tell application "Calendar" to get summary of (events of calendar "Home" whose start date > (current date))`
  Adding an event is a write to their calendar: fine when they asked for it.
- **Notes**: `tell application "Notes" to make new note at folder "Notes" with properties {name:"Title", body:"<h1>Title</h1><p>…</p>"}`
- **Mail**: drafting is fine (`make new outgoing message … with properties {visible:true}`);
  sending needs their explicit yes to that exact text.
- **Finder**: prefer `device_*` tools; `open -R path` reveals a file.
- **Music**: `tell application "Music" to play playlist "…"`.

Dates in AppleScript follow their locale; build them with `current date` arithmetic when unsure.
