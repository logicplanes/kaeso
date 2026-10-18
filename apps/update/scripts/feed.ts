import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { releaseArtSvg } from "@logicplanes/kaeso-ui/release-art";

export const FEED_ORIGIN = "https://update.kaeso.app";
export const RELEASES_URL = "https://api.github.com/repos/logicplanes/kaeso/releases?per_page=100";
export const MANIFEST = "release.json";
export const MAX_PAGES = 50;
export const MAX_FEED_BYTES = 3 << 20;
export const MAX_NOTES_BYTES = 64 << 10;

export interface GitHubAsset {
  name: string;
  browser_download_url: string;
}

export interface GitHubRelease {
  tag_name: string;
  draft: boolean;
  prerelease: boolean;
  published_at: string | null;
  body: string | null;
  html_url: string;
  assets: GitHubAsset[];
}

export interface Version {
  core: [number, number, number];
  pre: string[];
}

export interface Release {
  version: string;
  parsed: Version;
  prerelease: boolean;
  source: GitHubRelease;
}

export interface Latest {
  stable: string | null;
  prerelease: string | null;
}

export interface Source {
  releases: () => Promise<unknown[]>;
  manifest: (version: string, asset: GitHubAsset) => Promise<Uint8Array>;
}

export interface FeedSummary {
  releases: number;
  manifests: number;
  latest: Latest;
}

export type Fetch = (url: string, init?: { headers?: Record<string, string> }) => Promise<Response>;
type Env = Record<string, string | undefined>;

const IDENTIFIER = "(?:0|[1-9]\\d*|\\d*[A-Za-z-][0-9A-Za-z-]*)";
const TAG = new RegExp(`^v(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)(?:-(${IDENTIFIER}(?:\\.${IDENTIFIER})*))?$`);

export function parseTag(tag: string): Version | null {
  const m = TAG.exec(tag);
  if (!m) return null;
  return { core: [Number(m[1]), Number(m[2]), Number(m[3])], pre: m[4] ? m[4].split(".") : [] };
}

function compareIdentifiers(a: string, b: string): number {
  const an = /^\d+$/.test(a);
  const bn = /^\d+$/.test(b);
  if (an && bn) return Number(a) - Number(b);
  if (an !== bn) return an ? -1 : 1;
  return a < b ? -1 : a > b ? 1 : 0;
}

export function compareVersions(a: Version, b: Version): number {
  for (let i = 0; i < 3; i++) {
    const d = (a.core[i] ?? 0) - (b.core[i] ?? 0);
    if (d !== 0) return d;
  }
  if (!a.pre.length || !b.pre.length) return b.pre.length - a.pre.length;
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    const x = a.pre[i];
    const y = b.pre[i];
    if (x === undefined) return -1;
    if (y === undefined) return 1;
    const d = compareIdentifiers(x, y);
    if (d !== 0) return d;
  }
  return 0;
}

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const text = (value: unknown): string | null => (typeof value === "string" ? value : null);

export function readRelease(raw: unknown): GitHubRelease | null {
  if (!isRecord(raw)) return null;
  const tag = text(raw.tag_name);
  const html = text(raw.html_url);
  if (tag === null || html === null) return null;
  const assets = (Array.isArray(raw.assets) ? raw.assets : []).flatMap((a): GitHubAsset[] => {
    if (!isRecord(a)) return [];
    const name = text(a.name);
    const url = text(a.browser_download_url);
    return name !== null && url !== null ? [{ name, browser_download_url: url }] : [];
  });
  return {
    tag_name: tag,
    draft: raw.draft === true,
    prerelease: raw.prerelease === true,
    published_at: text(raw.published_at),
    body: text(raw.body),
    html_url: html,
    assets,
  };
}

