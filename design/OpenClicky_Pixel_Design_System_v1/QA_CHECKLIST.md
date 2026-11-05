# OpenClicky pixel design system — shipping QA checklist

This is a review checklist for the *app build*; the asset package cannot certify OS behavior by itself.

## File integrity
- [ ] All PNG sizes match suffixes in their file names.
- [ ] ICNS files contain proper multi-size PNG entries; macOS app notarization/integration tested on target OS.
- [ ] ICO files contain true multi-size icon frames; Windows taskbar/installer rendering verified.
- [ ] 16px smile still has clearly separated colon dots and a recognizable parenthesis.
- [ ] Dark/Light/Dither/High Contrast token mappings use only approved grayscale.
- [ ] SVG pixel glyphs are well-formed and visually aligned at 16, 24 and 32px.
- [ ] Wallpaper scenes render at full 4K and 1080p without interpolation blur.

## Application UI
- [ ] Dark, Light, High Contrast and Dither selectors persist correctly.
- [ ] Keyboard and assistive-tech focus are visible throughout the notch/pill and dialogs.
- [ ] System fonts / locally licensed font fallbacks work without network access.
- [ ] 100%, 125%, 150%, 200% Windows display scaling inspected manually.
- [ ] Retina/non-Retina and multiple-monitor layouts checked on macOS.
- [ ] Notch and overlay respect menu bar, taskbar, safe areas, and screen capture boundaries.
- [ ] `prefers-reduced-motion` and native reduced-motion options disable decorative animation.

## Sensitive desktop actions
- [ ] Microphone capture starts only after explicit user action and OS permission.
- [ ] Listening/processing/muted text reflects actual native state.
- [ ] Screen capture is disclosed when active and is not incorrectly shown as continuous.
- [ ] Agent actions are reviewed/authorized as appropriate and are cancellable.
- [ ] Third-party connectors show only real, licensed official provider identities.
- [ ] No false OS-permission dialog screenshots or fake success states.

## Audio & missing media
- [ ] Listen to every synthesized effect on headphones and laptop speakers; tune loudness and transients.
- [ ] Audio is optional, not a sole feedback channel, and respects user preference.
- [ ] Replace illustrative onboarding cards with actual OpenClicky video demonstrations where required.
- [ ] Record real up-to-date app-specific screenshots rather than reproducing copyrighted UI.
- [ ] Obtain proper font licenses if font binaries are bundled in a later app release.
