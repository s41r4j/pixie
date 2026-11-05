# OpenClicky — Design System v2 / Asset Reference

This is the **corrected** production pack for the previously shared premium design-sheet concept. The previous first-pass package was inconsistent: it contained only three low-detail app variants and did not include the microphone/system icon set shown in the board. This v2 package recreates **four distinct polished icon treatments** (Default, Light, Mono, Glass), a full outlined system icon set, 4 microphone states, 4 independent 4K backgrounds, UI pill assets, and native macOS/Windows exports.

## What is in the ZIP

- `app-icons/svg/` — **4 editable master SVGs**, one for each named design-sheet icon. Not upscaled crops.
- `app-icons/png/` — Each of the 4 icons exported at 1024, 512, 256, 128, 64, 48, 32, 24, 16 px (transparent outside the icon tile).
- `platform/macOS/` — `Default.iconset`, `Light.iconset`, `Mono.iconset`, `Glass.iconset`; each with complete 10-name iconset ladder, plus separate valid `.icns` files. `tray-template-22/44.png` uses no background tile.
- `platform/Windows/` — Four multi-size `.ico` files and Windows Store-sized PNGs. Use full-color `Default` unless user chooses a mode.
- `platform/extensions/` — toolbar/browser action icons and disabled states.
- `system-icons/svg/` & `system-icons/png/` — 15 original outline glyphs (microphone, sparkle, focus, cursor, layers, cube, chat, waveform, settings, bolt, code terminal, grid, sliders, refresh, mic-off). PNGs are exported at 32/64/128/256.
- `microphone/svg/` & `microphone/png/` — Idle, Listening, Processing, Muted state icons, 32/64/128/256.
- `wallpapers/svg/` & `wallpapers/png/` — Amber, Pearl, Violet, Horizon wave series, each 3840×2160 PNG with an editable vector original.
- `brand/svg/` & `brand/png/` — transparent mark and horizontal wordmarks for light/dark usage.
- `ui/svg/` & `ui/png/` — OpenClicky listening pill.
- `audio/` — retained original OpenClicky synthesized UI/agent/drawing interaction sound effects from the first pack; see original slot list.
- `preview/` — icon and wallpaper contact sheets generated from the actual export files.
- `design-tokens.json` — editable colors and gradients.
- `REPLACEMENT_MAP.csv` — direct old-asset slot to v2 file paths and related notes.

## Canonical defaults

**Default is the shipping app icon.** Light, Mono, Glass are optional appearance choices, *not* four alternative attempts at one default icon. Rebuild icons from the `.svg` masters when changing dimensions or colors.

**macOS:** For Xcode/AppKit use `platform/macOS/OpenClicky-Default.icns` or `platform/macOS/Default.iconset/` depending on the build. For Tauri, use `app-icons/png/default-1024.png` and the corresponding generated `.icns` file.

**Windows:** Use `platform/Windows/OpenClicky-Default.ico` for `.exe` and installer branding. Store art uses `OpenClicky-Default-Store.png`.

**Tray:** Use `platform/macOS/tray-template.png` as a monochrome alpha-based template image; do not put the full colored app icon in the macOS menubar.

**React / Tauri:** Prefer the SVG glyphs as UI components, and adopt `design-tokens.json` for CSS variables. Keep hit targets platform-native even when an icon asset uses the premium style.

## Reference mapping and distinctions

The reference design sheet is illustrative concept artwork; exported assets are clean vector redrawings that preserve the key treatments (dark amethyst + amber glass for Default, warm ivory and dark ink for Light, quiet graphite for Mono, translucent plum-glass for Glass). These are not pixel-exact crops from the sheet: the source preview was only a flattened raster illustration.

### Original HeyClicky slots addressed

- `app-icon-1024.png`, `app-icon-512.png`, `app-icon-{256,128,64,32,16}.png` → `app-icons/png/default-{size}.png`
- `AppIcon`, `heyclicky-logo` → `app-icons/png/default-1024.png` or `brand/png/smile-ivory-512.png` depending on placement.
- `tray-template.png` → `platform/macOS/tray-template.png`
- `ext-icon-{16,32,48,128}.png` → `platform/extensions/ext-icon-{size}.png`
- `ext-toolbar-{16,32}.png` and `-off.png` → matching `platform/extensions/` files.
- Voice mic, muted, listening and processing glyph usage → `microphone/png/mic-{state}-{size}.png`; responsive SVG masters in `microphone/svg/`.
- `ClickyCharacters` → not reproduced; original character art is not part of this asset pack.
- Onboarding video, paywall video, voice previews and third-party connector logos → **not recreated**. These require original new production content or appropriately licensed third-party brand assets.
- Agent/chat/drawing audio slots → retained from the first independent procedural audio pack (not copies of HeyClicky's original sounds).

No official affiliation with HeyClicky. Original Clicky source code (MIT) is at https://github.com/farzaa/clicky ; current product is at https://www.heyclicky.com/. Reproduce independent functionality and do not redistribute proprietary media extracted from the commercial app.

## QA criteria

- All four SVG masters render separately at 1024 pixels; Light isn't a recoloring of Default—its surface and wave geometry differ.
- No opaque black canvas around the rounded square. Alpha outside corner radius remains transparent.
- Windows .ico has multiple embedded sizes. macOS .icns is packaged for each variant.
- The menu bar mark is monochrome and tile-free.
- Backgrounds are **3840×2160** and have four distinct compositions.
- Individual system/mic SVGs are editable and their PNGs render at 32 px as well as 256 px.
