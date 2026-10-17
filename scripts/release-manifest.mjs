import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
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

export function manifest({ version, commit, digests, compose, previousCompose = null, manual = false }) {
  if (!isVersion(version)) throw new Error(`${version} is not a version`);
  if (!COMMIT.test(commit)) throw new Error(`${commit} is not a commit`);
  const images = {};
  for (const service of SERVICES) {
    const digest = digests[service];
    if (!DIGEST.test(digest ?? "")) throw new Error(`the ${service} image has no digest`);
    images[service] = { name: `ghcr.io/logicplanes/kaeso-${service}`, digest };
  }
  const text = compose.toString("utf8");
  return {
    schema: SCHEMA,
    version,
    commit,
    manual: manual || composeChanged(previousCompose, text, version),
    updater_protocol: UPDATER_PROTOCOL,
    images,
    compose: { name: "compose.yaml", sha256: sha256(compose) },
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
    commit: o.commit,
    digests,
    compose: readFileSync(o.compose),
    previousCompose: o.previous ? readFileSync(o.previous, "utf8") : null,
    manual: o.manual === "true",
  });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}
