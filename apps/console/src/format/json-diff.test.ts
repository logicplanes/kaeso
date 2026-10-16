import { describe, expect, it } from "vitest";
import { jsonDiff, stableJson } from "./json-diff";

describe("jsonDiff", () => {
  it("marks a changed field as removed and added, keeping the rest", () => {
    expect(jsonDiff({ name: "Default", content_logging: true }, { content_logging: true, name: "Main" })).toBe(
      ["  {", '    "content_logging": true,', '-   "name": "Default"', '+   "name": "Main"', "  }"].join("\n"),
    );
  });

  it("shows a created value as all added and a deleted one as all removed", () => {
    expect(jsonDiff(null, { a: 1 })).toBe(["+ {", '+   "a": 1', "+ }"].join("\n"));
    expect(jsonDiff({ a: 1 }, undefined)).toBe(["- {", '-   "a": 1', "- }"].join("\n"));
  });

  it("sorts keys so the order of fields doesn't count as a change", () => {
    expect(stableJson({ b: 1, a: { d: 2, c: 3 } })).toBe(stableJson({ a: { c: 3, d: 2 }, b: 1 }));
    expect(
      jsonDiff({ b: 1, a: 2 }, { a: 2, b: 1 })
        .split("\n")
        .every((l) => l.startsWith("  ")),
    ).toBe(true);
  });

  it("keeps the longest run of unchanged lines around added and removed ones", () => {
    expect(jsonDiff(["a", "b", "c", "d"], ["a", "c", "x", "d"])).toBe(["  [", '    "a",', '-   "b",', '    "c",', '+   "x",', '    "d"', "  ]"].join("\n"));
  });
});
