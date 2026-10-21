import { afterEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { Donut } from "./donut.js";

const three = [
  { id: "b", label: "Fraud screening", value: 4 },
  { id: "c", label: "Docs search", value: 5 },
  { id: "d", label: "Dev Series A", value: 6 },
];

describe("Donut", () => {
  it("draws data with fewer or more slices at once, never a slice the data doesn't have", () => {
    const { rerender } = render(<Donut aria-label="Cost by project" slices={three} />);
    rerender(<Donut aria-label="Cost by project" slices={[{ id: "a", label: "Checkout assistant", value: 3 }]} />);
    expect(document.querySelectorAll("svg path")).toHaveLength(1);
    rerender(<Donut aria-label="Cost by project" slices={three} />);
    expect(document.querySelectorAll("svg path")).toHaveLength(3);
  });
  afterEach(() => vi.restoreAllMocks());
  it("puts the legend under the ring when the chart is too narrow for it beside (a phone), and beside it otherwise", () => {
    const at = (width: number) => {
      vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(width);
      const { getByRole, unmount } = render(<Donut aria-label="Cost by project" slices={three} />);
      const legend = getByRole("list", { name: "Legend" });
      const row = legend.parentElement?.className ?? "";
      unmount();
      vi.restoreAllMocks();
      return row;
    };
    expect(at(340)).toContain("flex-col");
    expect(at(720)).not.toContain("flex-col");
  });
});

describe("Donut's legend and table", () => {
  const cost = [
    { id: "mini", label: "GPT-5 mini", value: 0.004 },
    { id: "sol", label: "GPT-6 Sol", value: 10.29 },
    { id: "opus", label: "Claude Opus 5.5", value: 9.89 },
    { id: "nano", label: "GPT-5.4 nano", value: 0.004 },
    { id: "five", label: "GPT-5.6", value: 20.54 },
  ];
  const legendRows = () => [...document.querySelectorAll("ul[aria-label=Legend] li")].map((li) => li.textContent);

  it("lists slices by value, the rest last, with the same rounded shares as its table", () => {
    const { getByRole } = render(<Donut aria-label="Cost by model" slices={cost} formatValue={(v) => v.toFixed(2)} />);
    expect(legendRows()).toEqual(["GPT-5.620.5451%", "GPT-6 Sol10.2925%", "Claude Opus 5.59.8924%", "Other0.01<1%"]);
    const table = getByRole("table", { hidden: true });
    expect([...table.querySelectorAll("tbody tr")].map((r) => r.textContent)).toEqual([
      "GPT-5.620.5451%",
      "GPT-6 Sol10.2925%",
      "Claude Opus 5.59.8924%",
      "Other0.01<1%",
    ]);
  });
});

describe("seriesColorsNear", () => {
  it("gives each hue the nearest free palette color, and the rest what is left", async () => {
    const { seriesColorsNear, VIZ } = await import("./core.js");
    const colors = seriesColorsNear(
      ["violet", "purple", "green", "plain"],
      new Map([
        ["violet", 262],
        ["purple", 265],
        ["green", 150],
      ]),
    );
    expect(colors.get("violet")).toBe(VIZ[4]);
    expect(colors.get("purple")).not.toBe(VIZ[4]);
    expect(colors.get("green")).toBe(VIZ[2]);
    expect(new Set(colors.values()).size).toBe(4);
  });
});
