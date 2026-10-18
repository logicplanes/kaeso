import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$/;

export const isVersion = (version) => SEMVER.test(version);

export const requirement = (version) => (version.includes("-") ? version : version.split(".").slice(0, 2).join("."));

export const ALLOWED = [
  { path: "CHANGELOG.md", why: "the sections of released versions" },
  { path: "pnpm-lock.yaml", why: "the versions of npm dependencies" },
  { path: "scripts/version.mjs", why: "the versions this list names" },
  { path: "scripts/changelog.test.mjs", why: "the versions of changelog fixtures" },
  { path: "scripts/version.test.mjs", why: "the versions of fixtures" },
  { path: "scripts/release-notes.test.mjs", why: "the versions of changelog fixtures" },
  { path: "apps/showcase/scripts/refuse-raw-markup.test.mjs", why: "the versions of changelog fixtures" },
  { path: "apps/docs/src/page/changelog.test.tsx", why: "the versions of changelog fixtures" },
  { path: "apps/docs/e2e/changelog.spec.ts", why: "the versions of changelog fixtures" },
  { path: "apps/docs/e2e/fixtures/CHANGELOG.md", why: "the versions of changelog fixtures" },
  { path: ".github/workflows/release.yml", why: "example versions in the run form and its errors" },
  { path: "apps/docs/content/*/development/releasing.md", why: "example versions and tags" },
  { path: "apps/docs/scripts/*.test.ts", why: "the versions of fixture packages" },
  { path: "apps/docs/scripts/content/*.test.ts", why: "the versions of fixture pages" },
  { path: "apps/console/src/test/fixtures/*.ts", why: "the gateway version of fixtures" },
  { path: "apps/console/src/features/instance/system-page.test.tsx", why: "the gateway version of its fixture" },
  { path: "apps/showcase/src/demos/*.tsx", why: "the versions of demo data" },
  { path: "apps/docs/content/*/development/architecture/gateway.md", version: "0.1.0", why: "the schema and the log form of the first release" },
  { path: "apps/docs/content/*/self-hosting/upgrades.md", version: "0.1.0", why: "the migrations of the first release" },
  { path: "apps/docs/content/*/development/architecture/updates.md", version: "0.1.0", why: "the signed release the updater's tests verify" },
  { path: "crates/kaeso-updater/tests/verify.rs", version: "0.1.0", why: "the signed images of the first release the updater's tests verify" },
  { path: "crates/kaeso-updater/tests/fixtures/*", version: "0.1.0", why: "the attestations and image indexes of the first release" },
];

const lineAt = (text, index) => text.slice(0, index).split("\n").length;

const matches = (text, pattern, group) => [...text.matchAll(pattern)].map((m) => ({ line: lineAt(text, m.index), value: m[group] }));

const replaceGroup = (text, pattern, value) => text.replace(pattern, (_, ...groups) => `${groups[0]}${value}${groups[2] ?? ""}`);

const lockPackages = (text) => {
  const starts = [...text.matchAll(/^\[\[package\]\]$/gm)].map((m) => m.index);
  return starts
    .map((start, i) => {
      const block = text.slice(start, starts[i + 1] ?? text.length);
      const version = /^version = "([^"]+)"$/m.exec(block);
      return {
        name: /^name = "([^"]+)"$/m.exec(block)?.[1],
        value: version?.[1],
        line: lineAt(text, start + (version?.index ?? 0)),
        local: !/^source = /m.test(block),
      };
    })
    .filter((entry) => entry.local)
    .map(({ name, value, line }) => ({ name, value, line }));
};

const cargoManifest = (text) => {
  const entries = [];
  let section = "";
  text.split("\n").forEach((raw, i) => {
    const line = raw.trim();
    const header = /^\[([^\]]+)\]$/.exec(line);
    if (header) {
      section = header[1];
      return;
    }
    const isPackage = section === "package" || section === "workspace.package";
    if (isPackage && /^version\.workspace = true$/.test(line)) entries.push({ line: i + 1, inherits: true });
    const own = isPackage && /^version = "([^"]+)"$/.exec(line);
    if (own) entries.push({ line: i + 1, value: own[1] });
    const dependency = /^[\w-]+ = \{.*\bpath = "[^"]+".*\}$/.test(line) && /\bversion = "([^"]+)"/.exec(line);
    if (dependency) entries.push({ line: i + 1, value: dependency[1] });
  });
  return entries;
};

