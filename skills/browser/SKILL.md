---
name: browser
description: Use their Chrome through the openclicky extension: read pages, click, fill forms, compare tabs.
---
# Using their browser

- Start with `browser {operation:"tabs"}` to see what's open; `(current)` is the tab they're on.
- `open {url}` puts new tabs in the ":) openclicky" group. Work in your own tabs; only touch
  their current tab when they asked about it.
- `snapshot` lists elements as `role: name [e12]`. Act with refs: `click {ref}`,
  `fill {ref, text, submit?}`, `press {key}`, `scroll {direction}`. After an action you get only
  what changed (`+`/`-` lines).
- `extract` gives the page's text and tables for reading; `screenshot` saves an image you can
  `view_image`.
- Prefer `batch {steps:[…]}` (up to 20 of open/click/fill/press/scroll/wait_for/back) to save time.
- Never type passwords or card numbers; hand those to them. Confirm before sending, buying,
  posting or deleting anything.
