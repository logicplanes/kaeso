import { describe, expect, it } from "vitest";
import { constantName, pagesWithoutVariants, variantProblems, variantExport } from "./variant-exports.mjs";

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

describe("constantName", () => {
  it("writes a component's name as its variants constant does", () => {
    expect(constantName("ContextMenu")).toBe("CONTEXT_MENU");
    expect(constantName("Menubar")).toBe("MENUBAR");
    expect(constantName("OTPInput")).toBe("OTP_INPUT");
    expect(constantName("QRCode")).toBe("QR_CODE");
  });
});

describe("pagesWithoutVariants", () => {
  const components = [
    { entry: "menu", variants: { MENU: { axes: {}, defaults: {} }, CONTEXT_MENU: { axes: {}, defaults: {} } } },
    { entry: "kaeso", variants: { CONVERSATION: { axes: { density: {} }, defaults: { density: "default" } } } },
  ];

  it("accepts a page whose component exports its own pair, or names the shared pair it uses", () => {
    expect(
      pagesWithoutVariants(
        [
          { file: "src/demos/context-menu.tsx", entry: "menu", component: "ContextMenu" },
          { file: "src/demos/conversation-view.tsx", entry: "kaeso", component: "ConversationView", variants: "CONVERSATION" },
        ],
        components,
      ),
    ).toEqual([]);
  });

  it("names a page whose component has no pair, even when another component of its entry has one", () => {
    expect(
      pagesWithoutVariants(
        [
          { file: "src/demos/menubar.tsx", entry: "menu", component: "Menubar" },
          { file: "src/demos/key-display.tsx", entry: "kaeso", component: "KeyDisplay", variants: "IDENTITY" },
          { file: "src/demos/gone.tsx", entry: "gone", component: "Gone" },
        ],
        components,
      ),
    ).toEqual([
      "src/demos/menubar.tsx shows Menubar, but menu exports no KAESO_MENUBAR_VARIANTS and KAESO_MENUBAR_DEFAULT_VARIANTS",
      "src/demos/key-display.tsx names KAESO_IDENTITY_VARIANTS, but kaeso exports no KAESO_IDENTITY_VARIANTS and KAESO_IDENTITY_DEFAULT_VARIANTS",
      "src/demos/gone.tsx shows Gone, but gone exports no KAESO_GONE_VARIANTS and KAESO_GONE_DEFAULT_VARIANTS",
    ]);
  });
});
