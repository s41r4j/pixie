# openclicky browser extension (MV3)

Lets openclicky use your Chrome when you ask it to: list tabs, open pages, read them as a
numbered outline, click, type, scroll, take screenshots. Wire path:

```
openclicky.app  ⇄  unix socket  ⇄  native host (com.openclicky.browser)  ⇄  this extension
```

- The `key` in `manifest.json` pins the extension ID to `hdbcbdoddehijplnbajmbhlpjammchmf`, which the
  native host manifest allows. The app writes that manifest for Chrome, Chromium, Brave, Edge and
  Arc on launch, and copies this folder to `~/Library/Application Support/com.openclicky.app/extension`.
- Install: `chrome://extensions` → Developer mode → Load unpacked → that folder.
- Pages are read and driven with `chrome.scripting` (no debugger banner). Password fields are
  never typed into. Only http(s) pages without credentials in the URL.
- TODO: CDP (`chrome.debugger`) for trusted input events, downloads/uploads, dialogs.
