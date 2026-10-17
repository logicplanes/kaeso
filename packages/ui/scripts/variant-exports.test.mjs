import { describe, expect, it } from "vitest";
import { unpairedVariantExports, variantExport } from "./variant-exports.mjs";

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

describe("unpairedVariantExports", () => {
  it("accepts components that export both", () => {
    expect(unpairedVariantExports({ PAGES: { entry: "blocks/pages", variants: {}, defaults: {} } })).toEqual([]);
  });

  it("names a component that exports only one of them", () => {
    expect(
      unpairedVariantExports({
        PAGES: { entry: "blocks/pages", defaults: {} },
        BUTTON: { entry: "components/button", variants: {} },
      }),
    ).toEqual([
      "blocks/pages exports KAESO_PAGES_DEFAULT_VARIANTS without KAESO_PAGES_VARIANTS",
      "components/button exports KAESO_BUTTON_VARIANTS without KAESO_BUTTON_DEFAULT_VARIANTS",
    ]);
  });
});
