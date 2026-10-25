import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join, matchesGlob } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { LABELS } from "./changelog.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const rules = () => {
  const parsed = [];
  for (const line of readFileSync(join(root, ".github", "labeler.yml"), "utf8").split("\n")) {
    const label = /^"?([^"\s][^"]*?)"?:$/.exec(line)?.[1];
    const mode = /^\s+- (any-glob-to-any-file|any-glob-to-all-files):$/.exec(line)?.[1];
    const glob = /^\s+- "?([^"]+?)"?$/.exec(line)?.[1];
    if (label) parsed.push({ label, mode: null, globs: [] });
    else if (mode) parsed.at(-1).mode = mode;
    else if (glob && parsed.at(-1)?.mode) parsed.at(-1).globs.push(glob);
  }
  return parsed;
};

const labelsFor = (files) =>
  rules()
    .filter(({ mode, globs }) =>
      mode === "any-glob-to-all-files"
        ? globs.some((glob) => files.every((file) => matchesGlob(file, glob)))
        : globs.some((glob) => files.some((file) => matchesGlob(file, glob))),
    )
    .map(({ label }) => label)
    .sort();

test("the labeler sets only labels the changelog knows, and one rule for each area and for internal", () => {
  const known = new Set(LABELS.map(({ name }) => name));
  const labelled = rules().map(({ label }) => label);
  for (const label of labelled) assert.ok(known.has(label), label);
  assert.deepEqual(
    labelled.toSorted(),
    LABELS.filter(({ name }) => name === "internal" || name.startsWith("area: "))
      .map(({ name }) => name)
      .toSorted(),
  );
  for (const { label, mode, globs } of rules()) assert.ok(mode && globs.length, label);
});

test("a pull request gets the area of every part it changes", () => {
  assert.deepEqual(labelsFor(["crates/kaeso-server/src/routes.rs", "openapi.json"]), ["area: gateway"]);
  assert.deepEqual(labelsFor(["crates/kaeso-codecs/src/lib.rs"]), ["area: kpp"]);
  assert.deepEqual(labelsFor(["plugins/kaeso/.mcp.json", ".claude-plugin/marketplace.json", ".cursor-plugin/marketplace.json"]), ["area: gateway"]);
  assert.deepEqual(labelsFor(["apps/console/src/routes/keys.tsx", "packages/ui/src/dialog.tsx"]), ["area: console", "area: ui library"]);
  assert.deepEqual(labelsFor(["apps/docs/content/en/index.md"]), ["area: documentation"]);
  assert.deepEqual(labelsFor(["deploy/compose.yaml"]), ["area: self-hosting"]);
  assert.deepEqual(labelsFor(["crates/kaeso-update/src/policy.rs"]), ["area: gateway"]);
  assert.deepEqual(labelsFor(["crates/kaeso-updater/src/engine.rs", "deploy/updater.Dockerfile"]), ["area: self-hosting"]);
  assert.deepEqual(labelsFor(["Cargo.lock", "pnpm-lock.yaml"]), []);
});

test("a change to the code of any crate gets a label", () => {
  for (const crate of readdirSync(join(root, "crates"), { withFileTypes: true }).filter((entry) => entry.isDirectory())) {
    assert.notDeepEqual(labelsFor([`crates/${crate.name}/src/lib.rs`]), [], crate.name);
  }
});

test("every file of the self-hosting package gets its area", () => {
  const shipped = execFileSync("git", ["ls-files", "deploy"], { cwd: root, encoding: "utf8" })
    .split("\n")
    .filter((file) => file && file !== "deploy/compose.dev.yaml");
  assert.ok(shipped.includes("deploy/.env.example"));
  for (const file of shipped) assert.deepEqual(labelsFor([file]), ["area: self-hosting"], file);
});

test("a pull request that changes only CI, tests, tooling or contributor guides is internal", () => {
  assert.deepEqual(labelsFor([".github/workflows/ci.yml", ".github/labeler.yml"]), ["internal"]);
  assert.deepEqual(labelsFor([".github/labeler.yml", "scripts/labeler.test.mjs", "rustfmt.toml"]), ["internal"]);
  assert.deepEqual(labelsFor(["scripts/changelog.mjs", "scripts/changelog.test.mjs", "AGENTS.md", "apps/docs/AGENTS.md"]), ["area: documentation", "internal"]);
  assert.deepEqual(labelsFor(["apps/console/src/keys.test.tsx", "apps/console/e2e/keys.spec.ts"]), ["area: console", "internal"]);
  assert.deepEqual(labelsFor(["crates/kaeso-codecs/tests/round_trip.rs", "fuzz/fuzz_targets/decode.rs"]), ["area: kpp", "internal"]);
  assert.deepEqual(labelsFor(["e2e/requirements.txt", "deploy/compose.dev.yaml"]), ["internal"]);
});

test("a pull request that changes what people use is not internal, even with tests beside it", () => {
  assert.deepEqual(labelsFor(["apps/console/src/keys.tsx", "apps/console/src/keys.test.tsx"]), ["area: console"]);
  assert.deepEqual(labelsFor(["crates/kaeso-core/src/limits.rs", "crates/kaeso-core/tests/limits.rs", ".github/workflows/ci.yml"]), ["area: gateway"]);
});

test("every label has a colour and a description GitHub accepts", () => {
  for (const { name, color, description } of LABELS) {
    assert.match(color, /^[0-9a-f]{6}$/, name);
    assert.ok(description.length > 0 && description.length <= 100, name);
    assert.ok(name.length <= 50, name);
  }
  assert.equal(new Set(LABELS.map(({ name }) => name)).size, LABELS.length);
});
