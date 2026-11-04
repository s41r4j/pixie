#!/usr/bin/env node
// Generates resources/manifest.json (with --manifest, only if absent) and original
// placeholder media for every manifest file that does not exist yet.
//
// Never overwrites: drop real media into resources/ and it is left alone.
// All placeholders are synthesized here (tones, gradients, a blank clip). Fonts are the
// upstream OFL releases from github.com/google/fonts.
//
//   node scripts/gen-placeholders.mjs            # fill in missing files
//   node scripts/gen-placeholders.mjs --manifest # also (re)create manifest.json if absent

import { existsSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "resources");
const MANIFEST = join(ROOT, "manifest.json");

// ---------------------------------------------------------------- manifest

function range(base, n, ext = "wav") {
  return Array.from({ length: n }, (_, i) => `sounds/${base}-${i + 1}.${ext}`);
}

function buildManifest() {
  const s = {};
  const add = (slot, files, opts = {}) => {
    s[slot] = { files, gain: opts.gain ?? 1.0, category: opts.category ?? "ui", ...(opts.bed ? { bed: true } : {}) };
  };
  const agent = { category: "agent" };
  add("agent-launch", ["sounds/agent-launch.wav"], agent);
  add("agent-done", ["sounds/agent-done.wav"], agent);
  add("agent-needs-you", ["sounds/agent-needs-you.wav"], agent);
  add("agent-close", ["sounds/agent-close.wav"], agent);
  add("badge-pop", range("badge-pop", 3), { gain: 0.8 });
  add("clicky-question", ["sounds/clicky-question.wav"], agent);
  add("clicky-surprised", ["sounds/clicky-surprised.wav"], agent);
  for (const v of ["open", "close", "send", "receive"]) add(`clicky-text-${v}`, [`sounds/clicky-text-${v}.wav`], { gain: 0.7 });
  add("ding", range("ding", 2));
  add("enter", ["sounds/enter.mp3"], { gain: 0.8 });
  add("pen-flick", range("pen-flick", 3), { gain: 0.7 });
  add("highlighter", range("highlighter", 3), { gain: 0.7 });
  const bed = { category: "ambient", bed: true, gain: 0.5 };
  add("pencil-paper-bed", range("pencil-paper-bed", 2), bed);
  add("pen-screen-bed", range("pen-screen-bed", 1), bed);
  add("handwriting-bed", range("handwriting-bed", 2), bed);
  add("eraser-bed", range("eraser-bed", 1), bed);
  add("paper-place", range("paper-place", 3));
  add("photo-drop", range("photo-drop", 3));
  add("slide", range("slide", 3));
  add("sticky-press", range("sticky-press", 3));
  add("stamp", range("stamp", 2));
  add("tada", range("tada", 2), agent);
  add("whoosh", range("whoosh", 2));
  add("nope", range("nope", 2), agent);
  add("sparkle", range("sparkle", 2));
  add("target-ping", range("target-ping", 2));
  add("page-clear", range("page-clear", 2));
  add("gel-pop", range("gel-pop", 4), { gain: 0.8 });
  add("type-keys", range("type-keys", 2), { gain: 0.6 });
  add("reveal-boot", ["sounds/reveal-boot.wav"]);
  add("hatching", ["sounds/hatching.wav"]);
  add("home-reveal", ["sounds/home-reveal.wav"]);
  add("boing", range("boing", 2));
  add("build-3d", range("build-3d", 4));

  const one = (f) => ({ files: [f] });
  return {
    version: 1,
    sounds: s,
    icons: {
      "app-icon-1024": one("icons/app-icon-1024.png"),
      "tray-template": one("icons/tray-template.png"),
      "ext-16": one("icons/ext-16.png"),
      "ext-32": one("icons/ext-32.png"),
      "ext-48": one("icons/ext-48.png"),
      "ext-128": one("icons/ext-128.png"),
    },
    videos: {
      "onboarding-intro": one("videos/onboarding-intro.mp4"),
      "paywall-intro": one("videos/paywall-intro.mp4"),
    },
    fonts: {
      "barlow-regular": one("fonts/Barlow-Regular.ttf"),
      "barlow-medium": one("fonts/Barlow-Medium.ttf"),
      "barlow-semibold": one("fonts/Barlow-SemiBold.ttf"),
      "patrick-hand": one("fonts/PatrickHand.ttf"),
    },
  };
}

