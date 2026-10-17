import { describe, expect, it } from "vitest";
import { variantProblems, variantExport } from "./variant-exports.mjs";

describe("variantExport", () => {
  it("reads the component of a variants export", () => {
    expect(variantExport("KAESO_DELETE_RESOURCE_VARIANTS")).toEqual({ component: "DELETE_RESOURCE", part: "variants" });
  });

  it("reads the component of a default variants export", () => {
    expect(variantExport("KAESO_DELETE_RESOURCE_DEFAULT_VARIANTS")).toEqual({ component: "DELETE_RESOURCE", part: "defaults" });
  });

  it("ignores other exports", () => {
    expect(variantExport("DeleteResource")).toBeNull();
    expect(variantExport("KAESO_VARIANTS")).toBeNull();
  });
});

describe("variantProblems", () => {
  it("accepts components that export both, with defaults their axes list", () => {
    expect(variantProblems({ PAGES: { entry: "blocks/pages", variants: {}, defaults: {} } })).toEqual([]);
    expect(variantProblems({ AVATAR: { entry: "components/avatar", variants: { size: { 24: "24px", 32: "32px" } }, defaults: { size: "24" } } })).toEqual([]);
  });

  it("names a default its axis does not list", () => {
    expect(
      variantProblems({
        BUTTON: { entry: "components/button", variants: { size: { sm: "32px" } }, defaults: { size: "md", tone: "quiet" } },
      }),
    ).toEqual([
      "components/button: KAESO_BUTTON_DEFAULT_VARIANTS sets size to md, which KAESO_BUTTON_VARIANTS does not list",
      "components/button: KAESO_BUTTON_DEFAULT_VARIANTS sets tone to quiet, which KAESO_BUTTON_VARIANTS does not list",
    ]);
  });

  it("names a component that exports only one of them", () => {
    expect(
      variantProblems({
        PAGES: { entry: "blocks/pages", defaults: {} },
        BUTTON: { entry: "components/button", variants: {} },
      }),
    ).toEqual([
      "blocks/pages exports KAESO_PAGES_DEFAULT_VARIANTS without KAESO_PAGES_VARIANTS",
      "components/button exports KAESO_BUTTON_VARIANTS without KAESO_BUTTON_DEFAULT_VARIANTS",
    ]);
  });
});
