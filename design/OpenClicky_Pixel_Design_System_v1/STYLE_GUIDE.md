# OpenClicky — Monochrome Pixel Design System

**Version:** 1.0.0  
**Identity:** `:)`  
**Platforms:** macOS and Windows (shared visual system; OS APIs remain native)  
**Source of visual direction:** approved black-and-white pixel style sheet, `references/approved-design-sheet.png`.

## 0. Single-sentence art direction

**A premium desktop assistant expressed in the language of an early computer: pure black/white, intentional grayscale and dithering, bitmap glyphs, precise grid alignment, and polished modern interactions.**

The nostalgic rendering method is intentional, not a reason to compromise interaction quality. It should feel like a design-forward current app rather than an emulated DOS terminal.

## 1. Never-do rules

- Never reintroduce purple, gold, rainbow accents, neon, glassmorphism, bloom or 3D beveled icons.
- Never use smooth color gradients; use **ordered 4×4 Bayer dithering** when grayscale transitions are required.
- Never use blurred drop shadows; an optional 4px hard offset is permitted.
- Never scale low-resolution icons with bicubic/bilinear interpolation: use nearest-neighbor or render from SVG at pixel grid boundaries.
- Never let decorative pixel type reduce legibility for body text, technical code or permissions.
- Never use prerecorded voice samples without provider licensing or the user's explicit choice.
- Never copy modern HeyClicky proprietary illustrations/videos. This is independently drawn OpenClicky artwork.

## 2. Exact color primitives (approved sheet)

| Token | Hex | Purpose |
|---|---|---|
| `--oc-black` | `#000000` | Primary canvas, text on white |
| `--oc-charcoal` | `#404040` | Muted borders, shadows, inactive dark areas |
| `--oc-mid` | `#A0A0A0` | Secondary content and 1-bit dithering transitions |
| `--oc-silver` | `#E5E5E5` | Secondary foreground / near-white surfaces |
| `--oc-white` | `#FFFFFF` | Text, glyphs, borders, high-contrast highlight |

Supporting near-black layout surfaces: `#080808` (surface), `#141414` (raised surface). These are grayscale only, never colored highlights.

### Semantic palettes

- **Dark — default:** `canvas #000000`, `surface #080808`, `raised #141414`, `fg #FFFFFF`, `muted #A0A0A0`, `border #404040`, selected bg `#FFFFFF`, selected fg `#000000`.
- **Light:** `canvas #FFFFFF`, `surface #FFFFFF`, `raised #E5E5E5`, `fg #000000`, `muted #404040`, `border #A0A0A0`, selected bg `#000000`, selected fg `#FFFFFF`.
- **High contrast:** strictly `#000000` / `#FFFFFF` foreground, background, borders, state indicators. No state conveyed by grayscale alone.
- **Dither:** same semantic colors as Dark; apply `assets/patterns/bayer25-32.png`, `bayer50-32.png`, or `bayer75-32.png` selectively as textures, never beneath dense body text.

Only these four themes are supported in v1.0. They are palettes, not brand variants: the same smiling brand symbol is used everywhere.

**Contrast:** White-on-black and black-on-white give 21:1; `#A0A0A0` on pure black is above common 4.5:1 normal-text requirements; `#404040` on white is high contrast. **Do not** use `#404040` as small text against black, or `#A0A0A0` as small text against white. Disabled content can be exempt from textual contrast requirements, but still needs other state cues.

## 3. Typography and licensing

| Use | Face | Size | Notes |
|---|---|---|---|
| Brand and display | **Press Start 2P** | 12/16/20/24/32px | Use sparingly; line-height 1.45, no cramped blocks |
| Status and expressive UI | **VT323** | 18/22/26/30px | Labels, badge counters, terminal output |
| Readable UI, code, paragraphs | **IBM Plex Mono** | 12/14/16px | Standard 14–16px, line-height 1.55 |
| OS-native fallback | `monospace` | ≥14px | Always work without online fonts |

Reference links:
- Press Start 2P: https://fonts.google.com/specimen/Press+Start+2P
- VT323: https://fonts.google.com/specimen/VT323
- IBM Plex Mono: https://fonts.google.com/specimen/IBM+Plex+Mono

**No font binary is included in this archive.** Add licensed font subsets to the app build only after reviewing and preserving the applicable redistribution licenses. The sample preview imports Google Fonts if available, and falls back offline.

## 4. Grids, spacing, corner shapes

### Layout grid

- Spacing unit: **4 CSS px**. Allowed rhythm: **4, 8, 12, 16, 24, 32, 48, 64**.
- Icon source grids: **16×16** system glyphs; **64×64** app icon art. All raster exports scale with nearest-neighbor.
- Pixel art may have stepped/chamfered 4/8/12px corners; **do not** apply 24px smooth rounding to pixel-art cards.
- Border weight: 1px substructure, 2px primary panel outlines and current focus, 4px special heavy divider.
- Keep high-DPI Windows/macOS scaling correct: CSS px for layout, pixel grid for image assets. At non-integer OS scaling, keep raster nearest and prefer pixel-aligned SVG.