// ---------------------------------------------------------------- audio

const RATE = 44100;

function hash(str) {
  let h = 2166136261;
  for (const c of str) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return h >>> 0;
}

function rng(seed) {
  let x = seed || 1;
  return () => ((x = (Math.imul(x, 1664525) + 1013904223) >>> 0) / 4294967296);
}

function wav(samples) {
  const data = Buffer.alloc(samples.length * 2);
  samples.forEach((v, i) => data.writeInt16LE(Math.max(-1, Math.min(1, v)) * 32767 | 0, i * 2));
  const h = Buffer.alloc(44);
  h.write("RIFF", 0); h.writeUInt32LE(36 + data.length, 4); h.write("WAVE", 8);
  h.write("fmt ", 12); h.writeUInt32LE(16, 16); h.writeUInt16LE(1, 20); h.writeUInt16LE(1, 22);
  h.writeUInt32LE(RATE, 24); h.writeUInt32LE(RATE * 2, 28); h.writeUInt16LE(2, 32); h.writeUInt16LE(16, 34);
  h.write("data", 36); h.writeUInt32LE(data.length, 40);
  return Buffer.concat([h, data]);
}

// notes: [[freqHz, startSec, durSec]]; soft sine + a little 2nd harmonic, exp decay.
function tones(notes, { amp = 0.35, decay = 6, harm = 0.15 } = {}) {
  const end = Math.max(...notes.map(([, s, d]) => s + d));
  const out = new Float32Array(Math.ceil(end * RATE) + 64);
  for (const [f, s, d] of notes) {
    const i0 = Math.floor(s * RATE), n = Math.floor(d * RATE);
    for (let i = 0; i < n; i++) {
      const t = i / RATE;
      const env = Math.min(1, i / (0.004 * RATE)) * Math.exp(-decay * t / d) * Math.min(1, (n - i) / (0.01 * RATE));
      out[i0 + i] += amp * env * (Math.sin(2 * Math.PI * f * t) + harm * Math.sin(4 * Math.PI * f * t));
    }
  }
  return out;
}

// Filtered noise; `loop` = constant level with short edge fades (for beds).
function noise(seed, dur, { amp = 0.12, smooth = 0.9, loop = false, decay = 4 } = {}) {
  const r = rng(seed), n = Math.floor(dur * RATE), out = new Float32Array(n);
  let y = 0;
  for (let i = 0; i < n; i++) {
    y = smooth * y + (1 - smooth) * (r() * 2 - 1);
    const t = i / n;
    const env = loop ? Math.min(1, i / 2000, (n - i) / 2000) * (0.8 + 0.2 * Math.sin(t * Math.PI * 6))
                     : Math.min(1, i / 200) * Math.exp(-decay * t);
    out[i] = amp * 4 * y * env;
  }
  return out;
}

const N = (semi) => 440 * 2 ** ((semi - 9) / 12 + 1); // semitones from C5

