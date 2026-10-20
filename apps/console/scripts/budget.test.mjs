import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { checkBudgets } from "./budget.mjs";

let made = [];
afterEach(() => {
  for (const dir of made) rmSync(dir, { recursive: true, force: true });
  made = [];
});

function build({ routes = ["src/routes/page.tsx"], manifest, maps = {}, contents = {} }) {
  const root = mkdtempSync(join(tmpdir(), "kaeso-budget-"));
  made.push(root);
  for (const route of routes) {
    mkdirSync(join(root, route, ".."), { recursive: true });
    writeFileSync(join(root, route), "");
  }
  const dist = join(root, "dist");
  mkdirSync(join(dist, ".vite"), { recursive: true });
  mkdirSync(join(dist, "assets"), { recursive: true });
  writeFileSync(join(dist, ".vite/manifest.json"), JSON.stringify(manifest));
  for (const { file } of Object.values(manifest)) writeFileSync(join(dist, file), contents[file] ?? "x".repeat(200));
  for (const [file, sources] of Object.entries(maps)) writeFileSync(join(dist, `${file}.map`), JSON.stringify({ sources }));
  return { root, dist };
}

const manifest = {
  "index.html": { file: "assets/index.js", isEntry: true, imports: [] },
  "src/routes/page.tsx?tsr-split=component": { file: "assets/page.js", imports: ["_gates.js"] },
  "_gates.js": { file: "assets/gates.js" },
};
const budget = { name: "page", route: "src/routes/page.tsx", also: ["src/auth/gates.ts"], target: 1, ceiling: 2 };

describe("the bundle budgets", () => {
  it("measure a route with the chunk its extra source lives in", () => {
    const { root, dist } = build({ manifest, maps: { "assets/gates.js": ["../../src/auth/gates.ts"] } });
    expect(checkBudgets({ root, dist, budgets: [budget] })).toEqual({
      notes: ["budget: page: 0.1 kB gzipped JS; ceiling 2 kB ok; within the target"],
      problems: [],
    });
  });

  it("measure the same size whatever hashes the chunk names carry", () => {
    const measured = (hash) => {
      const chunks = Object.fromEntries(Array.from({ length: 300 }, (_, i) => [`_c${i}.js`, { file: `assets/c${i}-${hash(i)}.js` }]));
      const page = Object.values(chunks)
        .map(({ file }) => `import"./${file.slice("assets/".length)}";`)
        .join("");
      const built = build({
        manifest: { ...manifest, ...chunks },
        contents: { "assets/page.js": page },
        maps: { "assets/gates.js": ["../../src/auth/gates.ts"] },
      });
      return checkBudgets({ ...built, budgets: [budget] }).notes;
    };
    const plain = (i) => String(i).padStart(7, "0");
    const random = (i) => createHash("sha256").update(String(i)).digest("base64url").slice(0, 7);
    expect(measured(random)).toEqual(measured(plain));
  });

  it("fail when a budgeted route was renamed or deleted", () => {
    const { root, dist } = build({ routes: [], manifest });
    expect(checkBudgets({ root, dist, budgets: [budget] }).problems).toEqual([
      "budget: page: src/routes/page.tsx doesn't exist; point the budget at the route's file",
    ]);
  });

  it("fail when a budgeted route or its extra source isn't in the build", () => {
    const { root, dist } = build({ manifest: { "index.html": manifest["index.html"] } });
    expect(checkBudgets({ root, dist, budgets: [budget] }).problems).toEqual(["budget: page: src/routes/page.tsx isn't in the build"]);
    const built = build({ manifest });
    expect(checkBudgets({ ...built, budgets: [budget] }).problems).toEqual(["budget: page: src/auth/gates.ts isn't in the build"]);
  });

  it("fail when a route goes over its ceiling or the entry holds page code", () => {
    const { root, dist } = build({ manifest, maps: { "assets/index.js": ["../../src/features/home.tsx"], "assets/gates.js": ["../../src/auth/gates.ts"] } });
    expect(checkBudgets({ root, dist, budgets: [{ ...budget, target: 0.01, ceiling: 0.01 }] }).problems).toEqual([
      "budget: the entry, which every page loads first, holds page code: ../../src/features/home.tsx",
      "budget: page: 0.1 kB gzipped JS; ceiling 0.01 kB EXCEEDED; target 0.01 kB not met",
    ]);
  });
});