### Layout presets

| UI element | Default | Notes |
|---|---|---|
| Tray/menubar | 22×22 pt / 44px @2x | black alpha mask template for macOS |
| Floating pill | 320×48px | can contract to 180px; system notch positions differ |
| Collapsed notch | 380×48px | macOS top-center optional; honor safe areas |
| Expanded assistant | 420×640px | scrollable; no forced screen obstruction |
| Agent edge job card | 300×96px | stack up to 3 before scrolling |
| Note card | 420px width | content height flexible |
| Chat message | ≤320px width | clear sender and time affordances |
| Button | 32/40/48px high | hit target at least 44×44 logical px where feasible |
| Screen target | 2px outline | pixel corners; avoid glow; add high-contrast arrows |
| Shortcut HUD | 8px padding increments | keyboard discoverable |

### Motion / transitions

- 80ms: button tap and key state.
- 100–120ms: hover and mic state changes.
- 160ms: panel open/close.
- `steps(3, end)` for cosmetic transitions, but **do not** apply stepped animation to real pointer tracking or screen reticle movement; tracking should remain responsive and accurate.
- For `prefers-reduced-motion` or native OS equivalent, disable decorative loops, background motion, and scrolling animations. Voice capture and focus indicators must still be perceptible.

## 5. Official logo: `:)`

The glyph is the brand. It is not a wordmark emoji. Do not substitute a yellow face, round eyes or smile emoji. The colon should consist of two distinctly separated square pixels/squares and the smile is a **right parenthesis**, not a curved cartoon mouth.

### Four approved application icons

1. **Default** — black field, white stepped frame, light 1-bit stippling in bottom-right, white `:)`.
2. **Light** — white field, black `:)` and minimal stepped footprint.
3. **Outline** — black field, thin white outline, white `:)`, minimal interior.
4. **Dither** — monochrome stippling texture and black `:)`; only five approved grey/white/black levels.

The icon variants are not separate platform brands: bundle the **Default** variant for production, with the other three kept for personalization, documentation, or an OS-aware alternative. The package includes Windows ICO and macOS ICNS for each variant.

**Small-size rule:** 16px and 32px glyphs use their own hand-aligned pixel shapes, so colon dots and smile do not collapse. Larger sizes use nearest-neighbor upscale from the 64px source.

## 6. System icon family

SVG and 16/24/32/48/64/128px PNG assets are included for: `mic`, `mic-off`, `sparkle`, `focus`, `cursor`, `layers`, `chat`, `waveform`, `settings`, `bolt`, `terminal`, `grid`, `sliders`, `sync`, `check`, `close`, `plus`, `arrow`, `eye`, `speaker`, `pause`, `stop`, `record`, `screen`, `lock`, `keyboard`.

- Each SVG uses a `16×16` logical viewBox, crisp rect/path geometry, and no Gaussian blur.
- Use `24px` for regular desktop UI, `16px` for dense menus, `32px` for primary controls.
- For interactive icons, add native `aria-label` or `accessibilityLabel` equivalents. Never label actions by graphic alone.
- Map glyph roles semantically: speaker for audio output, mic for input, lock for permissions, focus for screen-target selection.

## 7. Microphone states

| State | UI | What the user should perceive |
|---|---|---|
| Idle | base outlined microphone | Mic **not listening** |
| Listening | heavy frame, orbit ticks | Actively **capturing microphone** (consent required) |
| Processing | explicit waveform | Capture stopped, request being processed |
| Muted | crossed microphone | Microphone disabled; tap to unmute if authorized |

Use the four separate exported images, not color-shifted versions of the same image. All state changes must have a text label and an accessible announcement. Animations are decorative and can be disabled.

## 8. Wallpaper and pattern family

Four separately authored original scenes, each in Dark and Light at 3840×2160 (4K) and 1920×1080:

1. `01-moonrise` — moon, stars, mountain tiers.
2. `02-contour` — topographic contour wavefront.
3. `03-night-city` — high-contrast city skyline.
4. `04-grid-world` — perspective grid, wire mountains, focus marker.

`assets/patterns/`: checker, scanlines, Bayer 25/50/75%, stipple and grid. These are **tileable 32×32** PNG and SVG patterns. Use transparent overlays sparingly, never below text-heavy fields.

## 9. UI components

### Buttons

- **Primary:** white background with black text in Dark; invert in Light.
- **Secondary:** black/white outline and transparent interior.
- **Ghost:** dotted outline, no fill.
- **Danger:** maintain monochrome but require warning icon, descriptive text, confirmation step; do not imply danger with unsupported color hue.
- Pressed = 2px offset, shadow removed. Keyboard focus = 2px dashed visible outline offset by 4px.

