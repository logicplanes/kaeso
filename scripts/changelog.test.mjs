import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import * as prettier from "prettier";
import { draft, generate, history, notes, releasable, release } from "./changelog.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const repo = "https://github.com/logicplanes/kaeso";

const changelog = (sections, links) =>
  [
    "# Changelog",
    "",
    "All notable changes to Kaeso are documented in this file.",
    "",
    ...sections.flatMap((section) => [section, ""]),
    ...links.map((name) => (name === "Unreleased" ? `[Unreleased]: ${repo}/compare/v${links[1]}...HEAD` : `[${name}]: ${repo}/releases/tag/v${name}`)),
    "",
  ].join("\n");

const first = `## [0.1.0] - 2026-10-16

The first release of Kaeso.

### Added

#### Model API

- OpenAI- and Anthropic-compatible endpoints under \`/v1\`.
- Errors in the app's own format.

#### Console

- Every page in English, German and Simplified Chinese.`;

const withUnreleased = (lines) => `## [Unreleased]${lines ? `\n\n${lines}` : ""}`;

const isPrettier = async (text) => prettier.check(text, { ...(await prettier.resolveConfig(join(root, "CHANGELOG.md"))), filepath: "CHANGELOG.md" });

test("a release moves the lines under [Unreleased] into a dated section and links its tag", () => {
  const before = changelog([withUnreleased("### Fixed\n\n- Streams end cleanly."), first], ["Unreleased", "0.1.0"]);
  assert.equal(
    release(before, "0.1.1", "2026-11-02"),
    changelog(["## [Unreleased]", "## [0.1.1] - 2026-11-02\n\n### Fixed\n\n- Streams end cleanly.", first], ["Unreleased", "0.1.1", "0.1.0"]),
  );
});

test("a release leaves a changelog that already has its section alone", () => {
  const text = changelog([withUnreleased(""), first], ["Unreleased", "0.1.0"]);
  assert.equal(release(text, "0.1.0", "2026-11-02"), text);
});

const cycle = () => {
  let text = changelog([withUnreleased("### Added\n\n#### Model API\n\n- Batches.\n\n### Fixed\n\n- Quiet streams time out."), first], ["Unreleased", "0.1.0"]);
  text = release(text, "0.2.0-rc.1", "2026-11-01");
  text = text.replace(
    "## [Unreleased]\n",
    "## [Unreleased]\n\n### Fixed\n\n- Batches resume.\n\n### Added\n\n#### Console\n\n- A batch page.\n\n#### Model API\n\n- Batch results.\n",
  );
  return release(text, "0.2.0-rc.2", "2026-11-08");
};

test("a pre-release keeps the sections of earlier pre-releases", () => {
  const text = cycle();
  assert.match(
    text,
    /## \[0\.2\.0-rc\.2\] - 2026-11-08\n\n### Added\n\n#### Console\n\n- A batch page\.\n\n#### Model API\n\n- Batch results\.\n\n### Fixed\n\n- Batches resume\.\n\n## \[0\.2\.0-rc\.1\]/,
  );
  assert.match(text, /\[0\.2\.0-rc\.2\]: .+\n\[0\.2\.0-rc\.1\]: .+\n\[0\.1\.0\]: /);
});

test("a stable release folds its pre-releases into one section, merging the same headings and topics, newest first", () => {
  const expected = changelog(
    [
      "## [Unreleased]",
      `## [0.2.0] - 2026-11-15

### Added

#### Console

- A batch page.

#### Model API

- Batch results.
- Batches.

### Changed

- Batches are kept for 30 days.

### Fixed

- Batches resume.
- Quiet streams time out.`,
      first,
    ],
    ["Unreleased", "0.2.0", "0.1.0"],
  );
  const text = cycle().replace("## [Unreleased]\n", "## [Unreleased]\n\n### Changed\n\n- Batches are kept for 30 days.\n");
  assert.equal(release(text, "0.2.0", "2026-11-15"), expected);
});

