import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { add, gatewayAlias, summary } from "./lcov.mjs";

const report = (path, lines) => `TN:\nSF:${path}\n${lines.map(([n, hits]) => `DA:${n},${hits}`).join("\n")}\nLF:${lines.length}\nend_of_record\n`;

test("a line any shard ran counts as covered once", () => {
  const files = new Map();
  add(
    files,
    report("/src/a.rs", [
      [1, 1],
      [2, 0],
      [3, 0],
    ]),
  );
  add(
    files,
    report("/src/a.rs", [
      [1, 0],
      [2, 4],
      [3, 0],
    ]),
  );
  assert.deepEqual(summary(files), { total: 3, covered: 2, percent: (2 / 3) * 100 });
});

test("files only one shard loaded still count", () => {
  const files = new Map();
  add(files, report("/src/a.rs", [[1, 1]]));
  add(
    files,
    report("/src/b.rs", [
      [1, 0],
      [2, 0],
    ]),
  );
  assert.equal(summary(files).total, 3);
  assert.equal(summary(files).covered, 1);
});

test("nothing to count is fully covered", () => {
  assert.equal(summary(new Map()).percent, 100);
});

test("the shards read the minimum and the ignored files from the coverage-gateway alias", () => {
  const alias = gatewayAlias(readFileSync(new URL("../.cargo/config.toml", import.meta.url), "utf8"));
  assert.ok(alias.minimum >= 91);
  assert.ok(new RegExp(alias.ignore).test("/crates/kaeso-testkit/src/oidc.rs"));
  assert.ok(!new RegExp(alias.ignore).test("/crates/kaeso-server/src/lib.rs"));
});

test("a config without the alias is refused", () => {
  assert.throws(() => gatewayAlias("[alias]\n"), /no coverage-gateway alias/);
});
