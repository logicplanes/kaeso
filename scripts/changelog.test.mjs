import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import * as prettier from "prettier";
import { notes, releasable, release } from "./changelog.mjs";

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
