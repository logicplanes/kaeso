import { describe, expect, it } from "vitest";
import { figures, tabularText } from "./figures.js";

describe("figures", () => {
  it("sets names and versions with a hyphen between letters and figures in proportional figures", () => {
    for (const text of ["1.3.0 to 1.4.0-rc.2", "40.4.3-rc.2 available", "gpt-5-mini", "checkout-web · Developer", "Pre-release", "o3-mini"]) {
      expect(tabularText(text), text).toBe(false);
      expect(figures(text), text).toBeUndefined();
    }
  });

  it("keeps tabular figures for numbers, dates and times, so they line up", () => {
    for (const text of ["12", "1,204 tokens", "2026-10-13", "13 Oct", "Tue, Oct 13, 02:00 PM", "10月13日 14:00", "−5", "1.3.0 to 1.4.0"]) {
      expect(figures(text), text).toBe("tabular");
    }
    expect(figures(["1.3.0", " to ", 7])).toBe("tabular");
    expect(figures(["1.3.0 to ", "1.4.0-rc.2"])).toBeUndefined();
  });

  it("leaves an element tabular, as it was: its text can't be read before it renders", () => {
    expect(figures(<span>gpt-5-mini</span>)).toBe("tabular");
  });
});
