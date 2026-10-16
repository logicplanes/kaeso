import { describe, expect, it } from "vitest";
import { cn } from "./cn";

describe("cn: the components' own merges", () => {
  it.each([
    ["h-36 px-16 gap-6", "h-32 px-12", "gap-6 h-32 px-12"],
    ["cursor-pointer", "cursor-not-allowed", "cursor-not-allowed"],
    ["hover:bg-fill-1", "hover:bg-fill-2", "hover:bg-fill-2"],
    ["bg-fill-3 hover:bg-line-strong", "bg-ink hover:bg-ink", "bg-ink hover:bg-ink"],
    ["text-inherit", "text-fg", "text-fg"],
    ["size-14", "size-12", "size-12"],
    ["min-w-220", "min-w-(--anchor-width)", "min-w-(--anchor-width)"],
    ["material-secondary", "material-ghost", "material-ghost"],
    ["focus-ring", "focus-ring-inset", "focus-ring-inset"],
    ["popup-motion", "dialog-motion", "dialog-motion"],
    ["rounded-pill", "rounded-item", "rounded-item"],
    ["shadow-control", "shadow-none", "shadow-none"],
  ])("%s then %s", (base, override, merged) => {
    expect(cn(base, override)).toBe(merged);
  });
  it("keeps a type size and a text color: they are different groups", () => {
    expect(cn("text-body text-fg", "text-small text-fg-secondary")).toBe("text-small text-fg-secondary");
    expect(cn("text-body", "text-fg-secondary")).toBe("text-body text-fg-secondary");
    expect(cn("text-metric", "text-fg")).toBe("text-metric text-fg");
  });
  it("knows the token lengths, radii and shadows", () => {
    expect(cn("p-12", "p-6")).toBe("p-6");
    expect(cn("w-240", "w-320")).toBe("w-320");
    expect(cn("rounded-popover", "rounded-dialog")).toBe("rounded-dialog");
    expect(cn("shadow-control", "shadow-xs")).toBe("shadow-xs");
    expect(cn("font-medium", "font-semibold")).toBe("font-semibold");
  });
});

describe("cn: shadow tokens named like colors", () => {
  it.each([
    ["shadow-raised", "shadow-none", "shadow-none"],
    ["shadow-panel", "shadow-none", "shadow-none"],
    ["shadow-toast", "shadow-none", "shadow-none"],
    ["shadow-none", "shadow-raised", "shadow-raised"],
    ["shadow-panel", "shadow-toast", "shadow-toast"],
    ["shadow-raised", "shadow-dialog", "shadow-dialog"],
    ["shadow-control", "shadow-raised", "shadow-raised"],
    ["shadow-raised", "shadow-panel", "shadow-panel"],
    ["shadow-ring", "shadow-toast", "shadow-toast"],
    ["hover:shadow-control", "hover:shadow-raised", "hover:shadow-raised"],
  ])("%s then %s", (base, override, merged) => {
    expect(cn(base, override)).toBe(merged);
  });
  it("treats every shadow token as a box shadow", async () => {
    const { shadow } = await import("@logicplanes/kaeso-tokens");
    for (const a of Object.keys(shadow)) {
      expect(cn(`shadow-${a}`, "shadow-none")).toBe("shadow-none");
      for (const b of Object.keys(shadow)) expect(cn(`shadow-${a}`, `shadow-${b}`)).toBe(`shadow-${b}`);
    }
  });
  it("leaves the color tokens of the same names to the color utilities", () => {
    expect(cn("bg-raised", "bg-panel")).toBe("bg-panel");
    expect(cn("bg-toast shadow-toast", "bg-raised")).toBe("shadow-toast bg-raised");
    expect(cn("ring-panel-ring shadow-panel", "shadow-none")).toBe("ring-panel-ring shadow-none");
  });
});

describe("cn: a consumer's layout className", () => {
  it.each([
    ["mt-48 first:mt-0", "mt-0", "first:mt-0 mt-0"],
    ["flex min-w-0 flex-col", "flex-row", "flex min-w-0 flex-row"],
    ["inline-flex", "flex", "flex"],
    ["relative", "absolute", "absolute"],
    ["w-auto", "w-full", "w-full"],
    ["gap-8", "gap-16", "gap-16"],
    ["grid grid-cols-2 lg:grid-cols-4", "grid-cols-3", "grid lg:grid-cols-4 grid-cols-3"],
    ["self-center", "self-start", "self-start"],
    ["shrink-0", "shrink", "shrink"],
    ["mx-auto max-w-720", "max-w-480", "mx-auto max-w-480"],
  ])("%s with %s", (base, className, merged) => {
    expect(cn(base, className)).toBe(merged);
  });
  it("drops falsy parts", () => {
    expect(cn("flex", false, undefined, null, "", "gap-8")).toBe("flex gap-8");
  });
});
