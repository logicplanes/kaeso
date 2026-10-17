import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { composeChanged, latestMigrations, manifest, manualSince, previousManifest, retag, sha256, sourceCommit } from "./release-manifest.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const digest = (c) => `sha256:${c.repeat(64)}`;
const digests = { gateway: digest("a"), console: digest("b"), updater: digest("c") };
const commit = "5388ad0b98aa3d957fea99e200259842bf746680";
const migrations = { postgres: "20261017120001_updates", clickhouse: "0002_usage_rollups" };
const compose = (v, extra = "") =>
  `services:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:${v}\n  console:\n    image: ghcr.io/logicplanes/kaeso-console:${v}\n  caddy:\n    image: caddy:2${extra}\n`;

test("the manifest names every image by digest, the compose file by hash and the latest migrations", () => {
  const file = Buffer.from(compose("3.4.6"));
  const m = manifest({ version: "3.4.6", commit, digests, compose: file, migrations, previousCompose: compose("3.4.5") });
  assert.deepEqual(m, {
    schema: 1,
    version: "3.4.6",
    commit,
    manual: false,
    manual_since: null,
    updater_protocol: 1,
    images: {
      gateway: { name: "ghcr.io/logicplanes/kaeso-gateway", digest: digest("a") },
      console: { name: "ghcr.io/logicplanes/kaeso-console", digest: digest("b") },
      updater: { name: "ghcr.io/logicplanes/kaeso-updater", digest: digest("c") },
    },
    compose: { name: "compose.yaml", sha256: sha256(file) },
    migrations,
  });
});

test("a compose file that changes beyond the image versions, or a release marked so, needs steps by hand", () => {
  assert.equal(composeChanged(compose("3.4.5"), compose("3.4.6"), "3.4.6"), false);
  assert.equal(composeChanged(compose("3.4.5"), compose("3.4.6", "\n  extra: {}"), "3.4.6"), true);
  assert.equal(composeChanged(null, compose("3.4.6"), "3.4.6"), false);
  const changed = manifest({ version: "3.4.6", commit, digests, migrations, compose: Buffer.from(compose("3.4.6", "x")), previousCompose: compose("3.4.5") });
  assert.equal(changed.manual, true);
  assert.equal(changed.manual_since, "3.4.6");
  const marked = manifest({ version: "3.4.6", commit, digests, migrations, compose: Buffer.from(compose("3.4.6")), manual: true });
  assert.equal(marked.manual, true);
  assert.equal(retag(compose("3.4.5"), "3.5.0-rc.1"), compose("3.5.0-rc.1"));
});

test("the last release with steps by hand carries on to every later release", () => {
  assert.equal(manualSince({ version: "3.4.7", manual: false, previous: { manual_since: "3.4.6" } }), "3.4.6");
  assert.equal(manualSince({ version: "3.4.7", manual: true, previous: { manual_since: "3.4.6" } }), "3.4.7");
  assert.equal(manualSince({ version: "3.4.7", manual: false, previous: null }), null);
  const m = manifest({ version: "3.4.7", commit, digests, migrations, compose: Buffer.from(compose("3.4.7")), previous: { manual_since: "3.4.6" } });
  assert.equal(m.manual, false);
  assert.equal(m.manual_since, "3.4.6");
});

test("missing digests, commits, migrations and versions are refused", () => {
  const file = Buffer.from(compose("3.4.6"));
  assert.throws(() => manifest({ version: "3.4", commit, digests, migrations, compose: file }), /not a version/);
  assert.throws(() => manifest({ version: "3.4.6", commit: "main", digests, migrations, compose: file }), /not a commit/);
  assert.throws(
    () => manifest({ version: "3.4.6", commit, digests: { ...digests, updater: "latest" }, migrations, compose: file }),
    /updater image has no digest/,
  );
  assert.throws(() => manifest({ version: "3.4.6", commit, digests, compose: file }), /latest migrations/);
});

function repository() {
  const dir = mkdtempSync(join(tmpdir(), "release-commit-"));
  const run = (...args) => execFileSync("git", args, { cwd: dir, encoding: "utf8" }).trim();
  run("init", "--quiet", "--initial-branch=prod");
  run("config", "user.email", "release@example.com");
  run("config", "user.name", "Release");
  run("config", "commit.gpgsign", "false");
  run("config", "tag.gpgsign", "false");
  run("commit", "--quiet", "--allow-empty", "--message", "Add a feature");
  return { dir, run, head: () => run("rev-parse", "HEAD") };
}

test("with a release commit on top, release.json names the commit the workflow ran on, as its attestations do", () => {
  const repo = repository();
  try {
    const prod = repo.head();
    repo.run("commit", "--quiet", "--allow-empty", "--message", "Release 3.4.6");
    repo.run("tag", "--annotate", "v3.4.6", "--message", "Kaeso 3.4.6");
    const tagged = repo.head();
    assert.notEqual(tagged, prod);
    assert.equal(sourceCommit({ cwd: repo.dir, version: "3.4.6", commit: prod }), prod);
    assert.throws(() => sourceCommit({ cwd: repo.dir, version: "3.4.6", commit: tagged }), /not the commit the Release workflow ran on/);
  } finally {
    rmSync(repo.dir, { recursive: true, force: true });
  }
});

