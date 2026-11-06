import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";
import path from "node:path";

// Browser preview only (src/preview.ts): serve resource slots at /__slot/<kind>/<slot>,
// mirroring the app's slot:// protocol, so images load outside Tauri.
function slots(): Plugin {
  const root = path.resolve(__dirname, "../../resources");
  const kinds: Record<string, string> = { icon: "icons", sound: "sounds", video: "videos", font: "fonts" };
  const types: Record<string, string> = { ".png": "image/png", ".svg": "image/svg+xml", ".wav": "audio/wav", ".mp3": "audio/mpeg", ".ttf": "font/ttf", ".mp4": "video/mp4" };
  return {
    name: "openclicky-slots",
    configureServer(server) {
      server.middlewares.use("/__slot", (req, res) => {
        const [kind, slot] = (req.url ?? "").replace(/^\//, "").split("/");
        const manifest = JSON.parse(fs.readFileSync(path.join(root, "manifest.json"), "utf8"));
        const file = manifest[kinds[kind]]?.[slot]?.files?.[0];
        if (!file) {
          res.statusCode = 404;
          return res.end();
        }
        res.setHeader("Content-Type", types[path.extname(file)] ?? "application/octet-stream");
        res.end(fs.readFileSync(path.join(root, file)));
      });
    },
  };
}

// Excalidraw's fonts, served locally so the canvas works offline (public/excalidraw-assets/fonts,
// copied from the package; not committed). The 12 MB CJK font is left out: those characters
// fall back to the system font.
function excalidrawFonts(): Plugin {
  const copy = () => {
    const from = path.resolve(__dirname, "../../node_modules/@excalidraw/excalidraw/dist/prod/fonts");
    const to = path.resolve(__dirname, "public/excalidraw-assets/fonts");
    if (!fs.existsSync(from)) return;
    for (const family of fs.readdirSync(from)) {
      if (family === "Xiaolai") continue;
      fs.cpSync(path.join(from, family), path.join(to, family), { recursive: true });
    }
  };
  return { name: "excalidraw-fonts", buildStart: copy, configureServer: copy };
}

// Tauri expects a fixed port and no screen clearing.
export default defineConfig({
  plugins: [react(), slots(), excalidrawFonts()],
  clearScreen: false,
  // Excalidraw must share the app's React (a second copy sits at the repo root).
  resolve: { dedupe: ["react", "react-dom"] },
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  // The canvas window's Excalidraw + Mermaid chunks are big by nature; they load only there.
  build: { target: "safari16", outDir: "dist", emptyOutDir: true, chunkSizeWarningLimit: 2500 },
});
