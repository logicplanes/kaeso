import { afterEach, describe, expect, it, vi } from "vitest";
import { cellWidth, textWidth } from "./text-width";

describe("textWidth", () => {
  it("gives Chinese, Korean and full-width characters a full em and Latin letters about half of one", () => {
    expect(textWidth("Profile changed")).toBeCloseTo(15 * 7.4);
    expect(textWidth("资料已更改")).toBe(70);
    expect(textWidth("프로필")).toBe(42);
    expect(textWidth("ＡＢ")).toBe(28);
  });
});

describe("textWidth in a browser", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("measures with the cells' font", async () => {
    const fonts: string[] = [];
    vi.stubGlobal(
      "OffscreenCanvas",
      class {
        getContext() {
          return {
            set font(value: string) {
              fonts.push(value);
            },
            measureText: (text: string) => ({ width: text.length * 5 }),
          };
        }
      },
    );
    vi.resetModules();
    const measured = await import("./text-width");
    expect(measured.textWidth("Key created")).toBe(55);
    expect(fonts).toEqual([`14px ${getComputedStyle(document.body).fontFamily}`]);
  });

  it("estimates without a canvas", async () => {
    vi.stubGlobal("OffscreenCanvas", undefined);
    vi.resetModules();
    const estimated = await import("./text-width");
    expect(estimated.textWidth("Key")).toBeCloseTo(3 * 7.4);
  });

  it("estimates when the canvas has no 2D context", async () => {
    vi.stubGlobal(
      "OffscreenCanvas",
      class {
        getContext() {
          return null;
        }
      },
    );
    vi.resetModules();
    const estimated = await import("./text-width");
    expect(estimated.textWidth("Key")).toBeCloseTo(3 * 7.4);
  });
});

describe("cellWidth", () => {
  it("fits the longest label with the cell's padding, on the 8px grid", () => {
    expect(cellWidth(["Profile changed", "Service account created"], { min: 152, max: 320 })).toBe(200);
  });

  it("stays within its bounds", () => {
    expect(cellWidth(["Kaeso set up"], { min: 152, max: 320 })).toBe(152);
    expect(cellWidth(["x".repeat(80)], { min: 152, max: 264 })).toBe(264);
    expect(cellWidth([], { min: 152, max: 264 })).toBe(152);
  });
});