test("without a release commit, the tagged commit is the one the workflow ran on", () => {
  const repo = repository();
  try {
    const prod = repo.head();
    repo.run("tag", "--annotate", "v3.4.6", "--message", "Kaeso 3.4.6");
    assert.equal(sourceCommit({ cwd: repo.dir, version: "3.4.6", commit: prod }), prod);
    assert.throws(() => sourceCommit({ cwd: repo.dir, version: "3.4.6", commit }), /not the commit/);
  } finally {
    rmSync(repo.dir, { recursive: true, force: true });
  }
});

test("a previous release that shipped release.json must hand it on", () => {
  const repo = repository();
  try {
    repo.run("tag", "--annotate", "v3.4.4", "--message", "Kaeso 3.4.4");
    mkdirSync(join(repo.dir, "scripts"));
    writeFileSync(join(repo.dir, "scripts", "release-manifest.mjs"), "");
    repo.run("add", ".");
    repo.run("commit", "--quiet", "--message", "Write release.json");
    repo.run("tag", "--annotate", "v3.4.5", "--message", "Kaeso 3.4.5");
    writeFileSync(join(repo.dir, "previous.json"), JSON.stringify({ manual_since: "3.4.5" }));
    assert.equal(previousManifest({ cwd: repo.dir, tag: undefined }), null);
    assert.equal(previousManifest({ cwd: repo.dir, tag: "v3.4.4" }), null, "releases before release.json existed have none");
    assert.throws(() => previousManifest({ cwd: repo.dir, tag: "v3.4.5" }), /v3\.4\.5 shipped release\.json, but it wasn't given/);
    assert.deepEqual(previousManifest({ cwd: repo.dir, tag: "v3.4.5", path: join(repo.dir, "previous.json") }), { manual_since: "3.4.5" });
  } finally {
    rmSync(repo.dir, { recursive: true, force: true });
  }
});

test("the latest migrations are the last files of each database", () => {
  const dir = mkdtempSync(join(tmpdir(), "release-migrations-"));
  try {
    mkdirSync(join(dir, "postgres"));
    mkdirSync(join(dir, "clickhouse"));
    for (const name of ["20261001000000_a.sql", "20261017120001_updates.sql", "20261002000000_b.sql"]) writeFileSync(join(dir, "postgres", name), "");
    for (const name of ["0001_request_log.sql", "0002_usage_rollups.sql"]) writeFileSync(join(dir, "clickhouse", name), "");
    assert.deepEqual(latestMigrations(dir), migrations);
    rmSync(join(dir, "clickhouse", "0001_request_log.sql"));
    rmSync(join(dir, "clickhouse", "0002_usage_rollups.sql"));
    assert.throws(() => latestMigrations(dir), /clickhouse has no migrations/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
  assert.deepEqual(Object.keys(latestMigrations(join(here, "../crates/kaeso-store/migrations"))), ["postgres", "clickhouse"]);
});

test("the command line writes the manifest from the digest files, the migrations and the previous release", () => {
  const repo = repository();
  try {
    const prod = repo.head();
    repo.run("commit", "--quiet", "--allow-empty", "--message", "Release 3.4.6");
    repo.run("tag", "--annotate", "v3.4.6", "--message", "Kaeso 3.4.6");
    const dir = repo.dir;
    for (const [service, value] of Object.entries(digests)) writeFileSync(join(dir, service), `${value}\n`);
    writeFileSync(join(dir, "compose.yaml"), compose("3.4.6"));
    writeFileSync(join(dir, "previous.yaml"), compose("3.4.5"));
    writeFileSync(join(dir, "previous.json"), JSON.stringify({ manual_since: "3.4.4" }));
    mkdirSync(join(dir, "migrations", "postgres"), { recursive: true });
    mkdirSync(join(dir, "migrations", "clickhouse"));
    writeFileSync(join(dir, "migrations", "postgres", "20261017120001_updates.sql"), "");
    writeFileSync(join(dir, "migrations", "clickhouse", "0002_usage_rollups.sql"), "");
    const args = (sha) => [
      join(here, "release-manifest.mjs"),
      "--version",
      "3.4.6",
      "--commit",
      sha,
      "--digests",
      dir,
      "--compose",
      join(dir, "compose.yaml"),
      "--migrations",
      join(dir, "migrations"),
      "--previous",
      join(dir, "previous.yaml"),
      "--previous-tag",
      "v3.4.6",
      "--previous-manifest",
      join(dir, "previous.json"),
      "--manual",
      "false",
    ];
    const m = JSON.parse(execFileSync(process.execPath, args(prod), { cwd: dir, encoding: "utf8" }));
    assert.equal(m.commit, prod);
    assert.equal(m.images.console.digest, digest("b"));
    assert.equal(m.manual, false);
    assert.equal(m.manual_since, "3.4.4");
    assert.deepEqual(m.migrations, migrations);
    assert.throws(
      () => execFileSync(process.execPath, args(repo.head()), { cwd: dir, encoding: "utf8", stdio: "pipe" }),
      /not the commit the Release workflow ran on/,
    );
  } finally {
    rmSync(repo.dir, { recursive: true, force: true });
  }
});
