#!/usr/bin/env node
// Writes THIRD_PARTY_NOTICES.md: the licence of everything Pixie ships with, as those licences ask
// (MIT, BSD, ISC and Apache need their copyright and licence text to go with every copy).
//
// - JavaScript: the app's dependencies and theirs (not build tools), from node_modules.
// - Rust: every crate the desktop app links (normal dependencies, all platforms), from
//   `cargo metadata` and the crates' own licence files.
// - Fonts: Pixie's own (resources/fonts) and the ones Excalidraw bundles.
// - Downloaded on request (not shipped): sherpa-onnx and the Kokoro voice.
//
// Same licence text, different copyright lines: the text is printed once per group, with every
// package's own copyright lines listed above it. Run `npm run notices` after changing dependencies.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const LICENSE_FILE = /^(licen[cs]e|copying|unlicense)([-._].*)?$/i;
const NOTICE_FILE = /^notice([-._].*)?$/i;

const read = (p) => fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n").trim();
const files = (dir, re) => (fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => re.test(f) && fs.statSync(path.join(dir, f)).isFile()).sort() : []);

/// The licence file to quote: for "MIT OR Apache-2.0" and the like, the MIT one.
function pickLicense(dir, spdx) {
  const found = files(dir, LICENSE_FILE);
  if (!found.length) return null;
  const pref = /\bMIT\b/.test(spdx ?? "") ? found.find((f) => /mit/i.test(f)) : null;
  return read(path.join(dir, pref ?? found[0]));
}

const isCopyright = (l) => /^\s*(copyright\b|\(c\)|©)/i.test(l) || /\bcopyright \(c\)|copyright ©|copyright \d{4}/i.test(l);
function split(text) {
  const lines = text.split("\n");
  const copyright = lines.filter(isCopyright).map((l) => l.trim());
  const body = lines.filter((l) => !isCopyright(l)).join("\n").replace(/\n{3,}/g, "\n\n").trim();
  return { copyright, body, key: body.replace(/\s+/g, " ").toLowerCase() };
}

const packages = [];
const add = (p) => packages.push(p);

