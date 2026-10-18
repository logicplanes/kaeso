import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, before, test } from "node:test";
import { pathToFileURL } from "node:url";
import { changelogLoader, changelogSchema } from "../src/lib/changelog.ts";
import { release } from "../../../scripts/changelog.mjs";
import { refuseRawMarkup } from "./refuse-raw-markup.mjs";

const require = createRequire(import.meta.url);
const fromAstro = createRequire(`${dirname(require.resolve("astro/package.json"))}/`);
const { createMarkdownProcessor } = await import(pathToFileURL(fromAstro.resolve("@astrojs/markdown-remark")).href);

const folder = mkdtempSync(join(tmpdir(), "kaeso-showcase-"));
const file = pathToFileURL(join(folder, "CHANGELOG.md"));
const GUIDE = pathToFileURL(join(folder, "guide.md"));

let processor;
before(async () => {
  processor = await createMarkdownProcessor({ syntaxHighlight: false, remarkPlugins: [refuseRawMarkup] });
});
after(() => rmSync(folder, { recursive: true, force: true }));

const LINKS = "[Unreleased]: https://github.com/logicplanes/kaeso/compare/v1.0.0...HEAD\n[1.0.0]: https://github.com/logicplanes/kaeso/releases/tag/v1.0.0";

const loaded = async (text) => {
  writeFileSync(file, text);
  const entries = new Map();
  const context = {
    store: { clear: () => entries.clear(), set: ({ id, data }) => entries.set(id, data) },
    parseData: async ({ data }) => changelogSchema.parse(data),
    renderMarkdown: async (content, options) => ({ html: (await processor.render(content, { fileURL: options?.fileURL })).code }),
    generateDigest: () => "digest",
    logger: { error: () => {} },
  };
  await changelogLoader(file).load(context);
  return entries.get("kaeso");
};

const changelog = (markdown, links = LINKS) => `# Changelog\n\n## [Unreleased]\n\n${markdown}\n\n${links}\n`;
const FIRST = 5;
const render = (markdown, links) => loaded(changelog(markdown, links));
const refused = (markdown, line, reason = /raw HTML is not allowed/, links) =>
  assert.rejects(render(markdown, links), (error) => error.message.includes(`\nCHANGELOG.md line ${line}: `) && reason.test(error.message));

test("the changelog refuses raw HTML in an entry and names its line", async () => {
  await refused('- A <img src=x onerror="alert(1)"> picture.', FIRST);
  await refused("- A <script>alert(1)</script> script.", FIRST);
  await refused('- Fine.\n\n<div onclick="x()">block</div>', FIRST + 2);
});

test("the changelog refuses comments, processing instructions, declarations and HTML blocks", async () => {
  await refused("- A <!-- hidden --> note.", FIRST);
  await refused("- Fine.\n\n<!--\nhidden\n-->", FIRST + 2);
  await refused("- A <?php echo 1; ?> instruction.", FIRST);
  await refused("<!DOCTYPE html>", FIRST);
  await refused("<table>\n<tr><td>cell</td></tr>\n</table>", FIRST);
});

test("the changelog refuses a tag whose attributes continue on the next lines, at the line it starts on", async () => {
  await refused("- Fine.\n\n- A picture <img\n  src=x\n  onerror=alert(1)> here.", FIRST + 2);
  await refused("A picture <img\nsrc=x onerror=alert(1)>", FIRST);
});

test("the changelog refuses a tag after an escaped backtick, which opens no code span", async () => {
  await refused("- A \\`<img src=x onerror=alert(1)>\\` picture.", FIRST);
  await refused("- A \\`<script>alert(1)</script>\\`", FIRST);
});

test("the changelog refuses a tag after four spaces that only look like a fence", async () => {
  await refused("Text\n    ```\n<script>alert(1)</script>\n    ```", FIRST + 2);
  await refused("    ```\n\n<div>x</div>\n\n    ```", FIRST + 2);
});

test("the changelog refuses a tag after a code span that a backslash does not close", async () => {
  await refused("- The `a \\` <i>` span.", FIRST);
});

test("the changelog refuses raw HTML in a quote and in a table cell", async () => {
  await refused("> A <b>quote</b>", FIRST);
  await refused("| a | b |\n| - | - |\n| x | <b>y</b> |", FIRST + 2);
});

test("the changelog refuses links that are not http, https or mailto, however they are written", async () => {
  const unsafe = /links may only use http, https or mailto/;
  await refused("- A [link](javascript:alert(1)).", FIRST, unsafe);
  await refused("- A [one](https://example.com) and [two](javascript:alert(1)).", FIRST, unsafe);
  await refused("- A [link](\n  javascript:alert(1)).", FIRST, unsafe);
  await refused("- A [link](java&#9;script:alert(1)).", FIRST, unsafe);
  await refused("- A [link](JavaScript:alert(1)).", FIRST, unsafe);
  await refused("- A [link][r].\n\n[r]: data:text/html,x", FIRST + 2, unsafe);
  await refused("- An ![image](data:image/svg+xml,x).", FIRST, unsafe);
});

