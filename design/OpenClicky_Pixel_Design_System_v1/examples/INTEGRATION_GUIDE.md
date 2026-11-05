# OpenClicky Pixel UI — Tauri / React integration

This kit implements the **visual layer**, not native permissions, transcription, or AI agents.

## Suggested asset placement

Copy `assets/` into your Tauri frontend's `public/openclicky-assets/`. Put `tokens/openclicky.css` and `examples/AssistantPill.css` into your frontend styles and import them once from your app shell.

```tsx
import { AssistantPill, type AssistantState } from './AssistantPill';
import './openclicky.css';
import './AssistantPill.css';

// `state` must be driven by your actual Rust/native microphone lifecycle.
<AssistantPill
  state={state as AssistantState}
  onPrimaryAction={handleUserApprovedMicAction}
  onOpen={openAssistantPanel}
  shortcutLabel="Ctrl+Space (example)"
/>
```

**Do not** report `listening` until native microphone capture has actually started. Do not declare `muted` merely because the transcription stream is idle. Keep explicit consent and minimize background collection.

### Runtime icon paths

- macOS: `assets/platform/macos/OpenClicky-default.icns` for the app icon.
- Windows: `assets/platform/windows/OpenClicky-default.ico` for the app/installer.
- macOS menubar: use the alpha-mask `tray-template-22.png`/`44.png`, with native template tinting.
- Windows tray: use one of the readable dark/light tray outputs depending on actual background; allow system scaling.
- HTML assistant: SVG glyphs in `system-icons/svg`; no antialiasing on sprite images.

### Theme switching

```ts
document.documentElement.setAttribute('data-oc-theme', 'dark');
// Supported: dark, light, highContrast, dither.
```

For system-driven theme selection, read the OS theme preference and map it to `dark` or `light` without adding unapproved hue accents.

### Design policy

- Hard shadows only; no CSS glow, blurred box shadow, or chromatic backdrop effects.
- Screen reticles should move accurately, not via stepped animation.
- Keep keyboard focus and accessible labels separate from decorative bitmap artwork.
- Sound effects should be opt-in; if enabled, preview at quiet levels and respect reduced motion/audio settings.
