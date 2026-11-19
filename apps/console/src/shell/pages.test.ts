import { describe, expect, it } from "vitest";
import { ORG_AREA_PAGES, PROJECT_PAGES } from "./pages";

describe("pages", () => {
  it("gives every page in the sidebar an icon from the installed library", () => {
    for (const page of [...ORG_AREA_PAGES, ...PROJECT_PAGES]) expect(page.icon, page.id).toBeDefined();
  });

  it("gives every project page its own go-to letter, apart from the settings' `s`", () => {
    const keys = PROJECT_PAGES.map((p) => p.key);
    expect(new Set(keys).size).toBe(keys.length);
    expect(keys).not.toContain("s");
    expect(keys.every((k) => /^[a-z]$/.test(k))).toBe(true);
  });
});
