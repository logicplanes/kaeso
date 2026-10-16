import { describe, expect, it } from "vitest";
import { ORG_AREA_PAGES, PROJECT_PAGES } from "./pages";

describe("pages", () => {
  it("gives every page in the sidebar an icon from the installed library", () => {
    for (const page of [...ORG_AREA_PAGES, ...PROJECT_PAGES]) expect(page.icon, page.id).toBeDefined();
  });
});