### Inputs, toggles, sliders, dialogs

- Input `44px` height, 2px strong outline; focus must remain clear over dither textures.
- Toggle must contain on/off status text or explicit check glyph; do not communicate solely through position.
- Sliders: square handles, 2px track, keyboard arrow support.
- Toasts: short, text-first, icon-assisted; optional soft audio, never sound-only.
- Dialog: retain focus, explicit cancel, no click-through of controlled overlay while active.
- Notch: never overlap system menu items or cutouts; clamp to safe area and per-monitor work area.

### Screen capture and pointer overlays

- Use native OS permission prompts; do not draw fake permission dialogs.
- No permanent recording indicator unless actually capturing; always display capture state accurately.
- Overlay should be click-through except when user deliberately enters annotation/control mode.
- Reticles and screen coordinate pointers must be responsive, and target validation should use accessibility tree data and OCR when permitted. Accuracy matters more than animation.

## 10. Sound identity

This package includes **82 independently synthesized pixel/retro UI sound files**, using the exact legacy slot names in the supplied inventory where available. They are soft, minimal electronic cues, not borrowed HeyClicky recordings. Audio format: **44.1kHz, 16-bit PCM mono WAV**; the two legacy MP3 slots remain MP3.

- Peak is kept below -3dBFS with fades; final product must still be listened to on actual hardware before shipping.
- UI gain 0.7; agent gain 0.8; ambient gain 0.35; voice ducks ambient to 30%.
- Allow one UI sound, one ambient bed, and one voice stream simultaneously.
- Provide a **Sound Effects: Off / Quiet / Normal** setting and honor OS sound preferences.
- Voice samples for model providers are not generated or bundled: fetch from explicitly licensed voices/providers at runtime.

## 11. Platform integration

**macOS:** Use `assets/platform/macos/OpenClicky-default.icns` in the bundle; use `tray-template-22.png` / `tray-template-44.png` as menu-bar template images. On macOS explicitly mark menu-bar images as template, allowing the OS to tint them. Capture and control must use permitted ScreenCaptureKit, Accessibility, and event APIs.

**Windows:** Use `assets/platform/windows/OpenClicky-default.ico` as window/installer icon; use appropriate taskbar/tray icon with dark/light contrast. Screen capture uses Windows Graphics Capture, and input/accessibility interactions use Windows APIs with appropriate permission checks.

**Web/extension:** `assets/platform/browser/extension-icon-{16,32,48,128}.png`, plus toolbar variants. Extension icon size requirements can vary by browser store and manifest version.

**Shared React/Tauri:** Import `tokens/openclicky.css` and/or `tokens/tokens.ts`. SVG glyphs can be loaded as images or component imports; avoid raster scaling artifacts. Native API integration is separate from this asset pack.

## 12. Source, licenses, and release checklist

- Visual concept: generated in this conversation and approved as reference. Actual assets in the pack are newly constructed programmatically.
- Logo master: original pixel geometry; brand name OpenClicky, logo `:)`.
- Audio: procedural originals synthesized in `scripts/generate_assets.py` (no external samples).
- Third-party app/platform logos, copyrighted workflow screenshots, existing proprietary video assets, identities, and voice previews are **not re-created and not included**. Use licensed official marks or record new footage.
- Font links are references only; **no font files are bundled**.
- Run scripts or inspect packaged assets after any changes. Review accessibility, the app security model, at least 100/125/150% Windows scaling, macOS Retina, 16px logo readability, and animation preference support before release.

## 13. Directory reference

- `assets/app-icons` — 4 variants in SVG + 9 PNG sizes.
- `assets/platform/macos` — ICNS, iconsets, menu bar images.
- `assets/platform/windows` — ICO, native tray images.
- `assets/platform/browser` — extension/toolbar icons.
- `assets/system-icons` — 26 original monochrome glyphs.
- `assets/microphone-states` — idle/listening/processing/muted.
- `assets/wallpapers` — 4 scenes × 2 themes × 2 resolutions.
- `assets/patterns` — 7 tileable ordered pixel patterns.
- `assets/onboarding` — 5 designed illustrative fallbacks.
- `assets/ui-illustrations` — note, connector placeholder, reticle, permissions.
- `assets/audio` — 82 slot-compatible new SFX plus slot manifest.
- `assets/compat` — slot-named export copies.
- `tokens` — JSON, TypeScript, CSS palettes.
- `preview/index.html` — interactive design viewer.
- `preview/contact-sheet.png` — asset specimen image.
- `reference.md` — replacement guidance and missing-proprietary-content policy.
- `REPLACEMENT_MAP.csv` and `MANIFEST.csv` — per-file mapping and deliverable inventory.
- `scripts/generate_assets.py` — reproducible generator source.
