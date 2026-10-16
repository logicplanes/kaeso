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
