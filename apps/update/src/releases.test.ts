import { describe, expect, it, vi } from "vitest";
import { FEED_PATH, loadReleases, readFeed, splitReleases, upgradeGuide, type Release } from "./releases";

const entry = (tag: string, more: Record<string, unknown> = {}) => ({
  tag_name: tag,
  draft: false,
  prerelease: false,
  published_at: "2027-02-16T14:05:00Z",
  body: "## Added\n\n- Usage exports include the cached input tokens.\n",
  html_url: `https://github.com/logicplanes/kaeso/releases/tag/${tag}`,
  assets: [],
  ...more,
});

describe("the release feed", () => {
  it("reads the feed's releases in its order", () => {
    expect(readFeed([entry("v3.2.0-rc.1", { prerelease: true }), entry("v3.1.0", { body: null, published_at: null })])).toEqual([
      {
        version: "3.2.0-rc.1",
        prerelease: true,
        publishedAt: "2027-02-16T14:05:00Z",
        notes: "## Added\n\n- Usage exports include the cached input tokens.",
        url: "https://github.com/logicplanes/kaeso/releases/tag/v3.2.0-rc.1",
      },
      { version: "3.1.0", prerelease: false, publishedAt: null, notes: "", url: "https://github.com/logicplanes/kaeso/releases/tag/v3.1.0" },
    ]);
  });

  it("skips drafts and entries it cannot read", () => {
    expect(
      readFeed([entry("v3.3.0", { draft: true }), entry("nightly"), { tag_name: "v3.1.0" }, null, "v3.0.0", entry("v3.0.0")]).map((r) => r.version),
    ).toEqual(["3.0.0"]);
  });

  it("refuses a feed that isn't a list", () => {
    expect(() => readFeed({ message: "Not Found" })).toThrow("not a list");
  });

  it("loads the feed from this site", async () => {
    const fetchFn = vi.fn(async () => new Response(JSON.stringify([entry("v3.1.0")])));
    expect((await loadReleases(fetchFn)).map((r) => r.version)).toEqual(["3.1.0"]);
    expect(fetchFn).toHaveBeenCalledWith(FEED_PATH, { headers: { accept: "application/json" } });
    await expect(loadReleases(async () => new Response("", { status: 404 }))).rejects.toThrow("answered 404");
  });

  it("picks the newest stable release for the top and keeps the rest in order", () => {
    const r = (version: string, prerelease = false): Release => ({ version, prerelease, publishedAt: null, notes: "", url: "" });
    const list = [r("3.2.0-rc.1", true), r("3.1.0"), r("3.0.0")];
    expect(splitReleases(list)).toEqual({ latest: list[1], others: [list[0], list[2]] });
    expect(splitReleases(list.slice(0, 1))).toEqual({ latest: list[0], others: [] });
    expect(splitReleases([])).toEqual({ latest: null, others: [] });
  });

  it("links the upgrade guide in the reader's language", () => {
    expect(upgradeGuide("en")).toBe("https://docs.kaeso.app/self-hosting/upgrades");
    expect(upgradeGuide("de")).toBe("https://docs.kaeso.app/de/self-hosting/upgrades");
    expect(upgradeGuide("zh-CN")).toBe("https://docs.kaeso.app/zh-CN/self-hosting/upgrades");
  });
});
