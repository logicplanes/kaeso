import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  FEED_ORIGIN,
  MAX_PAGES,
  RELEASES_URL,
  compareVersions,
  githubManifest,
  githubReleases,
  latestOf,
  main,
  nextLink,
  parseTag,
  readRelease,
  releaseArt,
  selectReleases,
  sourceFromEnv,
  writeFeed,
  type Fetch,
  type GitHubRelease,
  type Source,
  type Version,
} from "./feed.ts";

const app = fileURLToPath(new URL("..", import.meta.url));
const fixtures = join(app, "fixtures");
const fixtureReleases = JSON.parse(readFileSync(join(fixtures, "releases.json"), "utf8")) as unknown[];

let dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  dirs = [];
});

function tempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "kaeso-update-feed-"));
  dirs.push(dir);
  return dir;
}

function release(tag: string, more: Partial<GitHubRelease> = {}): GitHubRelease {
  return {
    tag_name: tag,
    draft: false,
    prerelease: false,
    published_at: "2027-02-16T14:05:00Z",
    body: `Notes of ${tag}`,
    html_url: `https://github.com/logicplanes/kaeso/releases/tag/${tag}`,
    assets: [],
    ...more,
  };
}

const manifestAsset = (tag: string) => ({
  name: "release.json",
  browser_download_url: `https://github.com/logicplanes/kaeso/releases/download/${tag}/release.json`,
});

function respond(body: unknown, init: { status?: number; headers?: Record<string, string> } = {}): Response {
  const data: BodyInit = body instanceof Uint8Array ? (body as Uint8Array<ArrayBuffer>) : JSON.stringify(body);
  return new Response(data, { status: init.status ?? 200, headers: init.headers ?? {} });
}

function memorySource(raw: unknown[], manifests: Record<string, Uint8Array>): Source {
  return {
    releases: async () => raw,
    manifest: async (version) => {
      const bytes = manifests[version];
      if (!bytes) throw new Error(`no manifest for ${version}`);
      return bytes;
    },
  };
}

const fixtureSource = () => sourceFromEnv({ KAESO_RELEASES_FILE: join(fixtures, "releases.json"), KAESO_RELEASE_ASSETS_DIR: join(fixtures, "assets") }, fetch);

const read = (dir: string, path: string) => readFileSync(join(dir, path), "utf8");

describe("tags", () => {
  it("reads semver tags with a v and refuses everything else", () => {
    expect(parseTag("v3.1.0")).toEqual({ core: [3, 1, 0], pre: [] });
    expect(parseTag("v3.2.0-rc.1")).toEqual({ core: [3, 2, 0], pre: ["rc", "1"] });
    for (const tag of ["3.1.0", "nightly", "v3.1", "v03.1.0", "v3.1.0+build.7", "v3.1.0-", "v3.1.0-rc.01", "v3.1.0-rc..1"]) expect(parseTag(tag)).toBeNull();
  });

  it("orders versions as semver does", () => {
    const ordered = [
      "v2.9.9",
      "v3.0.0-alpha",
      "v3.0.0-alpha.1",
      "v3.0.0-alpha.beta",
      "v3.0.0-beta.2",
      "v3.0.0-beta.11",
      "v3.0.0-rc.1",
      "v3.0.0",
      "v3.0.1",
      "v3.1.0",
      "v10.0.0",
    ];
    const v = (tag: string | undefined) => parseTag(tag ?? "") as Version;
    for (let i = 1; i < ordered.length; i++) {
      expect(compareVersions(v(ordered[i - 1]), v(ordered[i]))).toBeLessThan(0);
      expect(compareVersions(v(ordered[i]), v(ordered[i - 1]))).toBeGreaterThan(0);
    }
    expect(compareVersions(v("v3.0.0-rc.1"), v("v3.0.0-rc.1"))).toBe(0);
  });
});

