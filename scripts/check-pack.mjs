import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, resolve, posix } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const arg = (name) => {
  const i = process.argv.indexOf(name);
  return i === -1 ? undefined : process.argv[i + 1];
};
const requestedOut = arg("--out");
const out = resolve(requestedOut ?? mkdtempSync(join(tmpdir(), "kaeso-pack-")));
const expected = arg("--version");

const REACT_LIBRARIES = /^(react|react-dom|@base-ui\/react|@hugeicons\/react|@tanstack\/react-[\w-]+|react-day-picker)(\/|$)/;
const failures = [];
const fail = (m) => failures.push(m);
const packages = [
  { dir: "packages/tokens", must: ["package.json", "LICENSE", "README.md"] },
  {
    dir: "packages/ui",
    must: [
      "package.json",
      "LICENSE",
      "NOTICE.md",
      "README.md",
      "AGENTS.md",
      "CLAUDE.md",
      "dist/fonts/LICENSE-Inter.txt",
      "dist/fonts/LICENSE-GeistMono.txt",
      "ai/component-registry.json",
      "ai/component-registry.md",
      "ai/component-guides.md",
      "ai/tokens.md",
      "ai/icons.md",
      "foundations/color.md",
    ],
  },
];

const versions = new Set();
const tarballs = [];
for (const p of packages) {
  const json = execFileSync("pnpm", ["pack", "--pack-destination", out, "--json"], { cwd: join(root, p.dir), encoding: "utf8" });
  const info = JSON.parse(json);
  const tarball = info.filename;
  tarballs.push(tarball);
  const files = execFileSync("tar", ["-tzf", tarball], { encoding: "utf8" })
    .trim()
    .split("\n")
    .map((f) => f.replace(/^package\//, ""));
  const manifest = JSON.parse(execFileSync("tar", ["-xOzf", tarball, "package/package.json"], { encoding: "utf8" }));
  versions.add(manifest.version);
  for (const f of p.must) if (!files.includes(f)) fail(`${manifest.name}: ${f} is missing`);
  const targets = [];
  const walk = (v) => (typeof v === "string" ? targets.push(v) : Object.values(v).forEach(walk));
  walk(manifest.exports);
  for (const t of targets) {
    const rel = t.replace(/^\.\//, "");
    const re = new RegExp(
      `^${rel
        .split("*")
        .map((x) => x.replace(/[.+?^${}()|[\]\\]/g, "\\$&"))
        .join("[^/]+")}$`,
    );
    if (!files.some((f) => re.test(f))) fail(`${manifest.name}: export target ${t} matches no packed file`);
  }
  for (const f of files)
    if (/^(src|test|scripts|node_modules)\//.test(f) || /\.(test|spec)\./.test(f) || /(^|\/)\.env/.test(f))
      fail(`${manifest.name}: ${f} must not be published`);
  const unpacked = mkdtempSync(join(tmpdir(), "kaeso-unpack-"));
  execFileSync("tar", ["-xzf", tarball, "-C", unpacked]);
  const packed = new Set(files);
  const modules = new Map();
  for (const f of files.filter((f) => /\.(js|d\.ts)$/.test(f))) {
    const code = readFileSync(join(unpacked, "package", f), "utf8");
    const module = { client: code.startsWith('"use client";'), react: false, deps: [] };
    for (const m of code.matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*)["']([^"']+)["']/g)) {
      if (REACT_LIBRARIES.test(m[1])) module.react = true;
      if (!m[1].startsWith(".")) continue;
      const target = posix.normalize(posix.join(posix.dirname(f), m[1]));
      const declared = f.endsWith(".d.ts") ? target.replace(/\.js$/, ".d.ts") : target;
      if (!packed.has(declared)) fail(`${manifest.name}: ${f} imports ${m[1]}, which Node does not resolve to a packed file`);
      else module.deps.push(declared);
    }
    if (f.endsWith(".js")) modules.set(f, module);
  }
  rmSync(unpacked, { recursive: true, force: true });
  const reachesReact = (f, seen = new Set()) => {
    if (seen.has(f)) return false;
    seen.add(f);
    const module = modules.get(f);
    return !!module && (module.react || module.deps.some((d) => reachesReact(d, seen)));
  };
  for (const [f, module] of modules) {
    const react = reachesReact(f);
    if (react && !module.client) fail(`${manifest.name}: ${f} renders React but does not start with "use client"`);
    if (!react && module.client) fail(`${manifest.name}: ${f} has no React but says "use client", so server components cannot call it`);
  }
  for (const [dep, range] of Object.entries({ ...manifest.dependencies, ...manifest.peerDependencies }))
    if (String(range).startsWith("workspace:")) fail(`${manifest.name}: ${dep} is still ${range}`);
  if (manifest.publishConfig?.access !== "public") fail(`${manifest.name}: publishConfig.access must be public`);
  if (manifest.repository?.url !== "git+https://github.com/logicplanes/kaeso.git" || manifest.repository?.directory !== p.dir)
    fail(`${manifest.name}: repository must point at logicplanes/kaeso, directory ${p.dir}`);
  if (manifest.publishConfig?.provenance !== true) fail(`${manifest.name}: publishConfig.provenance must be true`);
  if (manifest.license !== "MIT") fail(`${manifest.name}: license must be MIT`);
  const kb = (statSync(tarball).size / 1024).toFixed(0);
  console.log(`${manifest.name}@${manifest.version}: ${files.length} files, ${kb} kB packed`);
}
if (versions.size !== 1) fail(`versions differ: ${[...versions].join(", ")}`);
if (expected && !versions.has(expected)) fail(`tag says ${expected}, packages say ${[...versions].join(", ")}`);

if (!requestedOut) rmSync(out, { recursive: true, force: true });
if (failures.length) {
  for (const f of failures) console.error(`  ✗ ${f}`);
  process.exit(1);
}
console.log(requestedOut ? `✓ pack: ${tarballs.length} tarballs in ${out}` : `✓ pack: ${tarballs.length} tarballs checked`);
