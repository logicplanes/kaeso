import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { COVERS, cover, file } from "./mail-covers.mjs";

test("each email cover is its release art drawn with the wordmark, as committed", () => {
  for (const c of COVERS) {
    assert.equal(readFileSync(file(c.name), "utf8"), cover(c.art), `${c.name}: run node scripts/mail-covers.mjs`);
  }
});

test("each cover is wide, carries the wordmark and draws no text", () => {
  for (const c of COVERS) {
    const svg = cover(c.art);
    assert.match(svg, /^<svg xmlns="http:\/\/www.w3.org\/2000\/svg" viewBox="0 0 840 360">/, c.name);
    assert.match(svg, /<g fill="#[0-9a-f]{6}" transform="translate\(40 /, c.name);
    assert.doesNotMatch(svg, /<text/, c.name);
  }
});

test("no two covers share a style", () => {
  const styles = COVERS.map((c) => c.art.style);
  assert.equal(new Set(styles).size, styles.length);
});