describe("releases", () => {
  it("reads GitHub's release objects and drops what the gateway cannot use", () => {
    expect(readRelease(null)).toBeNull();
    expect(readRelease([])).toBeNull();
    expect(readRelease({ tag_name: "v3.1.0" })).toBeNull();
    expect(
      readRelease({
        tag_name: "v3.1.0",
        html_url: "https://github.com/logicplanes/kaeso/releases/tag/v3.1.0",
        id: 7,
        assets: [{ name: "release.json", browser_download_url: "https://example.com/r.json", size: 9 }, { name: "broken" }, "nonsense"],
      }),
    ).toEqual({
      tag_name: "v3.1.0",
      draft: false,
      prerelease: false,
      published_at: null,
      body: null,
      html_url: "https://github.com/logicplanes/kaeso/releases/tag/v3.1.0",
      assets: [{ name: "release.json", browser_download_url: "https://example.com/r.json" }],
    });
    expect(readRelease({ tag_name: "v3.1.0", html_url: "https://github.com/x", assets: "none" })?.assets).toEqual([]);
  });

  it("skips drafts, tags that aren't semver and repeated versions, newest first", () => {
    const selected = selectReleases([
      release("v3.0.0"),
      release("v3.3.0", { draft: true }),
      release("nightly"),
      release("v3.1.0"),
      release("v3.1.0", { body: "a second copy" }),
      release("v3.2.0-rc.1", { prerelease: true }),
      "nonsense",
    ]);
    expect(selected.map((r) => r.version)).toEqual(["3.2.0-rc.1", "3.1.0", "3.0.0"]);
    expect(selected[1]?.source.body).toBe("Notes of v3.1.0");
  });

  it("marks a release as a pre-release when GitHub does or when its version has a pre-release part", () => {
    const selected = selectReleases([release("v3.1.0"), release("v3.0.0-beta.2"), release("v3.0.1", { prerelease: true })]);
    expect(selected.map((r) => [r.version, r.prerelease])).toEqual([
      ["3.1.0", false],
      ["3.0.1", true],
      ["3.0.0-beta.2", true],
    ]);
  });

  it("names the newest stable release and the newest release of all", () => {
    expect(latestOf(selectReleases([release("v3.1.0"), release("v3.2.0-rc.1")]))).toEqual({ stable: "3.1.0", prerelease: "3.2.0-rc.1" });
    expect(latestOf(selectReleases([release("v3.2.0-rc.1")]))).toEqual({ stable: null, prerelease: "3.2.0-rc.1" });
    expect(latestOf([])).toEqual({ stable: null, prerelease: null });
  });
});

describe("GitHub", () => {
  it("finds the next page in a Link header", () => {
    const next = "https://api.github.com/repositories/1/releases?per_page=100&page=2";
    expect(nextLink(`<${next}>; rel="next", <https://api.github.com/repositories/1/releases?per_page=100&page=5>; rel="last"`)).toBe(next);
    expect(nextLink(`<https://api.github.com/x?page=1>; rel="prev", <${next}>; rel="next"`)).toBe(next);
    expect(nextLink(`<https://api.github.com/x?page=1>; rel="prev"`)).toBeNull();
    expect(nextLink(null)).toBeNull();
    expect(nextLink("")).toBeNull();
  });

  it("follows every page and sends the token when there is one", async () => {
    const page2 = "https://api.github.com/repositories/1/releases?per_page=100&page=2";
    const page3 = "https://api.github.com/repositories/1/releases?per_page=100&page=3";
    const pages: Record<string, Response> = {
      [RELEASES_URL]: respond([release("v3.1.0")], { headers: { link: `<${page2}>; rel="next"` } }),
      [page2]: respond([release("v3.0.1")], { headers: { link: `<${page3}>; rel="next", <${RELEASES_URL}>; rel="first"` } }),
      [page3]: respond([release("v3.0.0")], { headers: { link: `<${RELEASES_URL}>; rel="first"` } }),
    };
    const fetchFn = vi.fn<Fetch>(async (url) => pages[url] as Response);
    const list = await githubReleases(fetchFn, RELEASES_URL, "ghp_example")();
    expect(list.map((r) => (r as GitHubRelease).tag_name)).toEqual(["v3.1.0", "v3.0.1", "v3.0.0"]);
    expect(fetchFn.mock.calls.map(([url]) => url)).toEqual([RELEASES_URL, page2, page3]);
    expect(fetchFn.mock.calls[0]?.[1]?.headers).toMatchObject({ authorization: "Bearer ghp_example", accept: "application/vnd.github+json" });
  });

  it("asks without a token when there is none", async () => {
    const fetchFn = vi.fn<Fetch>(async () => respond([]));
    await githubReleases(fetchFn, RELEASES_URL)();
    expect(fetchFn.mock.calls[0]?.[1]?.headers).not.toHaveProperty("authorization");
  });

  it("says when GitHub refuses, and why when the rate limit is reached", async () => {
    await expect(githubReleases(async () => respond({}, { status: 500 }), RELEASES_URL)()).rejects.toThrow(`${RELEASES_URL} answered 500`);
    await expect(githubReleases(async () => respond({}, { status: 403, headers: { "x-ratelimit-remaining": "0" } }), RELEASES_URL)()).rejects.toThrow(
      "GitHub's rate limit is reached, so set GITHUB_TOKEN or try again later",
    );
    await expect(githubReleases(async () => respond({}, { status: 429 }), RELEASES_URL)()).rejects.toThrow("rate limit");
    await expect(githubReleases(async () => respond({}, { status: 403 }), RELEASES_URL)()).rejects.toThrow(/answered 403$/);
    await expect(githubReleases(async () => respond({ message: "Not Found" }), RELEASES_URL)()).rejects.toThrow("did not answer with a list of releases");
  });

  it("stops at a Link header that never ends", async () => {
    const fetchFn = vi.fn<Fetch>(async () => respond([], { headers: { link: `<${RELEASES_URL}>; rel="next"` } }));
    await expect(githubReleases(fetchFn, RELEASES_URL)()).rejects.toThrow(`more than ${MAX_PAGES} pages`);
    expect(fetchFn).toHaveBeenCalledTimes(MAX_PAGES);
  });

  it("downloads a release manifest as bytes", async () => {
    const bytes = new Uint8Array([123, 10, 32, 125, 13, 10]);
    const fetchFn = vi.fn<Fetch>(async () => respond(bytes));
    expect(await githubManifest(fetchFn)("3.1.0", manifestAsset("v3.1.0"))).toEqual(bytes);
    expect(fetchFn.mock.calls[0]?.[0]).toBe("https://github.com/logicplanes/kaeso/releases/download/v3.1.0/release.json");
    await expect(githubManifest(async () => respond("", { status: 404 }))("3.1.0", manifestAsset("v3.1.0"))).rejects.toThrow("answered 404");
  });
});

