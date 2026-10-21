import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { MESSAGES } from "../../src/i18n/messages/index.ts";
import { release } from "../../../../scripts/changelog.mjs";
import type { Block } from "../../src/markdown/ast.ts";
import { buildChangelog, changelogClash, changelogPages, changelogToc, withChangelog } from "./changelog.ts";
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

describe("a changelog with markup that could run", () => {
  const log = (entry: string) => `# Changelog\n\n## [Unreleased]\n\n## [1.0.0] - 2026-09-01\n\n### Added\n\n${entry}\n`;
  const FIRST = 9;
  const problemsOf = (entry: string) => {
    repo({ "CHANGELOG.md": log(entry) });
    return buildChangelog(join(root, "CHANGELOG.md"), root).problems;
  };
  const refused = (line: number) => [`CHANGELOG.md: line ${line}: raw HTML is not allowed; write it in a code span or remove it`];

  it("fails the build with the file and the line instead of drawing raw HTML", () => {
    repo({ "CHANGELOG.md": log('- A <img src=x onerror="alert(1)"> picture.') });
    const built = buildChangelog(join(root, "CHANGELOG.md"), root);
    expect(built.problems).toEqual(refused(FIRST));
  });

  it("refuses tags, comments, processing instructions, declarations and HTML blocks", () => {
    expect(problemsOf("- A <script>alert(1)</script> script.")).toEqual(refused(FIRST));
    expect(problemsOf('- Fine.\n\n<div onclick="x()">block</div>')).toEqual(refused(FIRST + 2));
    expect(problemsOf("- A <!-- hidden --> note.")).toEqual(refused(FIRST));
    expect(problemsOf("- Fine.\n\n<!--\nhidden\n-->")).toEqual(refused(FIRST + 2));
    expect(problemsOf("- A <?php echo 1; ?> instruction.")).toEqual(refused(FIRST));
    expect(problemsOf("<!DOCTYPE html>")).toEqual(refused(FIRST));
    expect(problemsOf("<table>\n<tr><td>cell</td></tr>\n</table>")).toEqual(refused(FIRST));
  });

  it("refuses a tag whose attributes continue on the next lines, at the line it starts on", () => {
    expect(problemsOf("- Fine.\n\n- A picture <img\n  src=x\n  onerror=alert(1)> here.")).toEqual(refused(FIRST + 2));
    expect(problemsOf("A picture <img\nsrc=x onerror=alert(1)>")).toEqual(refused(FIRST));
  });

  it("refuses a tag after an escaped backtick, which opens no code span", () => {
    expect(problemsOf("- A \\`<img src=x onerror=alert(1)>\\` picture.")).toEqual(refused(FIRST));
    expect(problemsOf("- A \\`<script>alert(1)</script>\\`")).toEqual(refused(FIRST));
  });

  it("refuses a tag after four spaces that only look like a fence, which is an indented code block only after a blank line", () => {
    expect(problemsOf("Text\n    ```\n<script>alert(1)</script>\n    ```")).toEqual(refused(FIRST + 2));
    expect(problemsOf("    ```\n\n<div>x</div>\n\n    ```")).toEqual(refused(FIRST + 2));
  });

  it("refuses a tag after a code span that a backslash does not escape, since code spans take no escapes", () => {
    expect(problemsOf("- The `a \\` <i>` span.")).toEqual(refused(FIRST));
  });

  it("refuses raw HTML in a quote and in a table cell", () => {
    expect(problemsOf("> A <b>quote</b>")).toEqual(refused(FIRST));
    expect(problemsOf("| a | b |\n| - | - |\n| x | <b>y</b> |")).toEqual(refused(FIRST + 2));
  });

  it("fails the build for a link that is not http, https or mailto", () => {
    expect(problemsOf("- A [link](javascript:alert(1)).")).toEqual([
      'CHANGELOG.md: [1.0.0] links to "javascript:alert(1)": only http, https and mailto addresses, and relative links to files in content/, are allowed',
    ]);
    expect(problemsOf("- A [link][r].\n\n[r]: data:text/html,x")).toEqual([
      'CHANGELOG.md: [1.0.0] links to "data:text/html,x": only http, https and mailto addresses, and relative links to files in content/, are allowed',
    ]);
  });

  it("checks each fragment as it is drawn: markup that is code in the whole file but not alone is refused, and the other way round is accepted", () => {
    const fenceOverHeading = (inside: string, after: string) => `- Fine.\n\n\`\`\`\ncode\n\n### Fixed\n\n${inside}\n\`\`\`${after}`;
    expect(problemsOf(fenceOverHeading("<b>x</b>", ""))).toEqual(refused(FIRST + 7));
    expect(problemsOf("```\ncode\n\n### Fixed\n\n```\n\n<b>x</b>")).toEqual([]);
  });

  it("names the line in the file for a fragment under a heading of its own, in a topic and in an older release", () => {
    repo({
      "CHANGELOG.md":
        "# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n#### Console\n\n- <i>Fine</i>.\n\n## [1.0.0] - 2026-09-01\n\nIntro.\n\n### Added\n\n- Fine.\n\n- <u>x</u>\n",
    });
    expect(buildChangelog(join(root, "CHANGELOG.md"), root).problems).toEqual(refused(19).concat(refused(9)));
  });

  it("accepts HTML written as code, in fences, in indented code, as an autolink or as an entity", () => {
    const entry = [
      "- The `<meta>` tag and ``<style>`` element.",
      "- See <https://example.com> and &lt;b&gt;, 1 < 2 and 3 > 2.",
      "",
      "  ```html",
      "  <script>alert(1)</script>",
      "  ```",
      "",
      "      <b>indented code</b>",
      "",
      "~~~",
      "<div>fenced</div>",
      "~~~",
    ].join("\n");
    expect(problemsOf(entry)).toEqual([]);
  });
});

