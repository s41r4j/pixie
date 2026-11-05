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

// Tauri expects a fixed port and no screen clearing.
export default defineConfig({
  plugins: [react(), slots()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "safari16", outDir: "dist", emptyOutDir: true },
});
