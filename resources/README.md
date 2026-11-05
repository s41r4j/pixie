# resources/

All media is loaded by **slot name** through `manifest.json`; code never hardcodes a path.

```jsonc
"sounds": {
  "badge-pop": { "files": ["sounds/badge-pop-1.wav", "…-2", "…-3"], "gain": 0.8, "category": "ui" },
  "pencil-paper-bed": { "files": ["…"], "category": "ambient", "bed": true }
}
```

- `files`: one or more variants (sounds pick one at random; icons/fonts/videos use the first).
- `gain`: per-slot gain (0–2), multiplied by the category gain (`ui`, `agent`, `ambient`) and master.
- `bed`: long looping sound; only one bed plays at a time.

## Replacing media

Drop a file at the same path (or point the manifest at a new one). No code changes.
Sounds and icons come from the OpenClicky Design System v2 (`design/OpenClicky_Design_System_v2`,
original work; `REPLACEMENT_MAP.csv` maps each slot). Videos are still placeholders from
`npm run placeholders` (`scripts/gen-placeholders.mjs`). The script never overwrites existing files. Fonts are the upstream OFL releases
(Barlow, Patrick Hand) from github.com/google/fonts, licences in `fonts/OFL-*.txt`.

Do not copy media from other products into this folder; use originals or CC0 sources.

## Theme

`theme.json` holds the design tokens (BUILD-SPEC §8). The UI reads it at startup and sets
them as CSS custom properties (`colors.ink` → `--ink`, `draw.sky` → `--draw-sky`).

## Local pack

`npm run assets:local` copies media you have locally into
`~/Library/Application Support/com.openclicky.app/resources`, which the app loads before
these files. Use it for private testing; it never touches this folder.