describe("a release written from pull requests", () => {
  const pull = (number: number, title: string, labels: string[]) => ({ number, title, url: `${repoUrl}/pull/${number}`, labels });
  const before = `# Changelog\n\n## [Unreleased]\n\n## [1.0.0] - 2026-09-01\n\n### Added\n\n- Batches.\n\n[Unreleased]: ${repoUrl}/compare/v1.0.0...HEAD\n[1.0.0]: ${repoUrl}/releases/tag/v1.0.0\n`;

  it("builds without problems and shows markup in a title as text, with the link to its pull request", () => {
    const written = release(before, "1.1.0", "2026-10-01", [
      pull(7, "Keep <Select> and [the menu](javascript:alert(1)) open around `<Dialog>`", ["fixed", "area: console"]),
      pull(8, "Rename the budget fields", ["breaking"]),
    ]);
    repo({ "CHANGELOG.md": written });
    const { content, problems } = buildChangelog(join(root, "CHANGELOG.md"), root);
    expect(problems).toEqual([]);
    const [latest] = content.releases;
    expect(latest?.version).toBe("1.1.0");
    expect(text(latest?.intro ?? [])).toContain(`${repoUrl}/pull/8`);
    expect(latest?.sections.map((s) => [s.name, s.topics.map((t) => t.name)])).toEqual([
      ["Changed", []],
      ["Fixed", ["Console"]],
    ]);
    const fixed = text(latest?.sections[1]?.topics[0]?.blocks ?? []);
    expect(fixed).toContain("<Select>");
    expect(fixed).toContain("javascript:alert(1)");
    expect(fixed).toContain(`${repoUrl}/pull/7`);
    expect(fixed).not.toContain('"href":"javascript');
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
    expect(site.search.slice(-3).map((e) => [e.path, e.title, e.headings.map((h) => h.label)])).toEqual([
      ["/de/changelog", "Änderungen", ["Unveröffentlicht", "1.3.0-rc.1", "1.2.0"]],
      ["/de/changelog/1.1", "Änderungen 1.1", ["1.1.1", "1.1.0"]],
      ["/de/changelog/1.0", "Änderungen 1.0", ["1.0.0"]],
    ]);
    expect(site.search.at(-1)?.words).toContain("stable");
    expect(site.metas.slice(-2)).toEqual([
      {
        path: "/de/changelog/1.1",
        title: "Änderungen 1.1",
        description: "Jede spürbare Änderung in den Releases 1.1.",
        label: "Änderungen 1.1",
        file: changelog.rel,
      },
      {
        path: "/de/changelog/1.0",
        title: "Änderungen 1.0",
        description: "Jede spürbare Änderung in den Releases 1.0.",
        label: "Änderungen 1.0",
        file: changelog.rel,
      },
    ]);
    expect(site.pages.slice(-3).map((p) => p.compiled?.changelog?.releases.map((r) => r.version))).toEqual([
      ["1.3.0-rc.1", "1.2.0"],
      ["1.1.1", "1.1.0"],
      ["1.0.0"],
    ]);
  });

  it("splits the releases into one page per minor series, with links between the pages", () => {
    const pages = changelogPages(changelog.content, "/changelog");
    expect(pages.map((p) => [p.path, p.series, p.content.releases.map((r) => r.version), !!p.content.unreleased])).toEqual([
      ["/changelog", undefined, ["1.3.0-rc.1", "1.2.0"], true],
      ["/changelog/1.1", "1.1", ["1.1.1", "1.1.0"], false],
      ["/changelog/1.0", "1.0", ["1.0.0"], false],
    ]);
    const [latest, older] = pages;
    expect(latest?.content.pages).toEqual([{ path: "/changelog" }, { path: "/changelog/1.1", series: "1.1" }, { path: "/changelog/1.0", series: "1.0" }]);
    expect(latest?.content.elsewhere).toMatchObject({ "v1.0.0": "/changelog/1.0", "v1.1.0": "/changelog/1.1" });
    expect(latest?.content.elsewhere).not.toHaveProperty("v1.2.0");
    expect(older?.content.elsewhere).toMatchObject({ unreleased: "/changelog", "v1.2.0-added": "/changelog" });
    expect(text(older?.content.releases[1]?.sections.flatMap((s) => s.blocks) ?? [])).toContain('"kind":"page","href":"/changelog/1.0#v1.0.0"');
    expect(latest?.content.repository).toBe(repoUrl);
  });

  it("points a link to a version on another page at that page, wherever the link sits", () => {
    const link = (id: string) => ({ t: "link" as const, kind: "anchor" as const, href: `#${id}`, c: [{ t: "strong" as const, c: [{ t: "text" as const, v: id }] }] });
    const inlines = [link("v0.1.0"), link("v0.2.0"), { t: "em" as const, c: [link("v0.1.0")] }, { t: "code" as const, v: "x" }];
    const blocks: Block[] = [
      { t: "heading", level: 4, id: "h", c: inlines },
      { t: "paragraph", c: inlines },
      { t: "list", ordered: false, start: 1, items: [{ c: [{ t: "paragraph", c: inlines }] }] },
      { t: "quote", c: [{ t: "paragraph", c: inlines }] },
      { t: "callout", kind: "note", c: [{ t: "paragraph", c: inlines }] },
      { t: "table", align: ["start"], head: [inlines], rows: [[inlines]] },
      {
        t: "links",
        items: [
          { kind: "anchor", href: "#v0.1.0", title: inlines, description: inlines },
          { kind: "anchor", href: "#v0.2.0", title: [], description: [] },
          { kind: "page", href: "/guides", title: [], description: [] },
        ],
      },
      { t: "rule" },
    ];
    const entry = (version: string, latest: boolean) => ({
      id: `v${version}`,
      version,
      prerelease: false,
      yanked: false,
      latest,
      intro: blocks,
      sections: [{ name: "Fixed", group: "fixed" as const, id: `v${version}-fixed`, blocks, topics: [{ name: "Console", id: `v${version}-fixed-console`, blocks }] }],
    });
    const [latest, older] = changelogPages({ releases: [entry("0.2.0", true), entry("0.1.0", false)] }, "/de/changelog");
    const drawn = text(latest?.content.releases[0]?.intro ?? []);
    expect(drawn.split('"kind":"page","href":"/de/changelog/0.1#v0.1.0"')).toHaveLength(1 + 7 * 2 + 5);
    expect(drawn).toContain('"kind":"anchor","href":"#v0.2.0"');
    expect(drawn).toContain('"kind":"page","href":"/guides"');
    expect(text(latest?.content.releases[0]?.sections[0]?.topics[0]?.blocks ?? [])).toBe(drawn);
    expect(text(older?.content.releases[0]?.intro ?? [])).toContain('"kind":"page","href":"/de/changelog#v0.2.0"');
  });

  it("keeps a changelog of one series, or none, on a single page", () => {
    const one = buildChangelog(fixture, docsRoot).content;
    const single = { ...one, releases: one.releases.filter((r) => r.version.startsWith("1.1.")) };
    expect(changelogPages(single, "/changelog").map((p) => [p.path, p.content.releases.length, p.content.pages])).toEqual([["/changelog", 2, undefined]]);
    expect(changelogPages({ releases: [] }, "/changelog")).toEqual([{ path: "/changelog", content: { releases: [] } }]);
  });

  it("starts the sidebar when there is no home page group", () => {
    const empty = { contentDir: "", pages: [], nav: [], metas: [], search: [], assets: new Set<string>(), diagrams: [], problems: [] };
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
    repo(everyLanguage({ "index.md": page("Start"), "changelog/index.md": page("Changes"), "changelog/notes.md": page("Notes") }));
    expect(buildDocs(join(root, "content"), root, "1.4.0", fixture).problems).toContain(
      "content/en/changelog/notes.md: its address /changelog/notes is the changelog's; rename it",
    );
  });
});
