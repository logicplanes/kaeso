import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { ALLOWED, apply, check, gitRepo, glob, isVersion, owned, requirement, strays } from "./version.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const fake = (files) => ({ files: () => Object.keys(files).sort(), read: (file) => files[file] ?? null });

const tree = (version, extra = {}) => ({
  "package.json": `{\n  "name": "kaeso",\n  "version": "${version}",\n  "private": true\n}\n`,
  "packages/ui/package.json": `{\n  "name": "@logicplanes/kaeso-ui",\n  "version": "${version}",\n  "dependencies": {\n    "uqr": "1.4.0"\n  },\n  "exports": {\n    "version": "./version.js"\n  }\n}\n`,
  "Cargo.toml": `[workspace]\nmembers = ["crates/*"]\n\n[workspace.package]\nversion = "${version}"\n\n[workspace.dependencies]\nkaeso-kpp = { path = "crates/kaeso-kpp", version = "${version}" }\nserde = { version = "1.4.0" }\n`,
  "crates/kaeso-kpp/Cargo.toml": `[package]\nname = "kaeso-kpp"\nversion.workspace = true\n\n[dependencies]\nserde.workspace = true\n`,
  "crates/kaeso-kpp/README.md": `# kaeso-kpp\n\n\`\`\`toml\n[dependencies]\nkaeso-kpp = "${requirement(version)}"\n\`\`\`\n`,
  "crates/kaeso-core/README.md": "# kaeso-core\n\nThe gateway's domain.\n",
  "Cargo.lock": `version = 4\n\n[[package]]\nname = "kaeso-kpp"\nversion = "${version}"\ndependencies = [\n "serde",\n]\n\n[[package]]\nname = "serde"\nversion = "1.4.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n`,
  "fuzz/Cargo.lock": `version = 4\n\n[[package]]\nname = "kaeso-kpp"\nversion = "${version}"\n\n[[package]]\nname = "kpp-fuzz"\nversion = "0.0.0"\n`,
  "openapi.json": `{\n  "openapi": "3.1.0",\n  "info": {\n    "title": "Kaeso Platform API",\n    "version": "${version}"\n  }\n}\n`,
  "deploy/compose.yaml": `services:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:${version}\n  console:\n    image: ghcr.io/logicplanes/kaeso-console:${version}\n  valkey:\n    image: valkey/valkey:1.4.0-alpine\n`,
  "CHANGELOG.md": `# Changelog\n\n## [Unreleased]\n\n## [${version}] - 2026-10-16\n`,
  "apps/docs/content/en/index.md": "This documentation describes Kaeso {{version}}.\n",
  "pnpm-lock.yaml": `packages:\n  find-proc@${version}: {}\n`,
  ...extra,
});

const messages = (problems) => problems.map(({ file, line, message }) => `${file}:${line}: ${message}`);

test("a version is a semantic version without a leading v", () => {
  for (const version of ["0.2.0", "1.4.0", "2.0.0-rc.1", "1.0.0-beta.2.x-y"]) assert.equal(isVersion(version), true, version);
  for (const version of ["v1.4.0", "1.4", "01.4.0", "1.4.0-", "1.4.0 ", ""]) assert.equal(isVersion(version), false, version);
});

test("a crate's README asks for the release's minor version, or a pre-release exactly", () => {
  assert.equal(requirement("0.2.0"), "0.2");
  assert.equal(requirement("1.4.3"), "1.4");
  assert.equal(requirement("2.0.0-rc.1"), "2.0.0-rc.1");
});

test("the release owns every manifest, lockfile, image tag and crate dependency, and nothing else", () => {
  assert.deepEqual(owned(fake(tree("1.4.0"))), [
    "Cargo.lock",
    "Cargo.toml",
    "crates/kaeso-kpp/README.md",
    "deploy/compose.yaml",
    "fuzz/Cargo.lock",
    "openapi.json",
    "package.json",
    "packages/ui/package.json",
  ]);
});

test("setting a version rewrites the files JavaScript owns and leaves other versions alone", () => {
  const before = tree("1.4.0");
  const after = tree("1.5.0");
  const changes = apply(fake(before), "1.5.0");
  assert.deepEqual([...changes.keys()].sort(), ["crates/kaeso-kpp/README.md", "deploy/compose.yaml", "package.json", "packages/ui/package.json"]);
  for (const [file, text] of changes) assert.equal(text, after[file], file);
  assert.match(changes.get("packages/ui/package.json"), /"uqr": "1\.4\.0"/);
  assert.match(changes.get("deploy/compose.yaml"), /valkey\/valkey:1\.4\.0-alpine/);
  assert.equal(apply(fake(after), "1.5.0").size, 0);
});

test("a pre-release pins the crates' README dependency to itself", () => {
  const changes = apply(fake(tree("1.4.0")), "2.0.0-rc.1");
  assert.match(changes.get("crates/kaeso-kpp/README.md"), /^kaeso-kpp = "2\.0\.0-rc\.1"$/m);
});

