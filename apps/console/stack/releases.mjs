import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";

const PORT = Number(process.env.PORT ?? 8080);
const TOKEN = process.env.UPDATER_TOKEN ?? "";
const RUNNING = JSON.parse(readFileSync(process.env.VERSION_FILE ?? "/version/package.json", "utf8")).version;
const STEPS = ["verifying", "downloading", "switching", "migrating", "online"];

const [major, minor, patch] = RUNNING.split("-")[0].split(".").map(Number);
const VERSIONS = {
  patch: `${major}.${minor}.${patch + 1}`,
  minor: `${major}.${minor + 1}.0`,
  major: `${major + 1}.0.0`,
};

const initial = () => ({ releases: "patch", updater: "ready", outcome: "succeeded", jobs: new Map() });
let scenario = initial();

const sha = (text) => createHash("sha256").update(text).digest("hex");

function list() {
  switch (scenario.releases) {
    case "none":
      return [];
    case "major":
      return [{ version: VERSIONS.major, manual: false }];
    case "manual":
      return [{ version: VERSIONS.minor, manual: true }];
    default:
      return [
        { version: VERSIONS.patch, manual: false },
        { version: `${VERSIONS.minor}-rc.1`, manual: false, prerelease: true },
      ];
  }
}

function release(base, r) {
  return {
    tag_name: `v${r.version}`,
    draft: false,
    prerelease: !!r.prerelease,
    published_at: "2026-10-16T09:30:00Z",
    html_url: `https://github.com/logicplanes/kaeso/releases/tag/v${r.version}`,
    body: `### Added\n\n- Instance admins see which release is newest and update from the console.\n\n### Fixed\n\n- Streams keep their last chunk when a provider closes early ([#412](https://github.com/logicplanes/kaeso/pull/412)).`,
    assets: [{ name: "release.json", browser_download_url: `${base}/download/v${r.version}/release.json` }],
  };
}

function manifest(version) {
  const r = list().find((x) => x.version === version);
  const image = (name) => ({ name: `ghcr.io/logicplanes/kaeso-${name}`, digest: `sha256:${sha(`${name}@${version}`)}` });
  return {
    schema: 1,
    version,
    commit: sha(version).slice(0, 40),
    manual: !!r?.manual,
    manual_since: r?.manual ? version : null,
    migrations: { postgres: "20261017120001_updates", clickhouse: "0002_usage_rollups" },
    updater_protocol: 1,
    images: { gateway: image("gateway"), console: image("console"), updater: image("updater") },
    compose: { name: "compose.yaml", sha256: sha(`compose ${version}`) },
  };
}

function job(entry) {
  const at = Math.min(entry.polls, STEPS.length - 1);
  const done = entry.polls >= STEPS.length;
  const failed = scenario.outcome !== "succeeded" && entry.polls >= 3;
  const state = failed ? scenario.outcome : done ? "succeeded" : "running";
  return {
    id: entry.id,
    from: RUNNING,
    to: entry.version,
    state,
    step: failed ? "migrating" : STEPS[at],
    reason: failed ? "unhealthy" : null,
    detail: failed ? "the new gateway was not healthy within 600 seconds" : null,
    started_at: entry.started,
    finished_at: state === "running" ? null : new Date().toISOString(),
  };
}

function send(res, status, body, headers = {}) {
  res.writeHead(status, { "content-type": "application/json", ...headers });
  res.end(body === undefined ? "" : JSON.stringify(body));
}

async function read(req) {
  let text = "";
  for await (const chunk of req) text += chunk;
  return text ? JSON.parse(text) : {};
}

createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const base = `http://${req.headers.host}`;
  const path = url.pathname;
  if (path === "/healthz") return send(res, 200, { status: "ok" });
  if (path === "/__scenario" && req.method === "POST") {
    scenario = { ...initial(), ...(await read(req)), jobs: new Map() };
    return send(res, 200, { running: RUNNING, versions: VERSIONS });
  }
  if (path === "/repos/logicplanes/kaeso/releases") {
    if (scenario.releases === "error") return send(res, 502, { message: "Bad gateway" });
    return send(
      res,
      200,
      list().map((r) => release(base, r)),
      { etag: `"${scenario.releases}"` },
    );
  }
  const download = /^\/download\/v([^/]+)\/release\.json$/.exec(path);
  if (download) return send(res, 200, manifest(download[1]));
  if (path.startsWith("/updater/")) {
    if (scenario.updater === "down") return send(res, 503, { error: "unavailable" });
    if (req.headers.authorization !== `Bearer ${TOKEN}`) return send(res, 401, { error: "the updater token is missing or wrong" });
    const rest = path.slice("/updater".length);
    if (rest === "/v1/status") return send(res, 200, { version: RUNNING, protocol: 1, docker: true, job: null });
    if (rest === "/v1/updates" && req.method === "POST") {
      const body = await read(req);
      const entry = { id: body.id, version: body.version, polls: 0, started: new Date().toISOString() };
      scenario.jobs.set(body.id, entry);
      return send(res, 202, job(entry));
    }
    const one = /^\/v1\/updates\/([^/]+)$/.exec(rest);
    const entry = one && scenario.jobs.get(one[1]);
    if (!entry) return send(res, 404, { error: "no such update" });
    entry.polls += 1;
    return send(res, 200, job(entry));
  }
  send(res, 404, { message: "Not Found" });
}).listen(PORT);
