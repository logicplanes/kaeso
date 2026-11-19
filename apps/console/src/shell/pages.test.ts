import { describe, expect, it } from "vitest";
import { ORG_AREA_PAGES, PROJECT_PAGES } from "./pages";
import { ACCOUNT_PAGES, INSTANCE_PAGES, ORG_SETTINGS_PAGES } from "./settings-pages";

describe("pages", () => {
  it("gives every page in the sidebar an icon from the installed library", () => {
    for (const page of [...ORG_AREA_PAGES, ...PROJECT_PAGES]) expect(page.icon, page.id).toBeDefined();
  });

  it("gives every page its own icon within its part of the navigation", () => {
    const navs = { org: ORG_AREA_PAGES, project: PROJECT_PAGES, account: ACCOUNT_PAGES, orgSettings: ORG_SETTINGS_PAGES, instance: INSTANCE_PAGES };
    for (const [nav, pages] of Object.entries(navs)) {
      const shared = pages.filter((p) => pages.some((q) => q !== p && q.icon === p.icon)).map((p) => p.id);
      expect(shared, nav).toEqual([]);
    }
  });

  it("gives every project page its own go-to letter, apart from the settings' `s`", () => {
    const keys = PROJECT_PAGES.map((p) => p.key);
    expect(new Set(keys).size).toBe(keys.length);
    expect(keys).not.toContain("s");
    expect(keys.every((k) => /^[a-z]$/.test(k))).toBe(true);
  });
});
