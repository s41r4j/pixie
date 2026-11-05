# openclicky — asset list

Every asset is a **slot**: the filename is fixed, the content is swappable.

`Size` = pixels or duration. `Expected` = what the file must contain/feel like.


## 1. Audio — UI, agent & drawing

| Slot file(s) | Position | Kind | Size | Expected |
|---|---|---|---|---|
| agent-close.wav, agent-done.wav, agent-launch.wav, agent-needs-you.wav | Agent state (pill / notch) | one-shot | 800–1120 ms | Launch, done, needs-you, close. Clear, soft, no melody. |
| badge-pop-1.wav, badge-pop-2.wav, badge-pop-3.wav | Edge card badge | one-shot | 300 ms | Counter increments on a job card. |
| boing-1.wav, boing-2.wav | Overlay canvas | one-shot | 450 ms | A drawn object lands or bounces. |
| build-3d-1.wav, build-3d-2.wav, build-3d-3.wav, build-3d-4.wav | 3D build scene | 4-step sequence | 300 ms | Assembly ticks while a 3D object builds. |
| button-appear-1.wav, button-appear-2.wav | Note card | one-shot | 400 ms | A button materialises in note.md. |
| clicky-question.wav | Voice bubble | one-shot | 850 ms | Agent asks the user something. |
| clicky-surprised.wav | Voice bubble | one-shot | 500 ms | Unexpected result, playful surprise. |
| clicky-text-open.wav | Chat bubble | one-shot | 700 ms | Message opens into view. |
| clicky-text-close.wav | Chat bubble | one-shot | 850 ms | Message collapses. |
| clicky-text-send.wav | Chat bubble | one-shot | 500 ms | User message sent. |
| clicky-text-receive.wav | Chat bubble | one-shot | 700 ms | Agent message received. |
| connection-question.wav | Settings / connector card | one-shot | 1500 ms | "Connect this app?" prompt. |
| copy-tick-1.wav, copy-tick-2.wav | Toolbar / note | one-shot | 300 ms | Copy confirmation. |
| ding-1.wav, ding-2.wav | System toast | one-shot | 800 ms | Generic notification. |
| enter.mp3 | Any text field | one-shot | 120 ms | Key press (mp3). |
| eraser-bed-1.wav, eraser-bed-2.wav | Overlay canvas | loop | 2975 ms | Eraser texture while erasing. |
| gel-pop-1.wav, gel-pop-2.wav, gel-pop-3.wav, gel-pop-4.wav | Pill, buttons, cards | one-shot, 4 pitches | 350 ms | Signature UI pop. |
| handwriting-bed-1.wav, handwriting-bed-2.wav | Overlay canvas | loop | 2975 ms | Handwriting texture. |
| hatching.wav | Onboarding | one-shot | 3958 ms | Egg hatch / first reveal. |
| highlighter-1.wav, highlighter-2.wav, highlighter-3.wav | Overlay canvas | one-shot | 900 ms | Highlighter stroke. |
| home-reveal.wav | Home / notch | one-shot | 1760 ms | Home surface reveal. |
| nope-1.wav, nope-2.wav | Anywhere | one-shot | 550 ms | Refusal / wrong action. |
| page-clear-1.wav, page-clear-2.wav | Note card | one-shot | 750 ms | Note cleared for a new turn. |
| paper-place-1.wav, paper-place-2.wav, paper-place-3.wav | Note card | one-shot | 700 ms | A card or line is placed on the page. |
| pen-flick-1.wav, pen-flick-2.wav, pen-flick-3.wav | Overlay canvas | one-shot | 250 ms | Pen tap / flick. |
| pen-screen-bed-1.wav | Overlay canvas | loop | 2975 ms | Pen-on-screen texture. |
| pencil-paper-bed-1.wav, pencil-paper-bed-2.wav | Overlay canvas | loop | 2975 ms | Pencil-on-paper texture. |
| photo-drop-1.wav, photo-drop-2.wav, photo-drop-3.wav | Note card | one-shot | 450 ms | Image dropped into the note. |
| reveal-boot.wav | App launch | one-shot | 1360 ms | Boot reveal. |
| skill-up.wav | Skills picker | one-shot | 790 ms | Skill toggled on. |
| skill-down.wav | Skills picker | one-shot | 750 ms | Skill toggled off. |
| slide-1.wav, slide-2.wav, slide-3.wav | Edge cards / sheets | one-shot | 450 ms | Panel slides in. |
| sparkle-1.wav, sparkle-2.wav | Anywhere | one-shot | 800 ms | Magic / success sparkle. |
| stamp-1.wav, stamp-2.wav | Note card | one-shot | 327–400 ms | Stamp pressed. |
| sticky-press-1.wav, sticky-press-2.wav, sticky-press-3.wav | Note card | one-shot | 400 ms | Sticky note placed. |
| tada-1.wav, tada-2.wav | Job complete | one-shot | 1000 ms | Celebration. |
| tapback-thumbs-up.wav | Chat | one-shot | 497 ms | Reaction. |
| tape-1.wav, tape-2.wav, tape-3.wav | Note card | one-shot | 532–550 ms | Tape strip applied. |
| target-ping-1.wav, target-ping-2.wav | Take-over / locate | one-shot | 700 ms | Element found on screen. |
| type-keys-1.wav, type-keys-2.wav | Take-over | loop | 900 ms | Typing texture. |
| whoosh-1.wav, whoosh-2.wav | Transitions | one-shot | 600 ms | Fast panel transition. |
| eshop.mp3 | Paywall | one-shot (mp3) | 1680 ms | Store / paywall sting. |
| voice-preview-* (13 files) | Settings → Voice | audition | 2403–4362 ms | TTS voice sample. |
| realtime-voice-preview-* (10 files) | Settings → Voice | audition | 3648–6408 ms | Realtime voice sample. |
| gemini-voice-preview-* (30 files) | Settings → Voice | audition | 3888–6600 ms | Gemini voice sample. |

