import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatGrid, StatTile, statGridFit, statGridLayout } from "./stat.js";

describe("StatTile", () => {
  it("keeps room for a trend only when it has one, loading or drawn", () => {
    render(
      <StatGrid aria-label="Usage">
        <StatTile label="Requests" value="334" detail="11 a day on average" />
        <StatTile label="Tokens" value="27.2K" trend={[]} />
        <StatTile label="Cost" value="$0.09" trend={[1, 3, 2]} />
      </StatGrid>,
    );
    const [plain, loading, drawn] = screen.getAllByRole("listitem");
    expect(plain?.children).toHaveLength(3);
    expect(loading?.children).toHaveLength(4);
    expect(drawn?.children).toHaveLength(4);
    expect(screen.getByLabelText("Cost per day")).toBeInTheDocument();
  });
});

describe("StatTile loading", () => {
  it("shows the tile's shape at its final height instead of a number, and says it is busy", () => {
    render(
      <StatGrid aria-label="Usage">
        <StatTile label="Requests" value="334" detail="vs. the previous 30 days" change={{ value: 0.29 }} trend={[]} loading />
      </StatGrid>,
    );
    const tile = screen.getByRole("listitem");
    expect(tile).toHaveAttribute("aria-busy", "true");
    expect(tile).toHaveTextContent(/^Requests$/);
    expect(tile.children).toHaveLength(4);
    expect(tile.querySelectorAll("[aria-hidden]")).toHaveLength(3);
    expect(tile.querySelectorAll("[data-stat-value]")).toHaveLength(1);
  });
  it("keeps a plain tile's height and padding while loading: no trend row it won't have", () => {
    const { rerender } = render(
      <StatGrid aria-label="Usage">
        <StatTile label="Projects" value="5" loading />
      </StatGrid>,
    );
    const tile = screen.getByRole("listitem");
    expect(tile.children).toHaveLength(3);
    expect(tile).toHaveClass("pb-16");
    rerender(
      <StatGrid aria-label="Usage">
        <StatTile label="Projects" value="5" />
      </StatGrid>,
    );
    expect(screen.getByRole("listitem").children).toHaveLength(3);
    expect(screen.getByRole("listitem")).toHaveClass("pb-16");
  });
});

describe("StatGrid", () => {
  it("gives three tiles three columns and never an empty fourth cell", () => {
    render(
      <StatGrid aria-label="Usage">
        <StatTile label="Requests" value="12" />
        <StatTile label="Tokens" value="1.2K" />
        <StatTile label="Errors" value="0" />
      </StatGrid>,
    );
    const grid = screen.getByRole("list", { name: "Usage" });
    expect(grid).toHaveClass("lg:grid-cols-3");
    expect(grid).not.toHaveClass("lg:grid-cols-4");
  });
  it("gives four tiles four columns", () => {
    render(
      <StatGrid aria-label="Usage">
        {["Requests", "Tokens", "Cost", "Errors"].map((l) => (
          <StatTile key={l} label={l} value="1" />
        ))}
      </StatGrid>,
    );
    expect(screen.getByRole("list", { name: "Usage" })).toHaveClass("lg:grid-cols-4");
  });
});

const tiles = (n: number) => Array.from({ length: n }, (_, i) => <StatTile key={i} label={`Metric ${i + 1}`} value={String(i)} />);

describe("StatGrid rows", () => {
  it("widens the last of five so no cell stays empty, on two columns and on four", () => {
    render(<StatGrid aria-label="Five">{tiles(5)}</StatGrid>);
    const items = screen.getAllByRole("listitem");
    expect(items.at(-1)).toHaveClass("col-span-2", "lg:col-span-4");
    expect(items[0]).not.toHaveClass("col-span-2");
  });
  it("names the change as better or worse by its direction", () => {
    render(<StatTile label="Errors" value="0.4%" change={{ value: -0.31, better: "down" }} />);
    expect(screen.getByText("↓31%")).toHaveClass("text-success-text");
  });
});

describe("StatGrid on a narrow screen", () => {
  it("keeps the largest size while every number fits its tile", () => {
    expect(statGridFit([106, 120, 136, 100], 1100, true)).toEqual({ step: 0, single: false, two: false });
  });
  it("steps every number down together on a phone, where $1,041.13 would be cut short", () => {
    expect(statGridFit([106, 120, 136, 100], 342, false)).toEqual({ step: 1, single: false, two: false });
  });
  it("gives each tile a row when even the smallest size doesn't fit", () => {
    expect(statGridFit([106, 120, 165, 100], 272, false)).toEqual({ step: 0, single: true, two: false });
  });
  it("counts the room of a tile that widens over a short row", () => {
    expect(statGridLayout(3, false)).toEqual({ cols: 2, spans: [1, 1, 2] });
    expect(statGridLayout(5, true)).toEqual({ cols: 4, spans: [1, 1, 1, 1, 4] });
    expect(statGridFit([20, 20, 20, 20], 370, true)).toEqual({ step: 0, single: false, two: true });
    expect(statGridFit([106, 120, 136, 100], 370, true)).toEqual({ step: 0, single: false, two: true });
    expect(statGridFit([60, 60, 240], 342, false)).toEqual({ step: 0, single: false, two: false });
  });
  it("draws the numbers at the grid's step, in a line of the largest size's height", () => {
    render(<StatTile label="Cost" value="$1,041.13" />);
    const value = screen.getByText("$1,041.13");
    expect(value).toHaveClass("text-title-lg", "truncate");
    expect(value.parentElement).toHaveClass("h-34");
  });
});
