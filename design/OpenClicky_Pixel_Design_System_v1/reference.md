# OpenClicky Pixel v1 — asset replacement reference

Source: attached user-provided slot inventory (`references/original-asset-slots.md`) and the user-approved monochrome pixel style sheet (`references/approved-design-sheet.png`).

**This is an original recreation, not a copy of the current proprietary HeyClicky app.** Existing filenames below are described only as integration/replacement targets.

## Exact role mapping

| Existing target | Use from this pack | Integration note |
|---|---|---|
| `app-icon-1024.png` | `assets/compat/app-icon-1024.png` | Canonical pixel Default `:)` app art |
| `app-icon-512.png` | `assets/compat/app-icon-512.png` | Finder/taskbar bitmap rendition |
| `app-icon-{16,32,64,128,256}.png` | `assets/compat/app-icon-<size>.png` | Multi-resolution fallback ladder |
| `AppIcon` | `assets/compat/AppIcon.png` | Source 1024×1024 brand PNG |
| `tray-template.png` | `assets/compat/tray-template.png` | Mark as macOS template image |
| `ext-icon-{16,32,48,128}.png` | `assets/compat/ext-icon-<size>.png` | Browser extension icons |
| `ext-toolbar-{16,32}[-off].png` | `assets/compat/ext-toolbar-<size>[-off].png` | Connected/disconnected toolbar |
| `heyclicky-logo` | `assets/brand/mark-white.svg` or `...mark-black.svg` | Rebrand to **OpenClicky** on all surfaces |
| Existing microphone artwork | `assets/microphone-states` | Four separate explicit states |
| Legacy UI icon set | `assets/system-icons` | 26 crisp editable system glyphs |
| Four background previews | `assets/wallpapers` | 4 new 4K black/white scenes + 4 inverted |
| `empty-note.png` | `assets/ui-illustrations/empty-note-256.png` | Original empty-state illustration |
| `connector-placeholder.png` | `assets/ui-illustrations/connector-placeholder-256.png` | Generic placeholder only; not an official mark |
| `onboarding-01..05.png` | `assets/onboarding` | New original static onboarding designs |
| 82 original legacy audio filenames | `assets/audio/<group>/<filename>` | 82 newly synthesized sounds, original names |
| `Barlow` / `PatrickHand` | Refer to `STYLE_GUIDE.md` | Suggest Press Start 2P / VT323 / IBM Plex Mono; font binaries not included |

## Not directly replaceable from a design pack

1. **Original HeyClicky video clips** (`onboarding-intro-v2.mp4`, `paywall-intro-v2.mp4`, demo samples): remake with actual OpenClicky UI recording, not a copied video.
2. **Provider voice sample libraries** (13+10+30 samples in the reference inventory): produce from permitted TTS providers at runtime. Speech cannot be accurately represented by synthesized UI sound effects.
3. **Third-party service logos** (OpenAI, GitHub, Gmail, etc.): retain properly sourced, licensed official assets only where relevant, otherwise use the placeholder.
4. **Screenshots of other applications and OS permission dialogs:** capture true screens from supported versions and permission states; never fabricate misleading OS images.
5. **Photos and people-specific testimonials:** do not invent or reuse likenesses; ask owners for rights or remove.
6. **Cat mode red atlases:** deliberately retired because the v1 theme is strictly grayscale; needs an independently authored monochrome animation if feature remains.

## What is actually supplied

- Four distinct app icon variants with ICNS and ICO exports.
- PNG compatibility ladder and SVG master art.
- 26 system glyphs in SVG plus six PNG dimensions each.
- Four microphone state graphics, four sizes each.
- Four themes via JSON, TypeScript and CSS tokens.
- 4 illustrations × 2 themes × 2 resolutions = 16 wallpaper PNGs.
- Seven reusable dithering, checkerboard and pixel grid patterns (PNG and SVG).
- Five 1280×720 onboarding static fallbacks.
- Four utility illustration templates.
- 82 original synthesized UI sounds.
- Local clickable sample design board and integration guides.

**See `REPLACEMENT_MAP.csv` for 201 explicit slot mappings and statuses.** A blank destination in that CSV means the content is *not* included.
