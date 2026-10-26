import { expect, it } from "vitest";
import { stableJson } from "./stable-json";

it("sorts keys so the order of fields doesn't count as a change", () => {
  expect(stableJson({ b: 1, a: { d: 2, c: 3 } })).toBe(stableJson({ a: { c: 3, d: 2 }, b: 1 }));
  expect(stableJson({ b: [{ y: 1, x: 2 }], a: null })).toBe(
    ["{", '  "a": null,', '  "b": [', "    {", '      "x": 2,', '      "y": 1', "    }", "  ]", "}"].join("\n"),
  );
});
