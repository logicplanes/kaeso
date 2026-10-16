import { readFileSync, existsSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8"));

const BUDGETS = [
  { name: "sign-in", route: "src/routes/sign-in.tsx", target: 120, ceiling: 130 },
  { name: "project overview", route: "src/routes/_app/orgs/$org/projects/$project/index.tsx", also: ["src/auth/route-gates.ts"], target: 180, ceiling: 238 },
];

function routeChain(route) {
  const parts = route.replace(/^src\/routes\//, "").split("/");
  const chain = ["src/routes/__root.tsx"];
  for (let i = 1; i < parts.length; i++) {
    const dir = `src/routes/${parts.slice(0, i).join("/")}`;
    chain.push(`${dir}.tsx`, `${dir}/route.tsx`);
  }
  return [...chain, route].filter((f) => existsSync(join(root, f)));
}

function closure(key, seen = new Set()) {
  if (seen.has(key) || !manifest[key]) return seen;
  seen.add(key);
  for (const dep of manifest[key].imports ?? []) closure(dep, seen);
  return seen;
}
const gz = (file) => gzipSync(readFileSync(join(dist, file))).length;
const entry = Object.keys(manifest).find((k) => manifest[k].isEntry);
const initial = closure(entry);

let failed = false;

const pageCode = [...initial]
  .map((k) => manifest[k]?.file)
  .filter((f) => f?.endsWith(".js") && existsSync(join(dist, `${f}.map`)))
  .flatMap((f) => JSON.parse(readFileSync(join(dist, `${f}.map`), "utf8")).sources.filter((s) => /(^|\/)src\/features\//.test(s)));
if (pageCode.length) {
  failed = true;
  console.log(`budget: the entry, which every page loads first, holds page code: ${pageCode.join(", ")}`);
}
for (const b of BUDGETS) {
  const routeKeys = existsSync(join(root, b.route))
    ? routeChain(b.route).flatMap((r) => Object.keys(manifest).filter((k) => k === r || k.startsWith(`${r}?`)))
    : [];
  if (routeKeys.length === 0) {
    console.log(`budget: ${b.name}: route not built yet, skipped`);
    continue;
  }
  const keys = new Set(initial);
  for (const k of [...routeKeys, ...(b.also ?? [])]) closure(k, keys);
  const bytes = [...keys]
    .map((k) => manifest[k]?.file)
    .filter((f) => f?.endsWith(".js"))
    .reduce((sum, f) => sum + gz(f), 0);
  const kb = bytes / 1024;
  const ok = kb <= b.ceiling;
  if (!ok) failed = true;
  const target = kb <= b.target ? "within the target" : `target ${b.target} kB not met`;
  console.log(`budget: ${b.name}: ${kb.toFixed(1)} kB gzipped JS; ceiling ${b.ceiling} kB ${ok ? "ok" : "EXCEEDED"}; ${target}`);
}
if (failed) process.exit(1);