test("a stable release after pre-releases needs no new lines", () => {
  const text = cycle();
  assert.equal(releasable(text, "0.2.0"), true);
  assert.doesNotMatch(release(text, "0.2.0", "2026-11-15"), /rc\.\d/);
});

test("a release with nothing to release is refused", () => {
  const text = cycle();
  assert.equal(releasable(text, "0.2.0-rc.3"), false);
  assert.equal(releasable(text, "0.2.1"), false);
  assert.equal(releasable(text, "0.2.0-rc.2"), true);
});

test("the notes of a release are its section without the heading", () => {
  const text = release(cycle(), "0.2.0", "2026-11-15");
  assert.match(notes(text, "0.2.0"), /^### Added\n\n#### Console\n\n- A batch page\.\n[\s\S]*- Quiet streams time out\.$/);
  assert.equal(notes(text, "0.1.0"), first.split("\n").slice(2).join("\n"));
  assert.throws(() => notes(text, "0.3.0"), /no section for 0\.3\.0/);
});

test("every changelog a release writes is formatted as Prettier formats it", async () => {
  const real = readFileSync(join(root, "CHANGELOG.md"), "utf8");
  const steps = [
    ["0.2.0-rc.1", "### Added\n\n#### Model API\n\n- Batches.\n"],
    ["0.2.0-rc.2", "### Fixed\n\n- Batches resume.\n\n### Added\n\n#### Console\n\n- A batch page.\n"],
    ["0.2.0", ""],
  ];
  let text = real;
  for (const [version, lines] of steps) {
    text = release(text.replace("## [Unreleased]\n", `## [Unreleased]\n\n${lines}`), version, "2026-11-15");
    assert.equal(await isPrettier(text), true, version);
  }
  assert.match(notes(text, "0.2.0"), /#### Model API\n\n- Batches\./);
  assert.equal(notes(text, "0.1.0"), notes(real, "0.1.0"));
});

const sample = changelog(
  [
    withUnreleased("### Added\n\n#### Console\n\n### Fixed"),
    `## [0.3.0-rc.1] - 2026-12-01

### Added

- Batches, which run:
  - in the background,
  - with [their own limits][limits].`,
    "## [0.2.1] - 2026-11-20 [YANKED]\n\n### Fixed\n\n- Streams end cleanly.",
    `## [0.2.0] - 2026-11-15

A release about batches.

### Added

- A batch page.

#### Model API

- Batch results that span
  two lines.

### Added

- A second list under the same heading.

### Security

- Keys are checked twice.`,
    first,
  ],
  ["Unreleased", "0.3.0-rc.1", "0.2.1", "0.2.0", "0.1.0"],
).replace("[0.2.0]: ", "[limits]: https://docs.kaeso.app/console/limits\n[0.2.0]: ");

test("the history leaves out an [Unreleased] section with nothing but headings", () => {
  assert.equal(history(sample).unreleased, null);
  assert.equal(history(changelog([withUnreleased(""), first], ["Unreleased", "0.1.0"])).unreleased, null);
});

const lineOf = (text, line) => text.split("\n").indexOf(line) + 1;

test("the history keeps lines under [Unreleased] apart from every release, with the compare view since the newest one", () => {
  const text = sample.replace("### Fixed\n", "### Fixed\n\n- Quiet streams time out.\n");
  assert.deepEqual(history(text).unreleased, {
    id: "unreleased",
    intro: "",
    introLine: lineOf(text, "## [Unreleased]") + 2,
    sections: [{ name: "Fixed", id: "unreleased-fixed", markdown: "- Quiet streams time out.", line: lineOf(text, "- Quiet streams time out."), topics: [] }],
    since: "0.3.0-rc.1",
    compare: `${repo}/compare/v0.3.0-rc.1...HEAD`,
  });
});

const summary = ({ id, version, date, prerelease, yanked, latest, previous, url, compare }) => ({
  id,
  version,
  date,
  prerelease,
  yanked,
  latest,
  previous,
  url,
  compare,
});

test("every release has a stable anchor, its date, its release and the compare view against the release before it", () => {
  const { repository, releases } = history(sample);
  assert.equal(repository, repo);
  assert.deepEqual(releases.map(summary), [
    {
      id: "v0.3.0-rc.1",
      version: "0.3.0-rc.1",
      date: "2026-12-01",
      prerelease: true,
      yanked: false,
      latest: false,
      previous: "0.2.1",
      url: `${repo}/releases/tag/v0.3.0-rc.1`,
      compare: `${repo}/compare/v0.2.1...v0.3.0-rc.1`,
    },
    {
      id: "v0.2.1",
      version: "0.2.1",
      date: "2026-11-20",
      prerelease: false,
      yanked: true,
      latest: false,
      previous: "0.2.0",
      url: `${repo}/releases/tag/v0.2.1`,
      compare: `${repo}/compare/v0.2.0...v0.2.1`,
    },
    {
      id: "v0.2.0",
      version: "0.2.0",
      date: "2026-11-15",
      prerelease: false,
      yanked: false,
      latest: true,
      previous: "0.1.0",
      url: `${repo}/releases/tag/v0.2.0`,
      compare: `${repo}/compare/v0.1.0...v0.2.0`,
    },
    {
      id: "v0.1.0",
      version: "0.1.0",
      date: "2026-10-16",
      prerelease: false,
      yanked: false,
      latest: false,
      previous: null,
      url: `${repo}/releases/tag/v0.1.0`,
      compare: null,
    },
  ]);
});

test("a release keeps its introduction, its sections and their topics, with the Markdown of every entry as written", () => {
  const { releases } = history(sample);
  const release = releases.find(({ version }) => version === "0.2.0");
  assert.equal(release.intro, "A release about batches.");
  assert.equal(release.introLine, lineOf(sample, "A release about batches."));
  assert.deepEqual(release.sections, [
    {
      name: "Added",
      id: "v0.2.0-added",
      markdown: "- A batch page.",
      line: lineOf(sample, "- A batch page."),
      topics: [
        {
          name: "Model API",
          id: "v0.2.0-added-model-api",
          markdown: "- Batch results that span\n  two lines.",
          line: lineOf(sample, "- Batch results that span"),
        },
      ],
    },
    {
      name: "Added",
      id: "v0.2.0-added-2",
      markdown: "- A second list under the same heading.",
      line: lineOf(sample, "- A second list under the same heading."),
      topics: [],
    },
    { name: "Security", id: "v0.2.0-security", markdown: "- Keys are checked twice.", line: lineOf(sample, "- Keys are checked twice."), topics: [] },
  ]);
  assert.equal(releases[0].sections[0].markdown, "- Batches, which run:\n  - in the background,\n  - with [their own limits][limits].");
});

test("the history hands on the link definitions, so reference links in entries resolve", () => {
  assert.match(
    history(sample).definitions,
    /^\[Unreleased\]: .+\n\[0\.3\.0-rc\.1\]: [\s\S]*\n\[limits\]: https:\/\/docs\.kaeso\.app\/console\/limits\n\[0\.2\.0\]: /,
  );
});

test("a changelog without releases has no history, and one without an [Unreleased] link no addresses", () => {
  assert.deepEqual(history("# Changelog\n"), { repository: null, definitions: "", definitionsLine: null, unreleased: null, releases: [] });
  const [only] = history(`# Changelog\n\n${first}\n`).releases;
  assert.equal(only.url, null);
  assert.equal(only.compare, null);
  assert.equal(only.latest, true);
});

test("every release in CHANGELOG.md has a date and the anchor v and its version, and one is the latest", () => {
  const { releases } = history(readFileSync(join(root, "CHANGELOG.md"), "utf8"));
  assert.ok(releases.length > 0);
  for (const { version, date, id } of releases) {
    assert.match(date ?? "", /^\d{4}-\d{2}-\d{2}$/, version);
    assert.equal(id, `v${version}`);
  }
  assert.equal(releases.filter(({ latest }) => latest).length, 1);
});

const fragments = (text) => {
  const log = history(text);
  const entries = [...(log.unreleased ? [log.unreleased] : []), ...log.releases];
  return entries.flatMap((entry) => [
    { markdown: entry.intro, line: entry.introLine },
    ...entry.sections.flatMap((section) => [{ markdown: section.markdown, line: section.line }, ...section.topics]),
  ]);
};

test("every fragment the history hands on says the line of the file where it starts, and so do the definitions", () => {
  for (const text of [sample, readFileSync(join(root, "CHANGELOG.md"), "utf8"), sample.replace("### Fixed\n", "### Fixed\n\n\n- Quiet streams time out.\n")]) {
    const lines = text.split("\n");
    for (const { markdown, line } of fragments(text)) {
      if (markdown) assert.equal(lines.slice(line - 1, line - 1 + markdown.split("\n").length).join("\n"), markdown);
    }
    const { definitions, definitionsLine } = history(text);
    assert.equal(lines[definitionsLine - 1], definitions.split("\n")[0]);
  }
});

const pull = (number, title, labels = [], author = "devin") => ({ number, title, url: `${repo}/pull/${number}`, labels, author });
const ref = (number) => `[#${number}](${repo}/pull/${number})`;

test("merged pull requests become entries under the section of their type label, in the order of Keep a Changelog and then by number", () => {
  const pulls = [
    pull(12, "Keep streams alive behind proxies", ["security"]),
    pull(9, "Refuse empty batches", ["fixed"]),
    pull(4, "Batch requests", ["added"]),
    pull(7, "Sunset the v0 keys", ["deprecated"]),
    pull(8, "Retire the legacy export", ["removed"]),
    pull(3, "Batch results", ["added"]),
    pull(5, "Show costs in cents", ["changed"]),
  ];
  assert.deepEqual(generate(pulls), [
    "### Added",
    "",
    `- Batch results (${ref(3)})`,
    `- Batch requests (${ref(4)})`,
    "",
    "### Changed",
    "",
    `- Show costs in cents (${ref(5)})`,
    "",
    "### Deprecated",
    "",
    `- Sunset the v0 keys (${ref(7)})`,
    "",
    "### Removed",
    "",
    `- Retire the legacy export (${ref(8)})`,
    "",
    "### Fixed",
    "",
    `- Refuse empty batches (${ref(9)})`,
    "",
    "### Security",
    "",
    `- Keep streams alive behind proxies (${ref(12)})`,
  ]);
});

test("without a type label, the title's first word chooses the section, and anything else is a change", () => {
  const section = (title) => generate([pull(1, title)])[0];
  assert.equal(section("Add batch requests"), "### Added");
  assert.equal(section("Introduce batches"), "### Added");
  assert.equal(section("Deprecate the v0 keys"), "### Deprecated");
  assert.equal(section("Remove the legacy export"), "### Removed");
  assert.equal(section("Drop Node.js 22"), "### Removed");
  assert.equal(section("fix quiet streams"), "### Fixed");
  assert.equal(section("Keep streams from failing"), "### Changed");
  assert.equal(section("Fixes are listed"), "### Changed");
  assert.equal(section("Additions to the catalogue"), "### Changed");
  assert.equal(section("Fix: quiet streams"), "### Fixed");
  assert.equal(section("`Add` button keeps focus"), "### Changed");
});

test("a type label wins over the title, and of several type labels security comes first, then the order of Keep a Changelog", () => {
  assert.equal(generate([pull(1, "Fix the export", ["changed"])])[0], "### Changed");
  assert.equal(generate([pull(1, "Add a check", ["fixed", "security"])])[0], "### Security");
  assert.equal(generate([pull(1, "Rework limits", ["fixed", "removed", "added"])])[0], "### Added");
});

test("one area label puts an entry under its topic, after the entries without one; none or several leave it without a topic", () => {
  const pulls = [
    pull(6, "Add a docs search", ["area: documentation"]),
    pull(5, "Add a batch page", ["area: console"]),
    pull(4, "Add batches", ["area: gateway", "area: console"]),
    pull(3, "Add a KPP batch type", ["area: kpp"]),
    pull(2, "Add a theme", ["area: ui library"]),
    pull(1, "Add a Compose profile", ["area: self-hosting"]),
    pull(7, "Add request tags", ["area: gateway"]),
  ];
  assert.deepEqual(generate(pulls), [
    "### Added",
    "",
    `- Add batches (${ref(4)})`,
    "",
    "#### Gateway",
    "",
    `- Add request tags (${ref(7)})`,
    "",
    "#### KPP",
    "",
    `- Add a KPP batch type (${ref(3)})`,
    "",
    "#### Console",
    "",
    `- Add a batch page (${ref(5)})`,
    "",
    "#### UI library",
    "",
    `- Add a theme (${ref(2)})`,
    "",
    "#### Documentation",
    "",
    `- Add a docs search (${ref(6)})`,
    "",
    "#### Self-hosting",
    "",
    `- Add a Compose profile (${ref(1)})`,
  ]);
});

test("internal pull requests are left out, Dependabot's included", () => {
  assert.deepEqual(generate([pull(1, "Speed up CI", ["internal"]), pull(2, "Bump actions/checkout", ["dependencies", "internal"], "dependabot[bot]")]), []);
});

test("Dependabot's pull requests fold into one line under Changed, after the other changes", () => {
  const pulls = [
    pull(9, "Bump serde from 1.0.1 to 1.0.2", ["dependencies", "rust", "area: gateway"], "dependabot[bot]"),
    pull(4, "Bump the pnpm-minor-and-patch group with 3 updates", ["javascript", "fixed"], "dependabot[bot]"),
    pull(6, "Update ClickHouse to 26.9", ["dependencies"]),
    pull(5, "Show costs in cents"),
  ];
  assert.deepEqual(generate(pulls), ["### Changed", "", `- Show costs in cents (${ref(5)})`, `- Dependencies updated (${ref(4)}, ${ref(6)}, ${ref(9)})`]);
});

test("a breaking pull request is also named in a note at the top of the release", () => {
  assert.deepEqual(generate([pull(3, "Rename the budget fields", ["breaking", "area: gateway"]), pull(2, "Add batches")]), [
    `- **Breaking:** Rename the budget fields (${ref(3)})`,
    "",
    "### Added",
    "",
    `- Add batches (${ref(2)})`,
    "",
    "### Changed",
    "",
    "#### Gateway",
    "",
    `- Rename the budget fields (${ref(3)})`,
  ]);
});

test("a title can't write HTML, links, headings or nested lists into the changelog, and keeps its code spans", () => {
  const [, , line] = generate([pull(1, "Keep <Select> and [the menu](javascript:alert(1)) open with `<Dialog>` in C:\\temp.")]);
  assert.equal(line, `- Keep \\<Select\\> and \\[the menu\\](javascript:alert(1)) open with \`<Dialog>\` in C:\\\\temp (${ref(1)})`);
  assert.equal(generate([pull(1, "# Batches")])[2], `- \\# Batches (${ref(1)})`);
  assert.equal(generate([pull(1, "- Batches")])[2], `- \\- Batches (${ref(1)})`);
  assert.equal(generate([pull(1, "2. Batches")])[2], `- 2\\. Batches (${ref(1)})`);
});

test("each pull request counts once, and one without a number, title, https address or labels is refused", () => {
  assert.deepEqual(generate([pull(2, "Add batches"), pull(2, "Add batches")]), ["### Added", "", `- Add batches (${ref(2)})`]);
  assert.throws(() => generate({}), /must be a JSON array/);
  assert.throws(() => generate([{ title: "Add batches", url: `${repo}/pull/1`, labels: [] }]), /pull request 1 has no number/);
  assert.throws(() => generate([{ ...pull(1, "x"), title: " " }]), /#1 has no title/);
  assert.throws(() => generate([{ ...pull(1, "x"), url: "javascript:alert(1)" }]), /#1 has no https address/);
  assert.throws(() => generate([{ ...pull(1, "x"), labels: "fixed" }]), /#1 has no list of labels/);
});

const merged = [pull(2, "Add batches", ["area: gateway"]), pull(3, "Fix quiet streams"), pull(4, "Speed up CI", ["internal"])];

test("a release with nothing under [Unreleased] writes its section from the pull requests merged since the last release", () => {
  const before = changelog([withUnreleased(""), first], ["Unreleased", "0.1.0"]);
  const after = release(before, "0.2.0", "2026-11-15", merged);
  assert.equal(
    after,
    changelog(
      [
        "## [Unreleased]",
        `## [0.2.0] - 2026-11-15\n\n### Added\n\n#### Gateway\n\n- Add batches (${ref(2)})\n\n### Fixed\n\n- Fix quiet streams (${ref(3)})`,
        first,
      ],
      ["Unreleased", "0.2.0", "0.1.0"],
    ),
  );
  assert.equal(notes(after, "0.2.0"), generate(merged).join("\n"));
});

const curatedLog = (lines) => changelog([withUnreleased(lines), first], ["Unreleased", "0.1.0"]);

test("lines written under [Unreleased] alone make the release when no pull request was merged", () => {
  const before = curatedLog("### Fixed\n\n- Streams end cleanly.");
  assert.equal(notes(release(before, "0.2.0", "2026-11-15"), "0.2.0"), "### Fixed\n\n- Streams end cleanly.");
  assert.equal(notes(release(before, "0.2.0", "2026-11-15", [pull(4, "Speed up CI", ["internal"])]), "0.2.0"), "### Fixed\n\n- Streams end cleanly.");
  const headings = curatedLog("### Added\n\n### Fixed");
  assert.equal(notes(release(headings, "0.2.0", "2026-11-15", merged), "0.2.0"), generate(merged).join("\n"));
});

test("lines written under [Unreleased] come first, and every pull request none of them links follows under its own section and topic", async () => {
  const before = curatedLog(
    `A release about batches.\n\n### Added\n\n#### Console\n\n- A page for batches, with their status. ([#5](${repo}/pull/5))\n\n### Fixed\n\n- Streams end cleanly.`,
  );
  const pulls = [
    ...merged,
    pull(5, "Add a batch page", ["area: console"]),
    pull(6, "Rename the budget fields", ["breaking", "area: gateway"]),
    pull(7, "Add batch exports", ["area: console"]),
  ];
  const expected = [
    "A release about batches.",
    "",
    `- **Breaking:** Rename the budget fields (${ref(6)})`,
    "",
    "### Added",
    "",
    "#### Console",
    "",
    `- A page for batches, with their status. ([#5](${repo}/pull/5))`,
    `- Add batch exports (${ref(7)})`,
    "",
    "#### Gateway",
    "",
    `- Add batches (${ref(2)})`,
    "",
    "### Changed",
    "",
    "#### Gateway",
    "",
    `- Rename the budget fields (${ref(6)})`,
    "",
    "### Fixed",
    "",
    "- Streams end cleanly.",
    `- Fix quiet streams (${ref(3)})`,
  ];
  assert.deepEqual(draft(before, "0.2.0", pulls), expected);
  const after = release(before, "0.2.0", "2026-11-15", pulls);
  assert.equal(notes(after, "0.2.0"), expected.join("\n"));
  assert.equal(await isPrettier(after), true);
});

test("with lines under [Unreleased], internal pull requests stay out and Dependabot's that no line links fold into one line under Changed", () => {
  const before = curatedLog(`### Changed\n\n- Serde 1.0.2 parses large numbers exactly. ([#9](${repo}/pull/9))`);
  const pulls = [
    pull(4, "Speed up CI", ["internal"]),
    pull(9, "Bump serde from 1.0.1 to 1.0.2", ["dependencies"], "dependabot[bot]"),
    pull(10, "Bump vite from 8.0.1 to 8.0.2", ["dependencies"], "dependabot[bot]"),
    pull(11, "Update ClickHouse to 26.9", ["dependencies"]),
    pull(12, "Show costs in cents"),
  ];
  assert.deepEqual(draft(before, "0.2.0", pulls), [
    "### Changed",
    "",
    `- Serde 1.0.2 parses large numbers exactly. ([#9](${repo}/pull/9))`,
    `- Show costs in cents (${ref(12)})`,
    `- Dependencies updated (${ref(10)}, ${ref(11)})`,
  ]);
  assert.deepEqual(draft(before, "0.2.0", [pull(4, "Speed up CI", ["internal"])]), [
    "### Changed",
    "",
    `- Serde 1.0.2 parses large numbers exactly. ([#9](${repo}/pull/9))`,
  ]);
});

test("only a link to the pull request itself covers it, not a link to an issue or another pull request with its number as text", () => {
  const before = curatedLog(`### Fixed\n\n- Streams end cleanly. ([#3](${repo}/issues/3), [#2](${repo}/pull/20))`);
  assert.deepEqual(draft(before, "0.2.0", merged), [
    "### Added",
    "",
    "#### Gateway",
    "",
    `- Add batches (${ref(2)})`,
    "",
    "### Fixed",
    "",
    `- Streams end cleanly. ([#3](${repo}/issues/3), [#2](${repo}/pull/20))`,
    `- Fix quiet streams (${ref(3)})`,
  ]);
});

test("a stable release lists a pull request that a pre-release already names only once", () => {
  let text = curatedLog("");
  text = release(text, "0.2.0-rc.1", "2026-11-01", [pull(2, "Add batches", ["area: gateway"])]);
  text = text.replace("## [Unreleased]\n", "## [Unreleased]\n\n### Fixed\n\n- Streams end cleanly.\n");
  assert.deepEqual(draft(text, "0.2.0", [pull(2, "Add batches", ["area: gateway"]), pull(3, "Fix quiet streams")]), [
    "### Added",
    "",
    "#### Gateway",
    "",
    `- Add batches (${ref(2)})`,
    "",
    "### Fixed",
    "",
    "- Streams end cleanly.",
    `- Fix quiet streams (${ref(3)})`,
  ]);
});

test("a release with only internal pull requests and nothing under [Unreleased] is refused", () => {
  const before = changelog([withUnreleased(""), first], ["Unreleased", "0.1.0"]);
  assert.equal(releasable(before, "0.2.0"), false);
  assert.equal(releasable(before, "0.2.0", [pull(4, "Speed up CI", ["internal"])]), false);
  assert.equal(releasable(before, "0.2.0", merged), true);
  assert.equal(releasable(changelog([first], ["Unreleased", "0.1.0"]), "0.2.0", merged), true);
});

test("a stable release folds the generated sections of its pre-releases with the pull requests merged after them", () => {
  let text = changelog([withUnreleased(""), first], ["Unreleased", "0.1.0"]);
  text = release(text, "0.2.0-rc.1", "2026-11-01", [pull(2, "Add batches", ["area: gateway"])]);
  text = release(text, "0.2.0", "2026-11-15", [pull(5, "Add a batch page", ["area: console"]), pull(6, "Fix batch retries")]);
  assert.equal(
    notes(text, "0.2.0"),
    `### Added\n\n#### Console\n\n- Add a batch page (${ref(5)})\n\n#### Gateway\n\n- Add batches (${ref(2)})\n\n### Fixed\n\n- Fix batch retries (${ref(6)})`,
  );
  assert.doesNotMatch(text, /rc\.1/);
});

test("a generated release is formatted as Prettier formats it", async () => {
  const real = readFileSync(join(root, "CHANGELOG.md"), "utf8").replace(/## \[Unreleased\]\n[\s\S]*?(?=## \[)/, "## [Unreleased]\n\n");
  const pulls = [
    ...merged,
    pull(5, "Keep <Select> open with `<Dialog>` [inside] C:\\temp", ["breaking", "security"]),
    pull(6, "Bump serde", ["dependencies"], "dependabot[bot]"),
    pull(7, "Add a batch page", ["area: console"]),
  ];
  const text = release(real, "0.2.0", "2026-11-15", pulls);
  assert.match(text, /## \[0\.2\.0\] - 2026-11-15\n\n- \*\*Breaking:\*\* Keep/);
  assert.equal(await isPrettier(text), true);
});

test("the draft command prints the section that the pull requests in a file make", () => {
  const dir = mkdtempSync(join(tmpdir(), "kaeso-changelog-"));
  try {
    writeFileSync(join(dir, "pulls.json"), JSON.stringify(merged));
    const out = execFileSync(process.execPath, [join(root, "scripts", "changelog.mjs"), "draft", join(dir, "pulls.json")], { encoding: "utf8" });
    assert.equal(out, `${generate(merged).join("\n")}\n`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the Release workflow writes the changelog from the merged pull requests, and its Tag job accepts changes to CHANGELOG.md and the version's files only", () => {
  const workflow = readFileSync(join(root, ".github", "workflows", "release.yml"), "utf8");
  const allowed = workflow
    .split("\n")
    .filter((line) => line.includes('> "$allowed"'))
    .map((line) => line.trim());
  assert.deepEqual(allowed, ['{ node scripts/version.mjs files; echo CHANGELOG.md; } > "$allowed"']);
  assert.match(workflow, /git apply --summary "\$patch"/);
  assert.match(workflow, /node scripts\/changelog\.mjs check "\$version" "\$pulls"/);
  assert.match(workflow, /node scripts\/changelog\.mjs release "\$VERSION" "\$\(date -u \+%F\)" "\$RUNNER_TEMP\/pulls\/pulls\.json" "\$\{notes\[@\]\}"/);
});

test("a draft is the section a release would write, without changing the changelog", () => {
  const before = changelog([withUnreleased(""), first], ["Unreleased", "0.1.0"]);
  assert.equal(draft(before, "0.2.0", merged).join("\n"), notes(release(before, "0.2.0", "2026-11-15", merged), "0.2.0"));
  const curated = changelog([withUnreleased("### Fixed\n\n- Streams end cleanly."), first], ["Unreleased", "0.1.0"]);
  assert.equal(draft(curated, "0.2.0", merged).join("\n"), notes(release(curated, "0.2.0", "2026-11-15", merged), "0.2.0"));
});

test("only the step that writes the release notes sees LOG_AI_KEY, in a job that can only read the repository, and a failure there never stops the release", () => {
  const workflow = readFileSync(join(root, ".github", "workflows", "release.yml"), "utf8");
  const uses = workflow.split("\n").filter((line) => line.includes("LOG_AI_KEY"));
  assert.deepEqual(
    uses.map((line) => line.trim()),
    ["LOG_AI_KEY: ${{ secrets.LOG_AI_KEY }}"],
  );
  const prepare = workflow.slice(workflow.indexOf("\n  prepare:"), workflow.indexOf("\n  checks:"));
  assert.match(prepare, /\n {4}permissions:\n {6}contents: read\n {4}env:/);
  const step = prepare.slice(prepare.indexOf("- name: Write the release notes"), prepare.indexOf("- name: Write the changelog"));
  assert.match(step, /continue-on-error: true/);
  assert.match(step, /timeout-minutes: 20/);
  assert.match(step, /RELEASE_NOTES_API_URL: https:\/\/ai\.logicplanes\.com/);
  assert.match(
    step,
    /node scripts\/release-notes\.mjs write --pulls "\$RUNNER_TEMP\/pulls\/pulls\.json" --version "\$VERSION" --out "\$RUNNER_TEMP\/notes\/notes\.md"/,
  );
});
