import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (file) => readFileSync(join(root, file), "utf8");

const denyIgnores = () =>
  [...read("deny.toml").matchAll(/\{ id = "([^"]+)", reason = "([^"]+)" \}/g)].map(([, id, reason]) => ({
    id,
    reason,
  }));

const osvIgnores = () =>
  read("osv-scanner.toml")
    .split("[[IgnoredVulns]]")
    .slice(1)
    .map((entry) => ({ id: /^id = "([^"]+)"$/m.exec(entry)?.[1], reason: /^reason = "([^"]+)"$/m.exec(entry)?.[1] }));

test("cargo deny and the OSV scanner ignore the same advisories for the same reasons", () => {
  assert.ok(denyIgnores().length > 0);
  assert.deepEqual(osvIgnores(), denyIgnores());
});
