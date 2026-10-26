import { describe, expect, it } from "vitest";
import { diffLines, diffStats, foldUnchanged, unifiedDiff } from "./diff.js";

const pick = (text: string, ranges: [number, number][] | undefined) => (ranges ?? []).map(([s, e]) => text.slice(s, e));

describe("diffLines", () => {
  it("keeps unchanged lines and numbers each side", () => {
    expect(diffLines("a\nb\nc", "a\nx\nc")).toEqual([
      { kind: "same", text: "a", oldNumber: 1, newNumber: 1 },
      { kind: "removed", text: "b", oldNumber: 2 },
      { kind: "added", text: "x", newNumber: 2 },
      { kind: "same", text: "c", oldNumber: 3, newNumber: 3 },
    ]);
  });

  it("shows a created text as all added and a deleted one as all removed", () => {
    expect(diffLines("", "a\nb").map((l) => l.kind)).toEqual(["added", "added"]);
    expect(diffLines("a\nb\n", "").map((l) => l.kind)).toEqual(["removed", "removed"]);
  });

  it("keeps the longest run of unchanged lines around a change", () => {
    expect(unifiedDiff(diffLines("a\nb\nc\nd", "a\nc\nx\nd"))).toBe(["  a", "- b", "  c", "+ x", "  d"].join("\n"));
  });

  it("marks only the words that changed inside a changed line", () => {
    const [removed, added] = diffLines('  "name": "Sub Experiment"', '  "name": "Agents"');
    expect(pick(removed?.text ?? "", removed?.changes)).toEqual(["Sub Experiment"]);
    expect(pick(added?.text ?? "", added?.changes)).toEqual(["Agents"]);
  });

  it("marks a changed id as one word", () => {
    const [removed, added] = diffLines('"id": "proj_01m5m8f0beeb8vqk",', '"id": "proj_01m5p292xvef2v2s",');
    expect(pick(removed?.text ?? "", removed?.changes)).toEqual(["proj_01m5m8f0beeb8vqk"]);
    expect(pick(added?.text ?? "", added?.changes)).toEqual(["proj_01m5p292xvef2v2s"]);
  });

  it("marks no words when two lines share nothing but punctuation", () => {
    const [removed, added] = diffLines('"model": 1,', '"stream": [');
    expect(removed?.changes).toBeUndefined();
    expect(added?.changes).toBeUndefined();
  });

  it("still diffs texts too long for a full table, as removed then added", () => {
    const before = Array.from({ length: 2100 }, (_, i) => `a${i}`).join("\n");
    const after = Array.from({ length: 2100 }, (_, i) => `b${i}`).join("\n");
    expect(diffStats(diffLines(before, after))).toEqual({ added: 2100, removed: 2100 });
  });
});

describe("foldUnchanged", () => {
  const lines = diffLines(
    Array.from({ length: 20 }, (_, i) => `line ${i}`).join("\n"),
    Array.from({ length: 20 }, (_, i) => (i === 10 ? "changed" : `line ${i}`)).join("\n"),
  );

  it("folds unchanged runs beyond the context around a change", () => {
    const rows = foldUnchanged(lines, 3, new Set());
    expect(rows.map((r) => (r.type === "fold" ? `fold ${r.to - r.from}` : r.line.kind))).toEqual([
      "fold 7",
      "same",
      "same",
      "same",
      "removed",
      "added",
      "same",
      "same",
      "same",
      "fold 6",
    ]);
  });

  it("shows a folded run once it is opened", () => {
    expect(foldUnchanged(lines, 3, new Set([0])).filter((r) => r.type === "fold")).toHaveLength(1);
  });

  it("never folds a single line", () => {
    const short = diffLines("a\nb\nc\nd\ne", "a\nb\nc\nd\nx");
    expect(foldUnchanged(short, 3, new Set()).every((r) => r.type === "line")).toBe(true);
  });
});