// ---- JavaScript ---------------------------------------------------------------------------------
function resolveNode(name, fromDir) {
  let dir = fromDir;
  for (;;) {
    const cand = path.join(dir, "node_modules", name, "package.json");
    if (fs.existsSync(cand)) return path.dirname(cand);
    const up = path.dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}
const seenJs = new Set();
function walkJs(dir) {
  const pkg = JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8"));
  const deps = { ...pkg.dependencies, ...pkg.optionalDependencies };
  for (const name of Object.keys(deps)) {
    const d = resolveNode(name, dir);
    if (!d) continue;
    const real = fs.realpathSync(d);
    if (seenJs.has(real)) continue;
    seenJs.add(real);
    const p = JSON.parse(fs.readFileSync(path.join(d, "package.json"), "utf8"));
    const spdx = typeof p.license === "string" ? p.license : p.license?.type ?? (p.licenses ?? []).map((l) => l.type).join(" OR ");
    const author = typeof p.author === "string" ? p.author : p.author?.name;
    add({ kind: "npm", name: p.name, version: p.version, spdx, author, text: pickLicense(d, spdx), notices: files(d, NOTICE_FILE).map((f) => read(path.join(d, f))) });
    walkJs(d);
  }
}
walkJs(path.join(repo, "apps/desktop"));
// Excalidraw's npm package carries no licence file; this is the one in its repository.
const excal = packages.find((p) => p.name === "@excalidraw/excalidraw");
if (excal && !excal.text) {
  excal.text = `MIT License

Copyright (c) 2020 Excalidraw

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;
}

// ---- Rust ---------------------------------------------------------------------------------------
const meta = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], { cwd: repo, maxBuffer: 256 << 20, encoding: "utf8" }));
const byId = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const root = meta.packages.find((p) => p.name === "pixie-desktop");
const linked = new Set();
const stack = [root.id];
while (stack.length) {
  const id = stack.pop();
  if (linked.has(id)) continue;
  linked.add(id);
  for (const d of nodes.get(id)?.deps ?? []) {
    // Build scripts and tests don't ship; only what's linked into the app.
    if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
  }
}
for (const id of linked) {
  const p = byId.get(id);
  if (!p.source) continue; // Pixie's own crates
  const dir = path.dirname(p.manifest_path);
  add({ kind: "crate", name: p.name, version: p.version, spdx: p.license ?? (p.license_file ? "see its licence file" : ""), author: (p.authors ?? [])[0], text: pickLicense(dir, p.license), notices: files(dir, NOTICE_FILE).map((f) => read(path.join(dir, f))) });
}

// ---- Write --------------------------------------------------------------------------------------
// The same package can sit in node_modules more than once.
const unique = [...new Map(packages.map((p) => [`${p.kind}:${p.name}@${p.version}`, p])).values()];
packages.length = 0;
packages.push(...unique);
const groups = new Map();
const bare = [];
for (const p of packages) {
  if (!p.text) {
    bare.push(p);
    continue;
  }
  const s = split(p.text);
  if (!groups.has(s.key)) groups.set(s.key, { body: s.body, members: [] });
  groups.get(s.key).members.push({ ...p, copyright: s.copyright });
}
// A group is named after the licence most of its packages declare.
const title = (members) => {
  const count = new Map();
  for (const m of members) count.set(m.spdx || "Licence", (count.get(m.spdx || "Licence") ?? 0) + 1);
  return [...count.entries()].sort((a, b) => b[1] - a[1])[0][0];
};
const label = (p) => `${p.name} ${p.version} (${p.kind === "npm" ? "npm" : "crates.io"}${p.spdx ? `, ${p.spdx}` : ""})`;
const sorted = [...groups.values()].sort((a, b) => b.members.length - a.members.length);

const fontDir = path.join(repo, "resources/fonts");
const pixieFonts = fs.readdirSync(fontDir).filter((f) => /^OFL-.*\.txt$/.test(f)).sort().map((f) => `  - ${read(path.join(fontDir, f)).split("\n").find(isCopyright).trim()}`).join("\n");
const ofl = read(path.join(fontDir, "OFL-vt323.txt"));
const oflBody = ofl.slice(ofl.search(/SIL OPEN FONT LICENSE/i));
let out = `# Third-party notices

Pixie is MIT-licensed (see [LICENSE](LICENSE)). It's built on the open-source work below, whose
licences ask for their copyright and licence text to go with every copy of Pixie. Thank you to
everyone who made them.

Generated by \`scripts/notices.mjs\` from the dependencies Pixie ships with: ${packages.filter((p) => p.kind === "npm").length} JavaScript
packages and ${packages.filter((p) => p.kind === "crate").length} Rust crates.

## Highlights

- **Excalidraw** (MIT, Copyright (c) 2020 Excalidraw): the canvas Pixie draws on. https://github.com/excalidraw/excalidraw
- **Mermaid** (MIT) and **@excalidraw/mermaid-to-excalidraw** (MIT): flowcharts laid out for the canvas.
- **three.js** (MIT): the 3D explanations.
- **React** (MIT) and **Tauri** (MIT or Apache-2.0): the app itself.
- **sherpa-onnx** (Apache-2.0, k2-fsa) and the **Kokoro** voice model (Apache-2.0, hexgrad): the
  natural voice. Pixie doesn't ship them: it downloads them from sherpa-onnx's GitHub releases
  when you ask for the natural voice (Settings → Voice). Pixie's voice server includes
  sherpa-onnx's C API header (Apache-2.0).
- The **streaming Zipformer** English speech model (Apache-2.0, k2-fsa icefall, packaged by
  csukuangfj): Pixie's own speech to text. Not shipped either: downloaded from Hugging Face when
  you ask for it (Settings → Voice), and run through sherpa-onnx.

## Fonts

Under the SIL Open Font License 1.1 (full text below):

- Pixie's own, with their copyright lines:
${pixieFonts}
  Their licence files are in \`resources/fonts/\`.
- Bundled by Excalidraw for the canvas: Excalifont, Virgil, Cascadia Code, Liberation Sans, Lilita
  One, Nunito, Xiaolai and Assistant (SIL OFL 1.1), and Comic Shanns (MIT). See
  https://github.com/excalidraw/excalidraw for each font's copyright holders.

<details><summary>SIL Open Font License 1.1</summary>

\`\`\`text
${oflBody}
\`\`\`

</details>

## Licences, grouped

`;
for (const g of sorted) {
  out += `### ${title(g.members)} (${g.members.length} ${g.members.length === 1 ? "package" : "packages"})\n\n`;
  for (const m of g.members.sort((a, b) => a.name.localeCompare(b.name))) {
    out += `- ${label(m)}${m.copyright.length ? `: ${[...new Set(m.copyright)].join("; ")}` : m.author ? `: by ${m.author}` : ""}\n`;
  }
  out += `\n<details><summary>Licence text</summary>\n\n\`\`\`text\n${g.body}\n\`\`\`\n\n</details>\n\n`;
}
const withNotices = packages.filter((p) => p.notices.length);
if (withNotices.length) {
  out += `## NOTICE files\n\n`;
  for (const p of withNotices) out += `### ${p.name} ${p.version}\n\n\`\`\`text\n${p.notices.join("\n\n")}\n\`\`\`\n\n`;
}
if (bare.length) {
  out += `## Packages without a licence file\n\nThese name their licence in their metadata but ship no licence text.\n\n`;
  for (const p of bare.sort((a, b) => a.name.localeCompare(b.name))) out += `- ${label(p)}${p.spdx ? "" : ": licence not stated"}${p.author ? `, by ${p.author}` : ""}\n`;
  const mit = [...groups.values()].find((g) => g.members.some((m) => m.spdx === "MIT"));
  if (mit) out += `\nThe MIT licence, for those that name it:\n\n\`\`\`text\n${mit.body}\n\`\`\`\n`;
  out += "\n";
}
fs.writeFileSync(path.join(repo, "THIRD_PARTY_NOTICES.md"), out.replace(/—/g, ":"));
console.log(`THIRD_PARTY_NOTICES.md: ${packages.length} packages in ${groups.size} licence groups, ${bare.length} without a licence file, ${(out.length / 1024).toFixed(0)} KB`);