test("a repository whose files agree on its version passes", () => {
  assert.deepEqual(check(fake(tree("1.4.0"))), []);
});

test("a full release, the cargo steps included, leaves a repository that passes", () => {
  const before = tree("1.4.0");
  const cargo = Object.fromEntries(["Cargo.toml", "Cargo.lock", "fuzz/Cargo.lock", "openapi.json"].map((file) => [file, tree("1.5.0")[file]]));
  const released = {
    ...before,
    ...Object.fromEntries(apply(fake(before), "1.5.0")),
    ...cargo,
    "CHANGELOG.md": `${before["CHANGELOG.md"]}\n## [1.5.0] - 2026-11-02\n`,
  };
  assert.deepEqual(check(fake(released)), []);
});

test("a manifest that disagrees names the file, the line and how versions change", () => {
  const problems = check(
    fake(
      tree("1.4.0", {
        "packages/ui/package.json": `{\n  "name": "@logicplanes/kaeso-ui",\n  "version": "1.3.0"\n}\n`,
        "crates/kaeso-kpp/README.md": `\`\`\`toml\nkaeso-kpp = "1.3"\n\`\`\`\n`,
        "Cargo.lock": tree("1.4.0")["Cargo.lock"].replace('name = "kaeso-kpp"\nversion = "1.4.0"', 'name = "kaeso-kpp"\nversion = "1.3.0"'),
      }),
    ),
  );
  assert.deepEqual(
    problems.map(({ file, line }) => `${file}:${line}`),
    ["Cargo.lock:5", "crates/kaeso-kpp/README.md:2", "packages/ui/package.json:3"],
  );
  assert.equal(
    messages(problems)[2],
    "packages/ui/package.json:3: says 1.3.0 for its package version, but the repository's version is 1.4.0 (package.json); versions change only with `node scripts/version.mjs set <version>`, which the Release workflow runs",
  );
});

test("a manifest without a version is a problem, a crate that inherits the workspace's is not", () => {
  const problems = check(
    fake(tree("1.4.0", { "apps/docs/package.json": `{\n  "name": "kaeso-docs"\n}\n`, "crates/kaeso-mail/Cargo.toml": `[package]\nname = "kaeso-mail"\n` })),
  );
  assert.deepEqual(messages(problems), ["apps/docs/package.json:1: has no package version", "crates/kaeso-mail/Cargo.toml:1: has no crate version"]);
});

test("the current version written by hand anywhere else fails with its file and line", () => {
  const problems = check(
    fake(
      tree("1.4.0", {
        "apps/docs/content/en/upgrades.md":
          "Upgrade to Kaeso 1.4.0.\n\nNot 11.4.0, 1.4.0.2, 1.4.01 or 1.4.0-rc.1.\n\n```bash\ndocker pull ghcr.io/logicplanes/kaeso-gateway:v1.4.0\n```\n",
        "deploy/compose.yaml": `${tree("1.4.0")["deploy/compose.yaml"]}    environment:\n      KAESO_RELEASE: 1.4.0\n`,
        "docs/logo.png": null,
      }),
    ),
  );
  assert.deepEqual(
    problems.map(({ file, line }) => `${file}:${line}`),
    ["apps/docs/content/en/upgrades.md:1", "apps/docs/content/en/upgrades.md:6", "deploy/compose.yaml:9"],
  );
  assert.match(problems[0].message, /^states 1\.4\.0 by hand, so a later release would leave it behind: write \{\{version\}\} in a documentation page/);
});

test("a planned version can be checked before the release sets it", () => {
  const repo = fake(tree("1.4.0", { "apps/docs/content/en/upgrades.md": "Kaeso 1.5.0 brings batches.\n" }));
  assert.deepEqual(
    strays(repo, "1.5.0").map(({ file, line }) => `${file}:${line}`),
    ["apps/docs/content/en/upgrades.md:1"],
  );
  assert.deepEqual(strays(repo, "1.6.0"), []);
});

test("history and examples are allowed where ALLOWED says, some only while their version is current", () => {
  const gateway = "apps/docs/content/de/development/architecture/gateway.md";
  const first = { [gateway]: "Das Schema von 0.1.0 ist ein Satz.\n", "scripts/changelog.test.mjs": 'release(text, "0.1.0", "2026-11-02");\n' };
  assert.deepEqual(check(fake(tree("0.1.0", first))), []);
  const later = { [gateway]: "Das Schema von 1.4.0 ist ein Satz.\n", "scripts/changelog.test.mjs": 'release(text, "1.4.0", "2026-11-02");\n' };
  assert.deepEqual(
    check(fake(tree("1.4.0", later))).map(({ file, line }) => `${file}:${line}`),
    [`${gateway}:1`],
  );
});

test("every ALLOWED entry names files of the repository", () => {
  const files = gitRepo(root).files();
  for (const { path } of ALLOWED)
    assert.ok(
      files.some((file) => glob(path).test(file)),
      path,
    );
});

test("the repository states its version only where the release sets it", () => {
  assert.deepEqual(messages(check(gitRepo(root))), []);
});
