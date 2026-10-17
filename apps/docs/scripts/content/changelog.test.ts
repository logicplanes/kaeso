import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { MESSAGES } from "../../src/i18n/messages/index.ts";
import type { Block } from "../../src/markdown/ast.ts";
import { buildChangelog, changelogClash, changelogToc, withChangelog } from "./changelog.ts";
import { buildDocs } from "./docs.ts";
import { buildSite } from "./site.ts";

const docsRoot = join(import.meta.dirname, "..", "..");
const fixture = join(docsRoot, "e2e/fixtures/CHANGELOG.md");
const repoUrl = "https://github.com/logicplanes/kaeso";

let root = "";
afterEach(() => root && rmSync(root, { recursive: true, force: true }));

function repo(files: Record<string, string>) {
  root = mkdtempSync(join(tmpdir(), "kaeso-docs-"));
  for (const [name, text] of Object.entries(files)) {
    const file = join(root, name);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
  }
  return root;
}

const page = (title: string) => `---\ntitle: ${title}\ndescription: About ${title.toLowerCase()}.\n---\n\nText.\n`;
const everyLanguage = (files: Record<string, string>) =>
  Object.fromEntries(["en", "de", "zh-CN"].flatMap((l) => Object.entries(files).map(([name, text]) => [`content/${l}/${name}`, text])));

const text = (blocks: Block[]) => JSON.stringify(blocks);

describe("buildChangelog", () => {
  const { content, problems, rel, words } = buildChangelog(fixture, docsRoot);

  it("reads every release of the changelog, newest first, with its anchor, date, release and compare view", () => {
    expect(problems).toEqual([]);
    expect(rel).toBe("e2e/fixtures/CHANGELOG.md");
    expect(content.repository).toBe(repoUrl);
    expect(content.releases.map((r) => [r.id, r.date, r.prerelease, r.yanked, r.latest])).toEqual([
      ["v1.3.0-rc.1", "2026-12-01", true, false, false],
      ["v1.2.0", "2026-11-15", false, false, true],
      ["v1.1.1", "2026-10-08", false, true, false],
      ["v1.1.0", "2026-10-01", false, false, false],
      ["v1.0.0", "2026-09-01", false, false, false],
    ]);
    const latest = content.releases[1];
    expect(latest?.url).toBe(`${repoUrl}/releases/tag/v1.2.0`);
    expect(latest?.compare).toBe(`${repoUrl}/compare/v1.1.1...v1.2.0`);
    expect(latest?.previous).toBe("1.1.1");
    expect(content.releases.at(-1)).not.toHaveProperty("compare");
  });

  it("keeps the lines under [Unreleased] apart, with the compare view since the newest release", () => {
    expect(content.unreleased?.id).toBe("unreleased");
    expect(content.unreleased?.since).toBe("1.3.0-rc.1");
    expect(content.unreleased?.compare).toBe(`${repoUrl}/compare/v1.3.0-rc.1...HEAD`);
    expect(content.unreleased?.sections.map((s) => [s.id, s.group])).toEqual([
      ["unreleased-added", "added"],
      ["unreleased-fixed", "fixed"],
    ]);
  });

  it("compiles sections, topics, nested and multi-line list items, and reference links", () => {
    const release = content.releases[1];
    expect(text(release?.intro ?? [])).toContain("Aliases across projects");
    expect(release?.sections.map((s) => [s.id, s.group, s.topics.map((t) => t.id)])).toEqual([
      ["v1.2.0-added", "added", ["v1.2.0-added-model-api", "v1.2.0-added-console"]],
      ["v1.2.0-changed", "changed", []],
      ["v1.2.0-deprecated", "deprecated", []],
      ["v1.2.0-removed", "removed", []],
      ["v1.2.0-security", "security", []],
    ]);
    const [api] = release?.sections[0]?.topics ?? [];
    const [list] = api?.blocks ?? [];
    expect(list?.t).toBe("list");
    const first = list?.t === "list" ? list.items[0]?.c : [];
    expect(first?.map((b) => b.t)).toEqual(["paragraph", "list"]);
    expect(text(api?.blocks ?? [])).toContain(
      '"v":"kaeso-model"},{"t":"text","v":" on every streamed response, so an app sees which model answered\\neven when it asked for an alias."',
    );
    expect(text(release?.sections[1]?.blocks ?? [])).toContain('"kind":"external","href":"https://docs.kaeso.app/console/request-logs"');
    expect(text(content.releases[3]?.sections[0]?.blocks ?? [])).toContain('"kind":"anchor","href":"#v1.0.0"');
    expect(words).toContain("aliases");
  });

  it("names every link the site can't follow, and a missing file", () => {
    repo({
      "CHANGELOG.md": "# Changelog\n\n## [Unreleased]\n\n## [1.0.0] - 2026-09-01\n\n### Added\n\n- See [the guide](docs/guide.md) and [the note](#v9.9.9).\n",
    });
    expect(buildChangelog(join(root, "CHANGELOG.md"), root).problems).toEqual([
      "CHANGELOG.md: [1.0.0] links to docs/guide.md: link to a full https address, or to a version on this page by its anchor, v and the version",
      "CHANGELOG.md: [1.0.0] links to #v9.9.9, which is not a heading on this page",
    ]);
    expect(buildChangelog(join(root, "NOTES.md"), root)).toMatchObject({ content: { releases: [] }, problems: ["NOTES.md does not exist"] });
  });
});