describe("the sources", () => {
  it("read the release list and the manifests from files when asked to", async () => {
    const source = fixtureSource();
    expect(await source.releases()).toEqual(fixtureReleases);
    expect(await source.manifest("3.1.0", manifestAsset("v3.1.0"))).toEqual(new Uint8Array(readFileSync(join(fixtures, "assets/v3.1.0/release.json"))));
    await expect(source.manifest("3.0.1", manifestAsset("v3.0.1"))).rejects.toThrow("release v3.0.1 has a release.json");
  });

  it("resolve relative paths from where the command was run", async () => {
    const source = sourceFromEnv({ INIT_CWD: app, KAESO_RELEASES_FILE: "fixtures/releases.json" }, fetch);
    expect(await source.releases()).toHaveLength(fixtureReleases.length);
  });

  it("refuse a releases file that isn't a list", async () => {
    const dir = tempDir();
    writeFileSync(join(dir, "releases.json"), "{}");
    await expect(sourceFromEnv({ KAESO_RELEASES_FILE: join(dir, "releases.json") }, fetch).releases()).rejects.toThrow("does not hold a list of releases");
  });

  it("ask GitHub when no files are given, with the token from GITHUB_TOKEN", async () => {
    const fetchFn = vi.fn<Fetch>(async (url) => (url === RELEASES_URL ? respond([release("v3.1.0")]) : respond(new Uint8Array([1, 2, 3]))));
    const source = sourceFromEnv({ GITHUB_TOKEN: "ghp_example" }, fetchFn);
    expect(await source.releases()).toHaveLength(1);
    expect(fetchFn.mock.calls[0]?.[1]?.headers).toMatchObject({ authorization: "Bearer ghp_example" });
    expect(await source.manifest("3.1.0", manifestAsset("v3.1.0"))).toEqual(new Uint8Array([1, 2, 3]));
    await sourceFromEnv({ GITHUB_TOKEN: "" }, fetchFn).releases();
    expect(fetchFn.mock.calls.at(-1)?.[1]?.headers).not.toHaveProperty("authorization");
  });
});

