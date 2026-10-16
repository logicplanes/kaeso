import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Heatmap } from "./heatmap.js";

describe("Heatmap", () => {
  it("names every cell and shades it by its share of the largest value", () => {
    render(
      <Heatmap
        aria-label="Requests by hour"
        rows={["Mon", "Tue"]}
        columns={["00", "01", "02", "03"]}
        values={[[0, 10, 40, 20], [5]]}
        columnLabelEvery={2}
        formatValue={(v) => `${v} req`}
      />,
    );
    const table = screen.getByRole("table", { name: "Requests by hour" });
    expect(screen.getAllByRole("row")).toHaveLength(2);
    expect(screen.getByRole("cell", { name: "Mon 02: 40 req" }).style.background).toBe("var(--k-viz-1)");
    expect(screen.getByRole("cell", { name: "Mon 00: 0 req" }).style.background).toBe("var(--k-fill-1)");
    expect(screen.getByRole("cell", { name: "Tue 03: 0 req" })).toBeInTheDocument();
    expect(table).toHaveTextContent("00");
    expect(table).not.toHaveTextContent("01");
  });

  it("stays at the lightest step when every value is zero", () => {
    render(<Heatmap aria-label="Requests" rows={["Mon"]} columns={["00"]} values={[[0]]} />);
    expect(screen.getByRole("cell", { name: "Mon 00: 0" }).style.background).toBe("var(--k-fill-1)");
  });
});