function soundFor(slot, variant) {
  const v = variant - 1, seed = hash(slot) + variant;
  switch (slot) {
    case "agent-launch": return tones([[N(0), 0, 0.12], [N(7), 0.09, 0.22]]);
    case "agent-done": return tones([[N(0), 0, 0.15], [N(4), 0.08, 0.15], [N(7), 0.16, 0.35]]);
    case "agent-needs-you": return tones([[N(4), 0, 0.18], [N(0), 0.16, 0.3]]);
    case "agent-close": return tones([[N(7), 0, 0.1], [N(0), 0.08, 0.2]], { amp: 0.25 });
    case "clicky-question": return tones([[N(0), 0, 0.12], [N(5), 0.1, 0.2]]);
    case "clicky-surprised": return tones([[N(-5), 0, 0.08], [N(9), 0.06, 0.25]]);
    case "tada": return tones([[N(0 + v), 0, 0.12], [N(4 + v), 0.1, 0.12], [N(7 + v), 0.2, 0.12], [N(12 + v), 0.3, 0.5]]);
    case "nope": return tones([[N(-12 - v), 0, 0.12], [N(-13 - v), 0.14, 0.2]], { harm: 0.5 });
    case "ding": return tones([[N(12 + 3 * v), 0, 0.6]], { decay: 5 });
    case "sparkle": return tones([0, 1, 2, 3].map((k) => [N(19 + k * 2 + v), k * 0.05, 0.25]), { amp: 0.2 });
    case "target-ping": return tones([[N(14 + v * 2), 0, 0.35]], { harm: 0, decay: 8 });
    case "badge-pop": case "gel-pop": return tones([[N(7 + v * 3), 0, 0.07]], { decay: 4, harm: 0.3 });
    case "boing": return tones(Array.from({ length: 8 }, (_, k) => [N(-5 + Math.round(4 * Math.sin(k)) + v), k * 0.04, 0.08]), { amp: 0.25 });
    case "build-3d": return tones([[N(-7 + v * 2), 0, 0.1], [N(v * 2), 0.07, 0.15]], { harm: 0.4 });
    case "home-reveal": case "reveal-boot": return tones([[N(-12), 0, 0.6], [N(-5), 0.15, 0.6], [N(0), 0.3, 0.8]], { amp: 0.25, decay: 3 });
    case "hatching": return noise(seed, 0.8, { smooth: 0.5, decay: 2 });
    case "whoosh": case "slide": return noise(seed, 0.35 + 0.05 * v, { smooth: 0.97, amp: 0.25, decay: 3 });
    case "paper-place": case "photo-drop": case "sticky-press": case "stamp": case "page-clear":
      return noise(seed, 0.15, { smooth: 0.8, amp: 0.3, decay: 10 });
    case "pen-flick": case "highlighter": case "type-keys": case "enter":
      return noise(seed, 0.08, { smooth: 0.4, amp: 0.2, decay: 12 });
    default:
      if (slot.endsWith("-bed")) return noise(seed, 3.0, { smooth: slot.startsWith("eraser") ? 0.6 : 0.85, amp: 0.05, loop: true });
      if (slot.startsWith("clicky-text")) return tones([[N(7 + (hash(slot) % 5)), 0, 0.08]], { amp: 0.2 });
      return tones([[N(hash(slot) % 12), 0, 0.12]]);
  }
}

function variantOf(file) {
  const m = file.match(/-(\d+)\.\w+$/);
  return m ? Number(m[1]) : 1;
}

// ---------------------------------------------------------------- png

const CRC = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
function png(size, pixel /* (x,y)->[r,g,b,a] in 0..255 */) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    for (let x = 0; x < size; x++) {
      const [r, g, b, a] = pixel(x + 0.5, y + 0.5);
      raw.set([r, g, b, a].map((v) => Math.max(0, Math.min(255, Math.round(v)))), y * (size * 4 + 1) + 1 + x * 4);
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0); ihdr.writeUInt32BE(size, 4); ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

const lerp = (a, b, t) => a + (b - a) * t;
const clamp01 = (v) => Math.max(0, Math.min(1, v));
// warm sunrise gradient: amber -> coral -> violet
function gradient(t) {
  const stops = [[255, 184, 77], [255, 99, 132], [139, 92, 246]];
  const k = clamp01(t) * 2, i = Math.min(1, Math.floor(k)), f = k - i;
  return stops[i].map((c, j) => lerp(c, stops[i + 1][j], f));
}

// Original mark: gradient orb with a soft white "spark" dot.
function orb(size, { square = false } = {}) {
  const c = size / 2;
  return png(size, (x, y) => {
    const nx = (x - c) / c, ny = (y - c) / c;
    let alpha;
    if (square) { // macOS-style rounded square (superellipse)
      const r = Math.abs(nx) ** 5 + Math.abs(ny) ** 5;
      alpha = clamp01((0.84 ** 5 - r) * size * 0.6);
    } else {
      alpha = clamp01((1 - Math.hypot(nx, ny)) * size * 0.5);
    }
    let [r, g, b] = gradient((nx + ny + 2) / 4);
    const d = Math.hypot(nx + 0.22, ny + 0.22);
    const spark = clamp01(1 - d / 0.2) ** 1.5;
    r = lerp(r, 255, spark); g = lerp(g, 255, spark); b = lerp(b, 255, spark);
    return [r, g, b, 255 * alpha];
  });
}

// Template image for the menu bar: black ring + dot on transparent (macOS tints it).
function trayTemplate(size) {
  const c = size / 2;
  return png(size, (x, y) => {
    const d = Math.hypot(x - c, y - c) / c;
    const ring = clamp01((0.12 - Math.abs(d - 0.72)) * size * 0.5);
    const dot = clamp01((0.28 - d) * size * 0.5);
    return [0, 0, 0, 255 * Math.max(ring, dot)];
  });
}