describe("the feed", () => {
  it("writes GitHub's release shape, newest first, with manifests on the feed's own origin", async () => {
    const out = tempDir();
    await writeFeed({ out, origin: "https://feed.example.com/", source: fixtureSource() });
    const feed = JSON.parse(read(out, "v1/releases.json")) as GitHubRelease[];
    expect(feed.map((r) => r.tag_name)).toEqual(["v3.2.0-rc.1", "v3.1.0", "v3.0.1", "v3.0.0", "v3.0.0-beta.2"]);
    expect(feed.map((r) => r.prerelease)).toEqual([true, false, false, false, true]);
    expect(feed.every((r) => r.draft === false)).toBe(true);
    expect(feed[1]).toEqual({
      tag_name: "v3.1.0",
      draft: false,
      prerelease: false,
      published_at: "2027-02-16T14:05:00Z",
      body: (fixtureReleases[3] as GitHubRelease).body,
      html_url: "https://github.com/logicplanes/kaeso/releases/tag/v3.1.0",
      assets: [
        { name: "compose.yaml", browser_download_url: "https://github.com/logicplanes/kaeso/releases/download/v3.1.0/compose.yaml" },
        { name: "release.json", browser_download_url: "https://feed.example.com/download/v3.1.0/release.json" },
      ],
    });
    expect(Object.keys(feed[0] ?? {})).toEqual(["tag_name", "draft", "prerelease", "published_at", "body", "html_url", "assets"]);
    expect(feed[0]?.assets).toEqual([
      { name: "release.json", browser_download_url: "https://feed.example.com/download/v3.2.0-rc.1/release.json" },
      { name: "release.json.sig", browser_download_url: "https://github.com/logicplanes/kaeso/releases/download/v3.2.0-rc.1/release.json.sig" },
    ]);
    expect(feed[3]?.body).toBeNull();
    expect(feed[2]?.assets).toEqual([]);
  });

  it("uses update.kaeso.app as the origin installs trust", async () => {
    const out = tempDir();
    await writeFeed({ out, origin: FEED_ORIGIN, source: fixtureSource() });
    const feed = JSON.parse(read(out, "v1/releases.json")) as GitHubRelease[];
    expect(feed[1]?.assets[1]?.browser_download_url).toBe("https://update.kaeso.app/download/v3.1.0/release.json");
  });

  it("mirrors every release manifest byte for byte", async () => {
    const out = tempDir();
    const odd = new Uint8Array([...new TextEncoder().encode('{ "version":"3.1.0" ,\r\n "note": "Grüße, 你好" }  \r\n'), 0xef, 0xbb, 0xbf]);
    const other = new TextEncoder().encode('{"version":"3.0.0"}');
    const summary = await writeFeed({
      out,
      origin: "https://feed.example.com",
      source: memorySource(
        [release("v3.1.0", { assets: [manifestAsset("v3.1.0")] }), release("v3.0.0", { assets: [manifestAsset("v3.0.0")] }), release("v2.9.0")],
        {
          "3.1.0": odd,
          "3.0.0": other,
        },
      ),
    });
    expect(new Uint8Array(readFileSync(join(out, "download/v3.1.0/release.json")))).toEqual(odd);
    expect(new Uint8Array(readFileSync(join(out, "download/v3.0.0/release.json")))).toEqual(other);
    expect(existsSync(join(out, "download/v2.9.0"))).toBe(false);
    expect(summary).toEqual({ releases: 3, manifests: 2, latest: { stable: "3.1.0", prerelease: "3.1.0" } });
  });

  it("mirrors the fixture manifests unchanged", async () => {
    const out = tempDir();
    await writeFeed({ out, origin: FEED_ORIGIN, source: fixtureSource() });
    for (const version of ["3.2.0-rc.1", "3.1.0", "3.0.0"])
      expect(readFileSync(join(out, `download/v${version}/release.json`))).toEqual(readFileSync(join(fixtures, `assets/v${version}/release.json`)));
    expect(readdirSync(join(out, "download")).sort()).toEqual(["v3.0.0", "v3.1.0", "v3.2.0-rc.1"]);
  });

  it("writes the newest stable release and the newest of all to latest.json", async () => {
    const out = tempDir();
    await writeFeed({ out, origin: FEED_ORIGIN, source: fixtureSource() });
    expect(read(out, "v1/latest.json")).toBe('{\n  "stable": "3.1.0",\n  "prerelease": "3.2.0-rc.1"\n}\n');
    const empty = tempDir();
    await writeFeed({ out: empty, origin: FEED_ORIGIN, source: memorySource([], {}) });
    expect(JSON.parse(read(empty, "v1/latest.json"))).toEqual({ stable: null, prerelease: null });
    expect(JSON.parse(read(empty, "v1/releases.json"))).toEqual([]);
  });

  it("draws a light and a dark artwork for every release, the same as the library's", async () => {
    const out = tempDir();
    await writeFeed({ out, origin: FEED_ORIGIN, source: fixtureSource() });
    expect(readdirSync(join(out, "art")).sort()).toEqual([
      "3.0.0-beta.2-dark.svg",
      "3.0.0-beta.2.svg",
      "3.0.0-dark.svg",
      "3.0.0.svg",
      "3.0.1-dark.svg",
      "3.0.1.svg",
      "3.1.0-dark.svg",
      "3.1.0.svg",
      "3.2.0-rc.1-dark.svg",
      "3.2.0-rc.1.svg",
    ]);
    const light = read(out, "art/3.1.0.svg");
    const dark = read(out, "art/3.1.0-dark.svg");
    expect(light).toMatch(/^<svg [^>]*aria-label="Kaeso 3\.1\.0"/);
    expect(light).toContain(">3.1.0<");
    expect(light).toContain(">Kaeso<");
    expect(light).not.toBe(dark);
    expect(light).toBe(releaseArt("3.1.0", "light"));
    expect(read(out, "art/3.0.0.svg")).not.toBe(light.replaceAll("3.1.0", "3.0.0"));
  });

  it("writes the same files for the same input and leaves nothing of an earlier run", async () => {
    const out = tempDir();
    await writeFeed({ out, origin: FEED_ORIGIN, source: fixtureSource() });
    const first = read(out, "v1/releases.json") + read(out, "art/3.1.0-dark.svg");
    mkdirSync(join(out, "download/v0.0.1"), { recursive: true });
    writeFileSync(join(out, "download/v0.0.1/release.json"), "{}");
    await writeFeed({ out, origin: FEED_ORIGIN, source: fixtureSource() });
    expect(read(out, "v1/releases.json") + read(out, "art/3.1.0-dark.svg")).toBe(first);
    expect(existsSync(join(out, "download/v0.0.1"))).toBe(false);
  });

  it("writes nothing when a manifest cannot be fetched", async () => {
    const out = tempDir();
    writeFileSync(join(out, "keep.txt"), "earlier build");
    await expect(writeFeed({ out, origin: FEED_ORIGIN, source: memorySource([release("v3.1.0", { assets: [manifestAsset("v3.1.0")] })], {}) })).rejects.toThrow(
      "no manifest for 3.1.0",
    );
    expect(read(out, "keep.txt")).toBe("earlier build");
  });

  it("copies the site's static files next to the feed", async () => {
    const out = join(tempDir(), "public");
    const staticDir = tempDir();
    writeFileSync(join(staticDir, "robots.txt"), "User-agent: *\n");
    await writeFeed({ out, origin: FEED_ORIGIN, source: memorySource([], {}), staticDir });
    expect(read(out, "robots.txt")).toBe("User-agent: *\n");
  });

  it("runs from the command line against the fixtures and says what it wrote", async () => {
    const root = tempDir();
    mkdirSync(join(root, "static"));
    writeFileSync(join(root, "static/favicon.svg"), "<svg/>");
    const line = await main(root, { KAESO_RELEASES_FILE: join(fixtures, "releases.json"), KAESO_RELEASE_ASSETS_DIR: join(fixtures, "assets") }, fetch);
    expect(line).toBe("feed: 5 releases, 3 manifests mirrored, latest 3.1.0, newest 3.2.0-rc.1");
    expect(read(root, "public/favicon.svg")).toBe("<svg/>");
    expect(existsSync(join(root, "public/v1/releases.json"))).toBe(true);
    const none = tempDir();
    mkdirSync(join(none, "static"));
    writeFileSync(join(none, "releases.json"), "[]");
    expect(await main(none, { KAESO_RELEASES_FILE: join(none, "releases.json") }, fetch)).toBe(
      "feed: 0 releases, 0 manifests mirrored, latest none, newest none",
    );
  });
});