describe("the changelog page", () => {
  const changelog = buildChangelog(fixture, docsRoot);

  it("lists Unreleased and every version on the page, in the page's language", () => {
    expect(changelogToc(changelog.content, MESSAGES.de).map((e) => e.label)).toEqual(["Unveröffentlicht", "1.3.0-rc.1", "1.2.0", "1.1.1", "1.1.0", "1.0.0"]);
    expect(changelogToc({ releases: [] }, MESSAGES.en)).toEqual([]);
  });

  it("follows the home page in the sidebar, between it and the next page, and comes last in the search", () => {
    repo(everyLanguage({ "index.md": page("Start"), "keys.md": page("Keys"), "guides/index.md": page("Guides") }));
    const site = withChangelog(
      buildSite(join(root, "content/de"), root, { prefix: "/de", labels: { home: "Einführung", overview: "Übersicht" }, version: "1.4.0" }),
      changelog,
      "/de",
      MESSAGES.de,
    );
    expect(site.nav[0]?.items.map((i) => [i.path, i.label])).toEqual([
      ["/de", "Einführung"],
      ["/de/changelog", "Änderungen"],
      ["/de/keys", "Keys"],
    ]);
    const meta = site.metas.find((m) => m.path === "/de/changelog");
    expect(meta).toMatchObject({ title: "Änderungen", prev: { path: "/de", title: "Start" }, next: { path: "/de/keys", title: "Keys" } });
    expect(site.metas[0]?.next).toEqual({ path: "/de/changelog", title: "Änderungen" });
    expect(site.search.at(-1)).toMatchObject({
      path: "/de/changelog",
      title: "Änderungen",
      headings: expect.arrayContaining([{ id: "v1.2.0", label: "1.2.0" }]),
    });
    expect(site.pages.at(-1)?.compiled?.changelog).toBe(changelog.content);
  });

  it("starts the sidebar when there is no home page group", () => {
    const empty = { contentDir: "", pages: [], nav: [], metas: [], search: [], assets: new Set<string>(), problems: [] };
    expect(withChangelog(empty, changelog, "", MESSAGES.en).nav).toEqual([{ id: "pages", items: [{ path: "/changelog", label: "Changelog", depth: 0 }] }]);
  });

  it("is built into every language, and a page at its address is a problem", () => {
    repo(everyLanguage({ "index.md": page("Start"), "changelog.md": page("Changes") }));
    const docs = buildDocs(join(root, "content"), root, "1.4.0", fixture);
    expect(docs.sites["zh-CN"].metas.map((m) => m.path)).toContain("/zh-CN/changelog");
    expect(docs.problems).toEqual([
      "content/en/changelog.md: its address /changelog is the changelog's; rename it",
      "content/de/changelog.md: its address /de/changelog is the changelog's; rename it",
      "content/zh-CN/changelog.md: its address /zh-CN/changelog is the changelog's; rename it",
    ]);
    expect(changelogClash(docs.sites.en, "/fr")).toEqual([]);
  });
});
