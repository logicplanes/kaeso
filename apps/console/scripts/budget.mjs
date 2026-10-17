import { readFileSync, existsSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

export const BUDGETS = [
  { name: "sign-in", route: "src/routes/sign-in.tsx", target: 120, ceiling: 126 },
  { name: "project overview", route: "src/routes/_app/orgs/$org/projects/$project/index.tsx", also: ["src/auth/route-gates.ts"], target: 180, ceiling: 190 },
];

export function checkBudgets({ root = ROOT, dist = join(root, "dist"), budgets = BUDGETS } = {}) {
  const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8"));
  const notes = [];
  const problems = [];

  const routeChain = (route) => {
    const parts = route.replace(/^src\/routes\//, "").split("/");
    const chain = ["src/routes/__root.tsx"];
    for (let i = 1; i < parts.length; i++) {
      const dir = `src/routes/${parts.slice(0, i).join("/")}`;
      chain.push(`${dir}.tsx`, `${dir}/route.tsx`);
    }
    return [...chain, route].filter((f) => existsSync(join(root, f)));
  };
  const closure = (key, seen = new Set()) => {
    if (seen.has(key) || !manifest[key]) return seen;
    seen.add(key);
    for (const dep of manifest[key].imports ?? []) closure(dep, seen);
    return seen;
  };
  const gz = (file) => gzipSync(readFileSync(join(dist, file))).length;
  const sources = (file) => (existsSync(join(dist, `${file}.map`)) ? JSON.parse(readFileSync(join(dist, `${file}.map`), "utf8")).sources : []);
  const chunkOf = (src) => (manifest[src] ? src : Object.keys(manifest).find((k) => sources(manifest[k].file).some((s) => s === src || s.endsWith(`/${src}`))));
  const entry = Object.keys(manifest).find((k) => manifest[k].isEntry);
  const initial = closure(entry);

  const pageCode = [...initial]
    .map((k) => manifest[k]?.file)
    .filter((f) => f?.endsWith(".js"))
    .flatMap((f) => sources(f).filter((s) => /(^|\/)src\/features\//.test(s)));
  if (pageCode.length) problems.push(`budget: the entry, which every page loads first, holds page code: ${pageCode.join(", ")}`);

  for (const b of budgets) {
    if (!existsSync(join(root, b.route))) {
      problems.push(`budget: ${b.name}: ${b.route} doesn't exist; point the budget at the route's file`);
      continue;
    }
    const routeKeys = routeChain(b.route).flatMap((r) => Object.keys(manifest).filter((k) => k === r || k.startsWith(`${r}?`)));
    if (!routeKeys.some((k) => k === b.route || k.startsWith(`${b.route}?`))) {
      problems.push(`budget: ${b.name}: ${b.route} isn't in the build`);
      continue;
    }
    const also = (b.also ?? []).map((src) => ({ src, key: chunkOf(src) }));
    const missing = also.filter((a) => !a.key).map((a) => a.src);
    if (missing.length) {
      problems.push(`budget: ${b.name}: ${missing.join(", ")} isn't in the build`);
      continue;
    }
    const keys = new Set(initial);
    for (const k of [...routeKeys, ...also.map((a) => a.key)]) closure(k, keys);
    const bytes = [...keys]
      .map((k) => manifest[k]?.file)
      .filter((f) => f?.endsWith(".js"))
      .reduce((sum, f) => sum + gz(f), 0);
    const kb = bytes / 1024;
    const target = kb <= b.target ? "within the target" : `target ${b.target} kB not met`;
    const line = `budget: ${b.name}: ${kb.toFixed(1)} kB gzipped JS; ceiling ${b.ceiling} kB ${kb <= b.ceiling ? "ok" : "EXCEEDED"}; ${target}`;
    (kb <= b.ceiling ? notes : problems).push(line);
  }
  return { notes, problems };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const { notes, problems } = checkBudgets();
  for (const n of notes) console.log(n);
  for (const p of problems) console.error(p);
  if (problems.length) process.exit(1);
}
