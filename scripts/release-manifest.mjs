import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { isVersion } from "./version.mjs";

export const SCHEMA = 1;
export const UPDATER_PROTOCOL = 1;
export const SERVICES = ["gateway", "console", "updater"];

const IMAGE = /(ghcr\.io\/logicplanes\/kaeso-(?:gateway|console|updater):)([0-9A-Za-z.+-]+)/g;
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const COMMIT = /^[0-9a-f]{40}$/;

export const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

export const retag = (text, version) => text.replace(IMAGE, (_, image) => `${image}${version}`);

export function composeChanged(previous, current, version) {
  return previous !== null && retag(previous, version) !== current;
}

export function latestMigrations(root) {
  const latest = (dir) => {
    const names = readdirSync(join(root, dir))
      .filter((n) => n.endsWith(".sql"))
      .sort();
    if (names.length === 0) throw new Error(`${dir} has no migrations`);
    return names[names.length - 1].replace(/\.sql$/, "");
  };
  return { postgres: latest("postgres"), clickhouse: latest("clickhouse") };
}

export function manualSince({ version, manual, previous }) {
  if (manual) return version;
  return previous?.manual_since ?? null;
}

const git = (cwd, ...args) => execFileSync("git", args, { cwd, encoding: "utf8" }).trim();

export function shippedManifest({ cwd, tag }) {
  try {
    execFileSync("git", ["cat-file", "-e", `${tag}:scripts/release-manifest.mjs`], { cwd, stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

export function previousManifest({ cwd, tag, path }) {
  if (path) return JSON.parse(readFileSync(path, "utf8"));
  if (!tag) return null;
  if (shippedManifest({ cwd, tag })) {
    throw new Error(`${tag} shipped release.json, but it wasn't given; download it so the steps by hand it names carry over`);
  }
  return null;
}

export function sourceCommit({ cwd, version, commit }) {
  const tag = `v${version}`;
  const tagged = git(cwd, "rev-parse", `${tag}^{commit}`);
  const subject = git(cwd, "log", "-1", "--format=%s", tagged);
  const source = subject === `Release ${version}` ? git(cwd, "rev-parse", `${tagged}^`) : tagged;
  if (commit !== source) {
    throw new Error(`${commit} is not the commit the Release workflow ran on (${source}); attestations name that commit, not ${tag}'s release commit`);
  }
  return commit;
}

export function manifest({ version, commit, digests, compose, migrations, previousCompose = null, previous = null, manual = false }) {
  if (!isVersion(version)) throw new Error(`${version} is not a version`);
  if (!COMMIT.test(commit)) throw new Error(`${commit} is not a commit`);
  if (!migrations?.postgres || !migrations?.clickhouse) throw new Error("the latest migrations are missing");
  const images = {};
  for (const service of SERVICES) {
    const digest = digests[service];
    if (!DIGEST.test(digest ?? "")) throw new Error(`the ${service} image has no digest`);
    images[service] = { name: `ghcr.io/logicplanes/kaeso-${service}`, digest };
  }
  const text = compose.toString("utf8");
  const needsSteps = manual || composeChanged(previousCompose, text, version);
  return {
    schema: SCHEMA,
    version,
    commit,
    manual: needsSteps,
    manual_since: manualSince({ version, manual: needsSteps, previous }),
    updater_protocol: UPDATER_PROTOCOL,
    images,
    compose: { name: "compose.yaml", sha256: sha256(compose) },
    migrations,
  };
}

function options(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1];
  return out;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const o = options(process.argv.slice(2));
  const digests = Object.fromEntries(SERVICES.map((s) => [s, readFileSync(`${o.digests}/${s}`, "utf8").trim()]));
  const result = manifest({
    version: o.version,
    commit: sourceCommit({ cwd: process.cwd(), version: o.version, commit: o.commit }),
    digests,
    compose: readFileSync(o.compose),
    migrations: latestMigrations(o.migrations),
    previousCompose: o.previous ? readFileSync(o.previous, "utf8") : null,
    previous: previousManifest({ cwd: process.cwd(), tag: o["previous-tag"], path: o["previous-manifest"] }),
    manual: o.manual === "true",
  });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}
