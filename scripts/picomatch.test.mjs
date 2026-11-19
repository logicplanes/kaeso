import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pnpmStore = join(root, "node_modules", ".pnpm");
const requireFromHere = createRequire(import.meta.url);

const installedPicomatch = () =>
  readdirSync(pnpmStore)
    .filter((entry) => entry.startsWith("picomatch@"))
    .sort()
    .map((entry) => ({
      entry,
      pm: requireFromHere(join(pnpmStore, entry, "node_modules", "picomatch")),
    }));

test("every installed picomatch copy literalizes single-branch repeated extglobs that nest unbounded wildcards (GHSA-fc5q-39xc-vjpf)", () => {
  const copies = installedPicomatch();
  assert.ok(copies.length >= 2, `expected both picomatch copies, found ${copies.length}`);

  const riskyPatterns = ["+(*)", "*(*)", "+(**)", "+(*a)", "*(*a)", "+(a*)", "+(a*b)", "+(?*a)", "+(*?a)", "+(foo|*a)", "+()", "+(*(a)a)"];
  const probes = [`${"a".repeat(42)}!`, `${"a".repeat(42)}/`, `${"ab".repeat(30)}c`];

  for (const { entry, pm } of copies) {
    for (const pattern of riskyPatterns) {
      for (const input of probes) {
        const start = performance.now();
        assert.equal(pm.isMatch(input, pattern), false, `${entry} matched ${pattern}`);
        const elapsed = performance.now() - start;
        assert.ok(elapsed < 200, `${entry} backtracked on ${pattern} (${elapsed.toFixed(1)} ms)`);
      }
    }

    assert.equal(pm.isMatch("abcabc", "+(abc)"), true, entry);
    assert.equal(pm.isMatch("foobar", "*(foo|bar)"), true, entry);
    assert.equal(pm.isMatch("index.tsx", "*.+(ts|tsx)"), true, entry);
    assert.equal(pm.isMatch("aaa", "+(*(a))"), true, entry);
    assert.equal(pm.isMatch("***", "+(\\*)"), true, entry);
    assert.equal(pm.isMatch("***", "+([*])"), true, entry);
  }
});
