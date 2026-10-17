import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import * as prettier from "prettier";
import { history, notes, releasable, release } from "./changelog.mjs";

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