export function selectReleases(raw: readonly unknown[]): Release[] {
  const byVersion = new Map<string, Release>();
  for (const item of raw) {
    const source = readRelease(item);
    if (!source || source.draft) continue;
    const parsed = parseTag(source.tag_name);
    if (!parsed) continue;
    const version = source.tag_name.slice(1);
    if (byVersion.has(version)) continue;
    byVersion.set(version, { version, parsed, prerelease: source.prerelease || parsed.pre.length > 0, source });
  }
  return [...byVersion.values()].sort((a, b) => compareVersions(b.parsed, a.parsed));
}

export const manifestPath = (version: string): string => `download/v${version}/${MANIFEST}`;

const encoder = new TextEncoder();
const byteLength = (text: string): number => encoder.encode(text).length;

export function capNotes(body: string | null, limit = MAX_NOTES_BYTES): string | null {
  if (body === null || byteLength(body) <= limit) return body;
  let end = Math.min(body.length, limit);
  while (end > 0 && byteLength(body.slice(0, end)) > limit) end -= 1;
  if (end > 0 && /[\uD800-\uDBFF]/.test(body.charAt(end - 1))) end -= 1;
  return body.slice(0, end);
}

export function historyEntry(release: Release, origin: string): GitHubRelease {
  const { source } = release;
  return {
    tag_name: source.tag_name,
    draft: false,
    prerelease: release.prerelease,
    published_at: source.published_at,
    body: source.body,
    html_url: source.html_url,
    assets: source.assets.map((a) => ({
      name: a.name,
      browser_download_url: a.name === MANIFEST ? `${origin}/${manifestPath(release.version)}` : a.browser_download_url,
    })),
  };
}

export function feedEntry(release: Release, origin: string): GitHubRelease {
  const entry = historyEntry(release, origin);
  return { ...entry, body: capNotes(entry.body) };
}

export function feedEntries(releases: readonly Release[], origin: string, limit = MAX_FEED_BYTES): GitHubRelease[] {
  const out: GitHubRelease[] = [];
  let size = byteLength("[\n]\n");
  for (const release of releases) {
    const entry = feedEntry(release, origin);
    const added = byteLength(JSON.stringify(entry, null, 2)) + 4;
    if (size + added > limit) break;
    out.push(entry);
    size += added;
  }
  return out;
}

export function latestOf(releases: readonly Release[]): Latest {
  return {
    stable: releases.find((r) => !r.prerelease)?.version ?? null,
    prerelease: releases[0]?.version ?? null,
  };
}

export function releaseArt(version: string, theme: "light" | "dark"): string {
  return releaseArtSvg({ seed: version, label: `Kaeso ${version}`, title: version, eyebrow: "Kaeso", theme });
}

export function nextLink(header: string | null): string | null {
  if (!header) return null;
  for (const part of header.split(",")) {
    const m = /^\s*<([^>]+)>(.*)$/.exec(part);
    if (m && /;\s*rel="?next"?\s*(;|$)/.test(m[2] ?? "")) return m[1] ?? null;
  }
  return null;
}

function refusal(res: Response, url: string): Error {
  const limited = res.status === 429 || (res.status === 403 && res.headers.get("x-ratelimit-remaining") === "0");
  const hint = limited ? "; GitHub's rate limit is reached, so set GITHUB_TOKEN or try again later" : "";
  return new Error(`${url} answered ${res.status}${hint}`);
}

export function githubReleases(fetchFn: Fetch, url: string, token?: string): () => Promise<unknown[]> {
  return async () => {
    const headers: Record<string, string> = {
      accept: "application/vnd.github+json",
      "x-github-api-version": "2022-11-28",
      "user-agent": "kaeso-update",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    };
    const all: unknown[] = [];
    let next: string | null = url;
    for (let pages = 0; next; pages++) {
      if (pages >= MAX_PAGES) throw new Error(`${url} has more than ${MAX_PAGES} pages of releases`);
      const res = await fetchFn(next, { headers });
      if (!res.ok) throw refusal(res, next);
      const page: unknown = await res.json();
      if (!Array.isArray(page)) throw new Error(`${next} did not answer with a list of releases`);
      all.push(...page);
      next = nextLink(res.headers.get("link"));
    }
    return all;
  };
}

