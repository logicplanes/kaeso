import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { composeChanged, manifest, retag, sha256 } from "./release-manifest.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const digest = (c) => `sha256:${c.repeat(64)}`;
const digests = { gateway: digest("a"), console: digest("b"), updater: digest("c") };
const commit = "5388ad0b98aa3d957fea99e200259842bf746680";
const compose = (v, extra = "") =>
  `services:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:${v}\n  console:\n    image: ghcr.io/logicplanes/kaeso-console:${v}\n  caddy:\n    image: caddy:2${extra}\n`;

test("the manifest names every image by digest and the compose file by hash", () => {
  const file = Buffer.from(compose("3.4.6"));
  const m = manifest({ version: "3.4.6", commit, digests, compose: file, previousCompose: compose("3.4.5") });
  assert.deepEqual(m, {
    schema: 1,
    version: "3.4.6",
    commit,
    manual: false,
    updater_protocol: 1,
    images: {
      gateway: { name: "ghcr.io/logicplanes/kaeso-gateway", digest: digest("a") },
      console: { name: "ghcr.io/logicplanes/kaeso-console", digest: digest("b") },
      updater: { name: "ghcr.io/logicplanes/kaeso-updater", digest: digest("c") },
    },
    compose: { name: "compose.yaml", sha256: sha256(file) },
  });
});

test("a compose file that changes beyond the image versions, or a release marked so, needs steps by hand", () => {
  assert.equal(composeChanged(compose("3.4.5"), compose("3.4.6"), "3.4.6"), false);
  assert.equal(composeChanged(compose("3.4.5"), compose("3.4.6", "\n  extra: {}"), "3.4.6"), true);
  assert.equal(composeChanged(null, compose("3.4.6"), "3.4.6"), false);
  const changed = manifest({ version: "3.4.6", commit, digests, compose: Buffer.from(compose("3.4.6", "x")), previousCompose: compose("3.4.5") });
  assert.equal(changed.manual, true);
  const marked = manifest({ version: "3.4.6", commit, digests, compose: Buffer.from(compose("3.4.6")), manual: true });
  assert.equal(marked.manual, true);
  assert.equal(retag(compose("3.4.5"), "3.5.0-rc.1"), compose("3.5.0-rc.1"));
});

test("missing digests, commits and versions are refused", () => {
  const file = Buffer.from(compose("3.4.6"));
  assert.throws(() => manifest({ version: "3.4", commit, digests, compose: file }), /not a version/);
  assert.throws(() => manifest({ version: "3.4.6", commit: "main", digests, compose: file }), /not a commit/);
  assert.throws(() => manifest({ version: "3.4.6", commit, digests: { ...digests, updater: "latest" }, compose: file }), /updater image has no digest/);
});

test("the command line writes the manifest from the digest files", () => {
  const dir = mkdtempSync(join(tmpdir(), "release-manifest-"));
  try {
    for (const [service, value] of Object.entries(digests)) writeFileSync(join(dir, service), `${value}\n`);
    writeFileSync(join(dir, "compose.yaml"), compose("3.4.6"));
    writeFileSync(join(dir, "previous.yaml"), compose("3.4.5", "y"));
    const out = execFileSync(
      process.execPath,
      [
        join(here, "release-manifest.mjs"),
        "--version",
        "3.4.6",
        "--commit",
        commit,
        "--digests",
        dir,
        "--compose",
        join(dir, "compose.yaml"),
        "--previous",
        join(dir, "previous.yaml"),
        "--manual",
        "false",
      ],
      { encoding: "utf8" },
    );
    const m = JSON.parse(out);
    assert.equal(m.images.console.digest, digest("b"));
    assert.equal(m.manual, true);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