// ---------------------------------------------------------------- generate

function have(cmd) {
  try { execFileSync("which", [cmd], { stdio: "ignore" }); return true; } catch { return false; }
}

function write(rel, buf) {
  const p = join(ROOT, rel);
  mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, buf);
}

const FONT_SRC = {
  "fonts/Barlow-Regular.ttf": "ofl/barlow/Barlow-Regular.ttf",
  "fonts/Barlow-Medium.ttf": "ofl/barlow/Barlow-Medium.ttf",
  "fonts/Barlow-SemiBold.ttf": "ofl/barlow/Barlow-SemiBold.ttf",
  "fonts/PatrickHand.ttf": "ofl/patrickhand/PatrickHand-Regular.ttf",
  "fonts/OFL-Barlow.txt": "ofl/barlow/OFL.txt",
  "fonts/OFL-PatrickHand.txt": "ofl/patrickhand/OFL.txt",
};

async function makeFile(rel, slot) {
  if (rel.startsWith("sounds/")) {
    const samples = soundFor(slot, variantOf(rel));
    if (rel.endsWith(".wav")) return write(rel, wav(samples));
    if (rel.endsWith(".mp3")) {
      const tmp = join(tmpdir(), `openclicky-${process.pid}.wav`);
      writeFileSync(tmp, wav(samples));
      try {
        if (have("ffmpeg")) execFileSync("ffmpeg", ["-loglevel", "error", "-y", "-i", tmp, "-b:a", "128k", join(ROOT, rel)]);
        else if (have("lame")) execFileSync("lame", ["--quiet", tmp, join(ROOT, rel)]);
        else return console.warn(`  ! skip ${rel}: needs ffmpeg or lame`);
      } finally { rmSync(tmp, { force: true }); }
      return;
    }
  }
  if (rel === "icons/app-icon-1024.png") return write(rel, orb(1024, { square: true }));
  if (rel === "icons/tray-template.png") return write(rel, trayTemplate(44));
  const ext = rel.match(/^icons\/ext-(\d+)\.png$/);
  if (ext) return write(rel, orb(Number(ext[1])));
  if (rel.startsWith("videos/")) {
    if (!have("ffmpeg")) return console.warn(`  ! skip ${rel}: needs ffmpeg`);
    mkdirSync(dirname(join(ROOT, rel)), { recursive: true });
    const args = ["-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=0x16141c:s=1280x720:d=2:r=30",
      "-pix_fmt", "yuv420p", "-movflags", "+faststart"];
    try { execFileSync("ffmpeg", [...args, "-c:v", "libx264", join(ROOT, rel)]); }
    catch { execFileSync("ffmpeg", [...args, join(ROOT, rel)]); }
    return;
  }
  if (FONT_SRC[rel]) {
    const url = `https://raw.githubusercontent.com/google/fonts/main/${FONT_SRC[rel]}`;
    try {
      const res = await fetch(url);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return write(rel, Buffer.from(await res.arrayBuffer()));
    } catch (e) {
      return console.warn(`  ! skip ${rel}: ${e.message} (UI falls back to system fonts)`);
    }
  }
  console.warn(`  ! no generator for ${rel}`);
}

if (process.argv.includes("--manifest") && !existsSync(MANIFEST)) {
  mkdirSync(ROOT, { recursive: true });
  writeFileSync(MANIFEST, JSON.stringify(buildManifest(), null, 2) + "\n");
  console.log(`wrote ${MANIFEST}`);
}

const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));
let made = 0;
for (const kind of ["sounds", "icons", "videos", "fonts"]) {
  for (const [slot, entry] of Object.entries(manifest[kind] ?? {})) {
    for (const rel of entry.files) {
      if (existsSync(join(ROOT, rel))) continue;
      await makeFile(rel, slot);
      if (existsSync(join(ROOT, rel))) { made++; console.log(`  + ${rel}`); }
    }
  }
}
for (const lic of ["fonts/OFL-Barlow.txt", "fonts/OFL-PatrickHand.txt"]) {
  if (!existsSync(join(ROOT, lic))) await makeFile(lic, "");
}
console.log(`placeholders: ${made} file(s) generated; existing files left untouched.`);
