import { describe, expect, it } from "vitest";
import brand from "./brand/tokens.json" with { type: "json" };
import * as tokens from "./index.js";

describe("the package's version", () => {
  it("is only the one in package.json, which every Kaeso package shares", () => {
    expect(Object.keys(brand)).not.toContain("version");
    expect(Object.keys(tokens).filter((name) => /version/i.test(name))).toEqual([]);
  });
});
