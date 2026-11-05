#!/usr/bin/env node
// Re-sign the built app so its identity is stable across rebuilds, then make the DMG.
//
// Why: an ad-hoc signature's designated requirement defaults to `cdhash H"…"`, which changes on
// every build. macOS stores Screen Recording / Accessibility grants against that requirement, so
// a rebuilt app shows as "Not allowed" even though its switch is on. We pin the requirement to
// the bundle identifier instead (`identifier "com.openclicky.app"`), which every build satisfies.
// With a real certificate (APPLE_SIGNING_IDENTITY) the default requirement is already stable.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const target = process.argv[2] ?? "target/release/bundle";
const bundleDir = path.join(repo, target);
const app = path.join(bundleDir, "macos/Pixie.app");
const entitlements = path.join(repo, "apps/desktop/src-tauri/entitlements.plist");
const identity = process.env.APPLE_SIGNING_IDENTITY || "-";
const version = JSON.parse(fs.readFileSync(path.join(repo, "apps/desktop/src-tauri/tauri.conf.json"), "utf8")).version;

const sign = (file, ident) => {
  const args = ["--force", "--sign", identity, "--options", "runtime", "--entitlements", entitlements, "--identifier", ident];
  if (identity === "-") args.push(`-r=designated => identifier "${ident}"`);
  execFileSync("codesign", [...args, file], { stdio: "inherit" });
};

if (!fs.existsSync(app)) {
  console.error(`no app at ${app}`);
  process.exit(1);
}
// Inner executables first, then the bundle (which seals them).
sign(path.join(app, "Contents/MacOS/openclicky-helper"), "com.openclicky.helper");
sign(path.join(app, "Contents/MacOS/openclicky-voice"), "com.openclicky.voice");
sign(app, "com.openclicky.app");
execFileSync("codesign", ["--verify", "--deep", "--strict", app], { stdio: "inherit" });
const req = execFileSync("codesign", ["-d", "-r-", app], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
console.log(req.trim().split("\n").pop());

const dmgDir = path.join(bundleDir, "dmg");
fs.mkdirSync(dmgDir, { recursive: true });
const dmg = path.join(dmgDir, `Pixie_${version}_${process.arch === "arm64" ? "aarch64" : "x64"}.dmg`);
fs.rmSync(dmg, { force: true });
const stage = fs.mkdtempSync(path.join(dmgDir, "stage-"));
execFileSync("ditto", [app, path.join(stage, "Pixie.app")]);
fs.symlinkSync("/Applications", path.join(stage, "Applications"));
execFileSync("hdiutil", ["create", "-volname", "Pixie", "-srcfolder", stage, "-ov", "-format", "UDZO", dmg], { stdio: "ignore" });
fs.rmSync(stage, { recursive: true, force: true });
console.log(`app: ${app}\ndmg: ${dmg}`);
