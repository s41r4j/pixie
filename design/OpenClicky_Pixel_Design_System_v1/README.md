# OpenClicky :) — Monochrome Pixel Design System v1.0

The production-oriented **black/white pixel** brand kit matching the approved visual sheet.

## Start here

1. Read `STYLE_GUIDE.md` (brand system, exact tokens, layout, mic states, accessibility and cross-platform rules).
2. Open `preview/index.html` locally to browse the actual icons, wallpapers, components and theme switcher. The only optional network use is for web font imports; it falls back offline.
3. Wire `tokens/openclicky.css` and `tokens/tokens.ts` into the Tauri frontend.
4. Use `assets/platform/macos/OpenClicky-default.icns` and `assets/platform/windows/OpenClicky-default.ico` for installers.
5. Read `reference.md` and `REPLACEMENT_MAP.csv` before replacing legacy HeyClicky resource slots.
6. Use `assets/audio` for original soft pixel-themed cues, and provide controls to disable sounds.

## Palette

`#000000` · `#404040` · `#A0A0A0` · `#E5E5E5` · `#FFFFFF`

Sub-surface neutrals: `#080808`, `#141414`. No color gradients.

## Build / audit

The programmatic construction source is `scripts/generate_assets.py`. Re-run using Python, Pillow, NumPy and FFmpeg. It regenerates everything in the same output folder in `/mnt/data`. `VALIDATION.txt` records integrity and size checks of the delivered run.

## Not included

Modern HeyClicky proprietary assets, third-party copyrighted logos, actual voice samples, and app footage. See `reference.md` for honest statuses and replacements. No font binaries distributed.
