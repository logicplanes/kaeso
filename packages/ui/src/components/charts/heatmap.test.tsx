import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Heatmap } from "./heatmap.js";

const live = () => document.querySelector("[aria-live=polite]")?.textContent ?? "";

describe("Heatmap", () => {
  const props = {
    "aria-label": "Requests by hour",
    rows: ["Mon", "Tue"],
    columns: ["00", "01", "02", "03"],
    values: [[0, 10, 40, 20], [5]],
    columnLabelEvery: 2,
    formatValue: (v: number) => `${v} req`,
  };

  it("shades each cell by its share of the largest value and keeps every value in a table", () => {
    const { container } = render(<Heatmap {...props} />);
    const cell = (r: number, c: number) => container.querySelector<HTMLElement>(`[data-cell="${r}-${c}"]`)!;
    expect(cell(0, 2).style.background).toBe("var(--k-viz-1)");
    expect(cell(0, 0).style.background).toBe("var(--k-fill-1)");
    expect(cell(0, 2)).toHaveClass("data-mark");
    const table = screen.getByRole("table", { hidden: true });
    expect(within(table).getAllByRole("row", { hidden: true })).toHaveLength(3);
    expect(table).toHaveTextContent("Mon0 req10 req40 req20 req");
    const grid = screen.getByRole("group", { name: "Requests by hour" });
    expect(grid).toHaveTextContent("00");
    expect(grid).not.toHaveTextContent("01");
  });

  it("is one tab stop whose arrow keys walk the cells and say each value", async () => {
    render(<Heatmap {...props} />);
    await userEvent.tab();
    expect(screen.getByRole("group", { name: "Requests by hour" })).toHaveFocus();
    expect(live()).toBe("Mon 00: 0 req");
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(live()).toBe("Mon 02: 40 req");
    await userEvent.keyboard("{ArrowDown}{End}");
    expect(live()).toBe("Tue 03: 0 req");
    await userEvent.keyboard("{Escape}");
    expect(live()).toBe("");
  });

  it("shows a cell's value on hover", () => {
    const { container } = render(<Heatmap {...props} />);
    fireEvent.pointerEnter(container.querySelector('[data-cell="0-1"]')!);
    expect(container.querySelector("[role=presentation]")).toHaveTextContent("Mon 0110 req");
  });

  it("stays at the lightest step when every value is zero, and says when there is nothing", () => {
    const { container, rerender } = render(<Heatmap aria-label="Requests" rows={["Mon"]} columns={["00"]} values={[[0]]} />);
    expect(container.querySelector<HTMLElement>('[data-cell="0-0"]')!.style.background).toBe("var(--k-fill-1)");
    rerender(<Heatmap aria-label="Requests" rows={[]} columns={[]} values={[]} />);
    expect(screen.getByText("No data for this period")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "View as table" })).toBeNull();
  });
});
