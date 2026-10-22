import { describe, expect, it } from "vitest";
import { internalPath, safeNext } from "./next";

describe("where sign-in goes", () => {
  it("follows the console's own paths", () => {
    expect(safeNext("/orgs/kaeso/projects/console?period=7d")).toBe("/orgs/kaeso/projects/console?period=7d");
    expect(internalPath("/settings/account/profile")).toBe("/settings/account/profile");
  });
  it("never another origin, and home without a path", () => {
    expect(safeNext("//elsewhere.test/x")).toBe("/");
    expect(safeNext("/\\elsewhere.test/x")).toBe("/");
    expect(safeNext("/\t/elsewhere.test/x")).toBe("/");
    expect(safeNext("/\n/elsewhere.test/x")).toBe("/");
    expect(safeNext("javascript:alert(1)")).toBe("/");
    expect(safeNext("https://elsewhere.test")).toBe("/");
    expect(safeNext(undefined)).toBe("/");
    expect(internalPath("")).toBeNull();
    expect(internalPath(null)).toBeNull();
  });
  it("never a path whose dot segments collapse into another origin", () => {
    for (const path of [
      "/.//evil.example",
      "/%2e//evil.example",
      "/%2E//evil.example",
      "/a/..//evil.example",
      "/./\\evil.example",
      "/orgs/../..//evil.example/x",
    ]) {
      expect(internalPath(path), path).toBeNull();
      expect(safeNext(path), path).toBe("/");
    }
  });
  it("keeps the path as the browser resolves it, not as it was written", () => {
    expect(internalPath("/settings/../orgs/kaeso?tab=keys#new")).toBe("/orgs/kaeso?tab=keys#new");
    expect(internalPath("/./orgs/%2e/kaeso")).toBe("/orgs/kaeso");
  });
});
