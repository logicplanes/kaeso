import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(join(__dirname, "../styles/kaeso.css"), "utf8");

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const path = join(dir, e.name);
    if (e.isDirectory()) return sources(path);
    return /\.tsx?$/.test(e.name) && !/\.test\.tsx?$/.test(e.name) ? [path] : [];
  });
}

describe("what scrolls on a phone", () => {
  it("starts hidden text at its containing block's edge, so text cut off in a cell can't widen the page", () => {
    const base = css.slice(css.indexOf("@layer base {"));
    const rule = base.slice(base.indexOf(".sr-only {"), base.indexOf("}", base.indexOf(".sr-only {")));
    expect(rule).toContain("inset-inline-start: 0;");
    expect(rule).not.toContain("top:");
  });

  it("keeps a swipe past the end of a sideways scroller from moving the page or going back a page", () => {
    const loose: string[] = [];
    for (const file of sources(join(__dirname)))
      for (const [classes] of readFileSync(file, "utf8").matchAll(/"[^"\n]*\boverflow-x-(?:auto|scroll)\b[^"\n]*"/g))
        if (!/\boverscroll-(?:x-)?contain\b/.test(classes)) loose.push(`${file.slice(__dirname.length + 1)}: ${classes}`);
    expect(loose).toEqual([]);
    const grid = css.slice(css.indexOf("@utility compare-grid {"), css.indexOf("\n}", css.indexOf("@utility compare-grid {")));
    expect(grid).toContain("overscroll-behavior-x: contain;");
  });
});
