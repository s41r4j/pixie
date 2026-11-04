#!/usr/bin/env node
// Build a per-user "local pack" of media for testing on this Mac only.
//
//   npm run assets:local                 # from work/reference-assets (default)
//   npm run assets:local -- --from DIR   # from any folder laid out like resources/
//   npm run assets:local -- --clear      # remove the pack, back to bundled placeholders
//
// The pack lives in ~/Library/Application Support/com.openclicky.app/resources and is picked
// up by the app before the bundled resources (see apps/desktop/src-tauri/src/resources.rs).
// It is never written into the repo or the .app bundle, so nothing third-party is committed
// or redistributed. Slots with no matching source file keep the original placeholder.
// Icons and fonts always stay openclicky's own (brand + OFL); only sounds and videos are taken.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const shipped = path.join(repo, "resources");
const dest = path.join(os.homedir(), "Library/Application Support/com.openclicky.app/resources");

const args = process.argv.slice(2);
if (args.includes("--clear")) {
  fs.rmSync(dest, { recursive: true, force: true });
  console.log(`removed ${dest}`);
  process.exit(0);
}
const fromIdx = args.indexOf("--from");
const from = path.resolve(fromIdx >= 0 ? args[fromIdx + 1] : path.join(repo, "work/reference-assets"));
if (!fs.existsSync(from)) {
  console.error(`source folder not found: ${from}`);
  process.exit(1);
}

const manifest = JSON.parse(fs.readFileSync(path.join(shipped, "manifest.json"), "utf8"));
const TAKE = ["sounds", "videos"];

/** Candidate source paths for a slot file, e.g. videos/onboarding-intro.mp4. */
function candidates(rel) {
  const { dir, name, ext } = path.parse(rel);
  const dirs = dir === "videos" ? ["videos", "video"] : [dir];
  return dirs.flatMap((d) => [path.join(from, d, name + ext), path.join(from, d, `${name}-v2${ext}`)]);
}

function copy(src, rel) {
  const out = path.join(dest, rel);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.copyFileSync(src, out);
}

fs.rmSync(dest, { recursive: true, force: true });
let taken = 0;
let kept = 0;
const keptSlots = [];
for (const kind of ["sounds", "icons", "videos", "fonts"]) {
  for (const [slot, entry] of Object.entries(manifest[kind] ?? {})) {
    for (const rel of entry.files) {
      const src = TAKE.includes(kind) ? candidates(rel).find((p) => fs.existsSync(p)) : undefined;
      if (src) {
        copy(src, rel);
        taken++;
      } else {
        copy(path.join(shipped, rel), rel);
        kept++;
        if (TAKE.includes(kind)) keptSlots.push(`${kind}/${slot}`);
      }
    }
  }
}
for (const f of ["manifest.json", "theme.json"]) copy(path.join(shipped, f), f);
for (const f of fs.readdirSync(path.join(shipped, "fonts")).filter((f) => f.endsWith(".txt"))) {
  copy(path.join(shipped, "fonts", f), path.join("fonts", f));
}

console.log(`local pack → ${dest}`);
console.log(`  ${taken} file(s) from ${path.relative(repo, from) || from}, ${kept} placeholder/own file(s)`);
if (keptSlots.length) console.log(`  no source for: ${[...new Set(keptSlots)].join(", ")}`);
console.log("Restart openclicky to pick it up. Local use only: do not commit or redistribute this pack.");