## 2. Audio rules

| Rule | Value |
|---|---|
| Format | WAV 44.1 kHz / 16-bit / mono (stings may be stereo); `enter`, `eshop` mp3 |
| Peak | −3 dBFS, no clipping, 20 ms fade in/out on one-shots |
| Beds | loop seamlessly, 2.8–3.0 s, no transient at the seam |
| Gain | ui 0.7 · agent 0.8 · ambient 0.35 |
| Concurrency | max 1 ui + 1 bed + 1 voice |
| Ducking | voice ducks beds to 30% |

## 3. Video

| Slot file | Size | Duration | Position | Expected |
|---|---|---|---|---|
| onboarding-intro-v2.mp4 | 1280×720 | 117.8 s | First-run, full bleed | H.264 + AAC, no letterbox, ends on home |
| paywall-intro-v2.mp4 | 1280×720 | 32.3 s | Paywall sheet | Plays muted-capable, one pass |
| clicky-drive-sample.mp4 | 1280×720 | 10.0 s | Onboarding demo card | Silent, loopable |
| clicky-spatial-demo.mp4 | 944×576 | 5.0 s | Draw/3D demo card | Silent, loopable |

## 4. Icons

| Slot file | Size | Position | Expected |
|---|---|---|---|
| app-icon-1024.png | 1024×1024 | App, DMG, About | Rounded-square art, opaque |
| app-icon-512.png | 512×512 | Finder | Same art |
| app-icon-{256,128,64,32,16}.png | as named | `.iconset` | Full ladder |
| tray-template.png | 22×22 (+44×44 @2x) | Menu bar | Monochrome template, alpha only |
| ext-icon-{16,32,48,128}.png | as named | Chrome toolbar/store | Full colour, 1 px padding |
| ext-toolbar-{16,32}.png | as named | Extension action | Idle |
| ext-toolbar-{16,32}-off.png | as named | Extension action | Disconnected (dimmed) |

## 5. Images (in-app)

