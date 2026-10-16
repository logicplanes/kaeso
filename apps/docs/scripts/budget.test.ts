import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomBytes } from "node:crypto";
import { afterEach, describe, expect, it } from "vitest";
import { FRAME_CEILING_KB, PAGE_CEILING_KB, measure } from "./budget.ts";

let dist = "";
afterEach(() => dist && rmSync(dist, { recursive: true, force: true }));

function build(files: Record<string, number>, manifest: Record<string, object>) {
  dist = mkdtempSync(join(tmpdir(), "kaeso-docs-dist-"));
  mkdirSync(join(dist, ".vite"), { recursive: true });
  mkdirSync(join(dist, "assets"), { recursive: true });
  for (const [file, bytes] of Object.entries(files)) writeFileSync(join(dist, file), randomBytes(bytes).toString("base64").slice(0, bytes));
  writeFileSync(join(dist, ".vite/manifest.json"), JSON.stringify(manifest));
  return dist;
}

const manifest = (pageFile = "assets/page.js") => ({
  "index.html": { file: "assets/index.js", isEntry: true, imports: ["_framework"], css: ["assets/index.css"] },
  _framework: { file: "assets/framework.js" },
  "kaeso-docs:site/en": { file: "assets/site-en.js", imports: ["_framework"] },
  "kaeso-docs:site/de": { file: "assets/site-de.js" },
  "../../packages/ui/dist/chunks/de-abc.js": { file: "assets/ui-de.js" },
  "kaeso-docs:site/zh-CN": { file: "assets/site-zh.js" },
  "kaeso-docs:page/en/guides": { file: pageFile },
  "src/shell/search.tsx": { file: "assets/search.js", imports: ["_framework", "_palette"] },
  _palette: { file: "assets/palette.js" },
});
const files = {
  "assets/index.js": 4000,
  "assets/index.css": 1000,
  "assets/framework.js": 4000,
  "assets/site-en.js": 500,
  "assets/site-de.js": 500,
  "assets/ui-de.js": 500,
  "assets/site-zh.js": 500,
  "assets/page.js": 800,
  "assets/search.js": 600,
  "assets/palette.js": 600,
};

describe("the bundle budget", () => {
  it("measures the frame per language, each page, and the search after the first paint", () => {
    const report = measure(build(files, manifest()));
    expect(report.failed).toBe(false);
    expect(report.frames.map((f) => f.name)).toEqual(["the frame in en", "the frame in de", "the frame in zh-CN"]);
    const [en, de] = report.frames;
    expect(de?.kb).toBeGreaterThan(en?.kb ?? 0);
    expect(report.pages).toEqual([expect.objectContaining({ name: "en/guides", ceiling: PAGE_CEILING_KB })]);
    expect(report.searchKb).toBeGreaterThan(0);
    expect(report.lines[0]).toMatch(new RegExp(`^budget: the frame in en: [\\d.]+ kB gzipped; ceiling ${FRAME_CEILING_KB} kB ok$`));
    expect(report.lines.at(-1)).toMatch(/^budget: the search, loaded after the first paint: [\d.]+ kB gzipped without its index$/);
  });

  it("fails on a page over its ceiling and names it", () => {
    const report = measure(build({ ...files, "assets/big.js": 60_000 }, manifest("assets/big.js")));
    expect(report.failed).toBe(true);
    expect(report.lines).toContainEqual(expect.stringMatching(/^budget: the page en\/guides is [\d.]+ kB gzipped, over 24 kB: split it into pages$/));
  });

  it("refuses a build without an entry", () => {
    expect(() => measure(build({}, {}))).toThrow("budget: the build has no entry");
  });
});