export function githubManifest(fetchFn: Fetch): Source["manifest"] {
  return async (_version, asset) => {
    const res = await fetchFn(asset.browser_download_url, { headers: { accept: "application/octet-stream", "user-agent": "kaeso-update" } });
    if (!res.ok) throw refusal(res, asset.browser_download_url);
    return new Uint8Array(await res.arrayBuffer());
  };
}

export function fileReleases(file: string): () => Promise<unknown[]> {
  return async () => {
    const list: unknown = JSON.parse(readFileSync(file, "utf8"));
    if (!Array.isArray(list)) throw new Error(`${file} does not hold a list of releases`);
    return list;
  };
}

export function directoryManifest(dir: string): Source["manifest"] {
  return async (version) => {
    const file = join(dir, `v${version}`, MANIFEST);
    if (!existsSync(file)) throw new Error(`${file} is missing, but release v${version} has a ${MANIFEST}`);
    return new Uint8Array(readFileSync(file));
  };
}

export function sourceFromEnv(env: Env, fetchFn: Fetch): Source {
  const at = (path: string) => resolve(env.INIT_CWD ?? process.cwd(), path);
  return {
    releases: env.KAESO_RELEASES_FILE ? fileReleases(at(env.KAESO_RELEASES_FILE)) : githubReleases(fetchFn, RELEASES_URL, env.GITHUB_TOKEN || undefined),
    manifest: env.KAESO_RELEASE_ASSETS_DIR ? directoryManifest(at(env.KAESO_RELEASE_ASSETS_DIR)) : githubManifest(fetchFn),
  };
}

const json = (value: unknown): string => `${JSON.stringify(value, null, 2)}\n`;

function write(out: string, path: string, data: string | Uint8Array): void {
  const file = join(out, path);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, data);
}

export async function writeFeed(options: {
  out: string;
  origin: string;
  source: Source;
  staticDir?: string | undefined;
  maxFeedBytes?: number | undefined;
}): Promise<FeedSummary> {
  const origin = options.origin.replace(/\/+$/, "");
  const releases = selectReleases(await options.source.releases());
  const manifests: [string, Uint8Array][] = [];
  for (const release of releases) {
    const asset = release.source.assets.find((a) => a.name === MANIFEST);
    if (asset) manifests.push([release.version, await options.source.manifest(release.version, asset)]);
  }
  const latest = latestOf(releases);
  rmSync(options.out, { recursive: true, force: true });
  mkdirSync(options.out, { recursive: true });
  if (options.staticDir) cpSync(options.staticDir, options.out, { recursive: true });
  write(options.out, "v1/releases.json", json(feedEntries(releases, origin, options.maxFeedBytes)));
  write(options.out, "v1/history.json", json(releases.map((r) => historyEntry(r, origin))));
  write(options.out, "v1/latest.json", json(latest));
  for (const [version, bytes] of manifests) write(options.out, manifestPath(version), bytes);
  for (const { version } of releases) {
    write(options.out, `art/${version}.svg`, releaseArt(version, "light"));
    write(options.out, `art/${version}-dark.svg`, releaseArt(version, "dark"));
  }
  return { releases: releases.length, manifests: manifests.length, latest };
}

export function summaryLine(summary: FeedSummary): string {
  const { stable, prerelease } = summary.latest;
  return `feed: ${summary.releases} releases, ${summary.manifests} manifests mirrored, latest ${stable ?? "none"}, newest ${prerelease ?? "none"}`;
}

export async function main(root: string, env: Env, fetchFn: Fetch): Promise<string> {
  const summary = await writeFeed({ out: join(root, "public"), staticDir: join(root, "static"), origin: FEED_ORIGIN, source: sourceFromEnv(env, fetchFn) });
  return summaryLine(summary);
}

if (import.meta.main) {
  main(join(dirname(fileURLToPath(import.meta.url)), ".."), process.env, fetch).then(
    (line) => console.log(line),
    (error: unknown) => {
      console.error(`feed: ${error instanceof Error ? error.message : String(error)}`);
      process.exitCode = 1;
    },
  );
}