const PACKAGE_VERSION = /^( {2}"version": ")([^"]*)(")/m;
const IMAGE_TAG = /(ghcr\.io\/logicplanes\/kaeso-[a-z]+:)([0-9A-Za-z.+-]+)()/g;
const CRATE_REQUIREMENT = /^(kaeso-[a-z-]+ = ")([^"]+)(")$/gm;

const RULES = [
  {
    what: "package version",
    match: (file) => /^((apps|packages)\/[^/]+\/)?package\.json$/.test(file),
    manifest: true,
    required: true,
    read: (text) => matches(text, new RegExp(PACKAGE_VERSION.source, "gm"), 2),
    write: (text, version) => replaceGroup(text, PACKAGE_VERSION, version),
  },
  {
    what: "image tag",
    match: (file) => file === "deploy/compose.yaml",
    required: true,
    read: (text) => matches(text, IMAGE_TAG, 2),
    write: (text, version) => replaceGroup(text, IMAGE_TAG, version),
  },
  {
    what: "dependency on a Kaeso crate",
    match: (file) => /^crates\/[^/]+\/README\.md$/.test(file),
    expect: requirement,
    read: (text) => matches(text, CRATE_REQUIREMENT, 2),
    write: (text, version) => replaceGroup(text, CRATE_REQUIREMENT, requirement(version)),
  },
  {
    what: "crate version",
    match: (file) => file === "Cargo.toml" || /^crates\/[^/]+\/Cargo\.toml$/.test(file),
    manifest: true,
    required: true,
    read: cargoManifest,
  },
  {
    what: "workspace crate version",
    match: (file) => file === "Cargo.lock",
    manifest: true,
    required: true,
    read: lockPackages,
  },
  {
    what: "workspace crate version",
    match: (file) => file === "fuzz/Cargo.lock",
    manifest: true,
    required: true,
    read: (text, repo) => {
      const workspace = new Set(lockPackages(repo.read("Cargo.lock") ?? "").map((entry) => entry.name));
      return lockPackages(text).filter((entry) => workspace.has(entry.name));
    },
  },
  {
    what: "Platform API version",
    match: (file) => file === "openapi.json",
    manifest: true,
    required: true,
    read: (text) => matches(text, /^ {4}"version": "([^"]*)"/gm, 1).slice(0, 1),
  },
];

const ruleOf = (file) => RULES.find((rule) => rule.match(file));

export function gitRepo(root) {
  const listed = execFileSync("git", ["ls-files", "-z", "--cached", "--others", "--exclude-standard"], { cwd: root, encoding: "utf8" });
  const files = [...new Set(listed.split("\0").filter(Boolean))].filter((file) => existsSync(join(root, file)) && statSync(join(root, file)).isFile()).sort();
  const cache = new Map();
  return {
    files: () => files,
    read: (file) => {
      if (!cache.has(file)) {
        const bytes = existsSync(join(root, file)) ? readFileSync(join(root, file)) : null;
        cache.set(file, bytes && !bytes.includes(0) ? bytes.toString("utf8") : null);
      }
      return cache.get(file);
    },
  };
}

const entriesOf = (repo, file) => {
  const text = repo.read(file);
  return text === null ? [] : ruleOf(file).read(text, repo);
};

export function owned(repo) {
  return repo.files().filter((file) => ruleOf(file) && entriesOf(repo, file).some((entry) => !entry.inherits));
}

export function apply(repo, version) {
  const changes = new Map();
  for (const file of owned(repo)) {
    const { write } = ruleOf(file);
    const text = repo.read(file);
    const written = write ? write(text, version) : text;
    if (written !== text) changes.set(file, written);
  }
  return changes;
}