test("the changelog accepts HTML written as code, in fences, in indented code, as an autolink or as an entity", async () => {
  const markdown = [
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
  const { unreleased } = await render(markdown);
  assert.match(unreleased.intro, /&#x3C;script>alert\(1\)&#x3C;\/script>/);
  assert.doesNotMatch(unreleased.intro, /<script/i);
});

test("the changelog accepts http, https, mailto, relative and anchor links", async () => {
  const markdown = "- [a](https://example.com), [b](http://example.com/x?y:z), [c](mailto:owner@example.com), [d](#v9.9.9), [e](docs/page.md) and [f](/x:y).";
  const { unreleased } = await render(markdown);
  assert.match(unreleased.intro, /href="https:\/\/example\.com"/);
});

test("only the changelog is held to this: other pages keep their markup", async () => {
  const { code } = await processor.render("A <b>bold</b> word.", { fileURL: GUIDE });
  assert.match(code, /<b>bold<\/b>/);
});

test("each fragment is checked as it is rendered: a fence that swallows a heading in the whole file does not hide markup in the fragment after it", async () => {
  const markdown = "- Fine.\n\n```\ncode\n\n### Fixed\n\n<b>x</b>\n```";
  await refused(markdown, FIRST + 7);
});

test("each fragment is checked as it is rendered: a fence that closes in the next fragment leaves that fragment's markup as code", async () => {
  const { unreleased } = await render("```\ncode\n\n### Fixed\n\n```\n\n<b>x</b>");
  assert.match(unreleased.sections[0].html, /&#x3C;b>x&#x3C;\/b>/);
  assert.doesNotMatch(unreleased.sections[0].html, /<b>x<\/b>/);
});

test("the line named is the line of the file, for a section, a topic, an older release and the reference definitions", async () => {
  const text = [
    "# Changelog",
    "",
    "## [Unreleased]",
    "",
    "### Fixed",
    "",
    "#### Console",
    "",
    "- <i>Fine</i>.",
    "",
    "## [1.0.0] - 2026-09-01",
    "",
    "Intro.",
    "",
    "### Added",
    "",
    "- Fine.",
    "",
    "- <u>x</u>",
    "",
    LINKS,
    "",
  ].join("\n");
  const without = (...fixes) => fixes.reduce((current, [from, to]) => current.replace(from, to), text);
  const fixI = ["- <i>Fine</i>.", "- Fine."];
  const fixU = ["- <u>x</u>", "- Fine."];
  const lineFailure = (broken, line) => assert.rejects(loaded(broken), (error) => error.message.includes(`\nCHANGELOG.md line ${line}: raw HTML`));
  await lineFailure(text, 9);
  await lineFailure(without(fixI), 19);
  await lineFailure(without(fixI, fixU, ["Intro.", "Intro <b>bold</b>."]), 13);
  const unsafe = without(fixI, fixU, ["[1.0.0]:", "[bad]: javascript:alert(1)\n[1.0.0]:"]);
  const definitions = unsafe.split("\n").indexOf(LINKS.split("\n")[0]) + 1;
  await assert.rejects(loaded(unsafe), (error) => error.message.includes(`\nCHANGELOG.md line ${definitions + 1}: links may only`));
});

test("a changelog that passes renders what the whole file would, with the same HTML", async () => {
  const text = changelog("- A `<b>` tag and [a link](https://example.com).\n\n### Fixed\n\n- Done [here][r].\n\n[r]: https://example.com/r");
  const { unreleased } = await loaded(text);
  const direct = await processor.render("- A `<b>` tag and [a link](https://example.com).", { fileURL: GUIDE });
  assert.equal(unreleased.intro, direct.code);
  assert.match(unreleased.sections[0].html, /<a href="https:\/\/example\.com\/r">here<\/a>/);
});

test("a release written from pull requests renders markup in a title as text, with the link to its pull request", async () => {
  const repository = "https://github.com/logicplanes/kaeso";
  const before = `# Changelog\n\n## [Unreleased]\n\n## [1.0.0] - 2026-09-01\n\n### Added\n\n- Batches.\n\n${LINKS}\n`;
  const written = release(before, "1.1.0", "2026-10-01", [
    {
      number: 7,
      title: "Keep <Select> and [the menu](javascript:alert(1)) open around `<Dialog>`",
      url: `${repository}/pull/7`,
      labels: ["fixed", "area: console"],
    },
  ]);
  const [latest] = (await loaded(written)).releases;
  const [topic] = latest.sections[0].topics;
  assert.equal(topic.name, "Console");
  assert.match(topic.html, /Keep &#x3C;Select> and \[the menu\]\(javascript:alert\(1\)\) open around <code>&#x3C;Dialog><\/code>/);
  assert.match(topic.html, /<a href="https:\/\/github\.com\/logicplanes\/kaeso\/pull\/7">#7<\/a>/);
  assert.doesNotMatch(topic.html, /<select|href="javascript/i);
});