| Asset | Size | Position | Expected |
|---|---|---|---|
| AppIcon | 1024×1024 | App icon renditions | PNG sRGB, @1x/@2x/@3x |
| ClickyCharacters | 1678×937 | Onboarding/home hero sprite | PNG sRGB, @1x/@2x/@3x |
| add-a-project-in-codex | 1090×898 | Guide: add project | PNG sRGB, @1x/@2x/@3x |
| add-a-project-in-codex-from-sidebar | 666×362 | Guide: add project (sidebar) | PNG sRGB, @1x/@2x/@3x |
| airtable-logo | 354×296 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| apple-notes-logo | 240×240 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| apple-reminders-logo | 400×400 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| catmode-red-notch-atlas | 1280×864 | Easter egg: red notch | PNG sRGB, @1x/@2x/@3x |
| catmode-red-overlay-atlas | 1536×2976 | Easter egg: red overlay | PNG sRGB, @1x/@2x/@3x |
| chatgpt-logo | 640×640 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| claude-code-logo | 2×2 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| claude-logo | 5000×5000 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| codex-app-screenshot | 2000×1125 | Guide: Codex app | PNG sRGB, @1x/@2x/@3x |
| codex-home-screen | 1994×1315 | Guide: Codex home | PNG sRGB, @1x/@2x/@3x |
| codex-permissions | 1686×1556 | Guide: Codex permissions | PNG sRGB, @1x/@2x/@3x |
| discord-logo | 48×48 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| excalidraw-logo | 2000×2000 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| farza-avatar | 80×80 | Testimonial avatar | PNG sRGB, @1x/@2x/@3x |
| findmy-logo | 1000×1000 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| git-tools-prompt | 948×426 | Guide: git tools | PNG sRGB, @1x/@2x/@3x |
| github-logo | 2048×2048 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| gmail-logo | 512×407 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| google-logo | 96×96 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| google-meet-logo | 192×192 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| heyclicky-logo | 1000×1000 | Brand mark (about, share) | PNG sRGB, @1x/@2x/@3x |
| imessage-logo | 376×376 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| inside-the-makesomething-project-folder | 1876×870 | Guide: project folder | PNG sRGB, @1x/@2x/@3x |
| instagram-logo | 48×48 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| linear-logo | 400×400 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| linkedin-logo | 48×48 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| makesomething-project-folder-in-downloads | 1718×649 | Guide: folder in Downloads | PNG sRGB, @1x/@2x/@3x |
| ms-teams-logo | 128×128 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| notion-logo | 200×200 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| obsidian-logo | 1024×1024 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| onboarding-google-unverified-warning | 1468×1016 | Onboarding: Google warning | PNG sRGB, @1x/@2x/@3x |
| onboarding-keychain-allow-prompt | 982×466 | Onboarding: Keychain prompt | PNG sRGB, @1x/@2x/@3x |
| openai-logo | 640×640 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| polymarket-logo | 1024×1024 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| powerpoint-logo | 1163×1125 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| reddit-logo | 48×48 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| referral-sample-farza | 400×400 | Referral sample | PNG sRGB, @1x/@2x/@3x |
| referral-sample-kelli | 400×400 | Referral sample | PNG sRGB, @1x/@2x/@3x |
| referral-sample-nilesh | 400×400 | Referral sample | PNG sRGB, @1x/@2x/@3x |
| spotify-logo | 336×336 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| steve | 964×604 | Testimonial image | PNG sRGB, @1x/@2x/@3x |
| tiktok-logo | 48×48 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| whatsapp-logo | 96×96 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| word-logo | 70×72 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| x-logo | 48×48 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| youtube-logo | 58×40 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |
| zoom-logo | 512×512 | Connector / integration card | PNG sRGB, @1x/@2x/@3x |

## 6. Fonts

| Slot file | Weight | Position | Expected |
|---|---|---|---|
| Barlow-Regular.ttf | 400 | Body, lists | OFL, latin + latin-ext |
| Barlow-Medium.ttf | 500 | Labels, buttons | OFL |
| Barlow-SemiBold.ttf | 600 | Headings, tiles | OFL |
| PatrickHand.ttf | 400 | Handwritten marks, sticky notes | OFL, latin |

## 7. UI geometry

| Element | Size | Position |
|---|---|---|
| Tray icon | 22×22 pt (44 @2x) | Menu bar, right |
| Floating pill | 32×32 pt | Top-right of focused screen, 24 pt inset |
| Notch panel | 380×220 collapsed / 420×640 expanded | Top centre under notch |
| Note card | 420 pt wide | Edge card + full window |
| Edge job cards | 300×96 pt | Right edge, 16 pt gap, 12 pt inset |
| Overlay canvas | full display | Above all windows, click-through |
| Take-over highlight | 2 pt stroke + 12 pt glow | Around target element |
| Chat bubble | max 320 pt wide | Notch panel |

## 8. Still to create

| Asset | Size | Why |
|---|---|---|
| app-icon-1024.png | 1024×1024 | Bundle only has ≤256 px |
| tray-template.png | 22/44 | Not shipped as a file |
| empty-note.png | 840×472 | Empty-state for a fresh note |
| connector-placeholder.png | 128×128 | Unknown connector fallback |
| onboarding-01..05.png | 1280×720 | Static fallback if video fails |