export const glob = (pattern) => new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, "[^/]*")}$`);

const allowed = (file, version) => ALLOWED.find((entry) => glob(entry.path).test(file) && (!entry.version || entry.version === version));

const mentions = (version) => new RegExp(`(?<![\\d.])${RegExp.escape(version)}(?!\\d|\\.\\d|-[0-9A-Za-z])`);

function current(repo) {
  return JSON.parse(repo.read("package.json")).version;
}

function disagreements(repo, version) {
  const problems = [];
  for (const file of repo.files()) {
    const rule = ruleOf(file);
    if (!rule) continue;
    const entries = entriesOf(repo, file);
    if (rule.required && !entries.length) problems.push({ file, line: 1, message: `has no ${rule.what}` });
    const expected = (rule.expect ?? ((v) => v))(version);
    for (const entry of entries.filter((e) => !e.inherits && e.value !== expected)) {
      problems.push({
        file,
        line: entry.line,
        message: `says ${entry.value} for its ${rule.what}, but the repository's version is ${version} (package.json); versions change only with \`node scripts/version.mjs set <version>\`, which the Release workflow runs`,
      });
    }
  }
  return problems;
}

export function strays(repo, version) {
  const pattern = mentions(version);
  const problems = [];
  for (const file of repo.files()) {
    const rule = ruleOf(file);
    if (rule?.manifest || allowed(file, version)) continue;
    const text = repo.read(file);
    if (text === null) continue;
    const setByRelease = new Set(rule ? rule.read(text, repo).map((entry) => entry.line) : []);
    text.split("\n").forEach((line, i) => {
      if (setByRelease.has(i + 1) || !pattern.test(line)) return;
      problems.push({
        file,
        line: i + 1,
        message: `states ${version} by hand, so a later release would leave it behind: write {{version}} in a documentation page, read the version from package.json or env!("CARGO_PKG_VERSION") in code, let scripts/version.mjs set it, or add history and examples to ALLOWED there`,
      });
    });
  }
  return problems;
}

export function check(repo) {
  const version = current(repo);
  return [...disagreements(repo, version), ...strays(repo, version)];
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const run = (command, args, options = {}) => execFileSync(command, args, { cwd: root, stdio: ["ignore", "inherit", "inherit"], ...options });

const report = (problems) => {
  const annotate = process.env.GITHUB_ACTIONS === "true";
  for (const { file, line, message } of problems) console.error(annotate ? `::error file=${file},line=${line}::${message}` : `${file}:${line}: ${message}`);
};

const commands = {
  set: (version) => {
    if (!isVersion(version ?? "")) throw new Error(`set needs a version like 1.4.0 or 1.4.0-rc.1, not ${version}`);
    for (const [file, text] of apply(gitRepo(root), version)) writeFileSync(join(root, file), text);
    run("cargo", ["set-version", "--workspace", version]);
    run("cargo", ["update", "--workspace"]);
    const fuzz = entriesOf(gitRepo(root), "fuzz/Cargo.lock").map((entry) => entry.name);
    run("cargo", ["update", "--manifest-path", "fuzz/Cargo.toml", ...fuzz]);
    writeFileSync(join(root, "openapi.json"), run("cargo", ["run", "-q", "-p", "kaeso", "--", "openapi"], { stdio: ["ignore", "pipe", "inherit"] }));
    commands.check();
  },
  check: (planned) => {
    if (planned !== undefined && !isVersion(planned)) throw new Error(`check needs a version like 1.4.0 or 1.4.0-rc.1, not ${planned}`);
    const repo = gitRepo(root);
    const problems = planned === undefined ? check(repo) : strays(repo, planned);
    report(problems);
    if (problems.length) process.exitCode = 1;
    else if (planned === undefined) console.log(`version: ${current(repo)}, stated only where a release sets it`);
    else console.log(`version: no file states ${planned} by hand, so a release can set it`);
  },
  files: () => process.stdout.write(`${owned(gitRepo(root)).join("\n")}\n`),
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  if (!commands[command]) {
    console.error("usage: node scripts/version.mjs set <version> | check [<version>] | files");
    process.exit(2);
  }
  commands[command](...args);
}
