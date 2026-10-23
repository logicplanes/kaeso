import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChartFrame, Legend } from "./core.js";

const table = { columns: ["Day", "Input"], rows: [["2 Oct", 1200]] };
const items = [
  { id: "input", label: "Input", color: "var(--k-data-1)" },
  { id: "cache_read", label: "Cache read", color: "var(--k-data-2)" },
];

describe("ChartFrame", () => {
  it("keeps View as table apart from the legend: at the end of its last line, or on a line of its own under it when narrow", async () => {
    const { container } = render(
      <ChartFrame label="Tokens by kind per day" height={200} table={table} legend={<Legend items={items} />}>
        <svg />
      </ChartFrame>,
    );
    expect(screen.getByRole("figure", { name: "Tokens by kind per day" })).toHaveClass("@container");
    const toggle = screen.getByRole("button", { name: "View as table" });
    const legend = screen.getByRole("list", { name: "Legend" });
    const row = toggle.parentElement!;
    expect(row).toHaveClass("flex-col", "@chart-legend:flex-row");
    expect(toggle).toHaveClass("ml-auto", "self-end", "shrink-0");
    expect(legend.parentElement).toHaveClass("@chart-legend:flex-1");
    expect(row.contains(legend)).toBe(true);
    expect(legend.contains(toggle)).toBe(false);

    await userEvent.click(toggle);
    expect(screen.queryByRole("list", { name: "Legend" })).toBeNull();
    expect(screen.getByRole("button", { name: "View as chart" }).parentElement!.children).toHaveLength(1);
    expect(container.querySelector("table")).not.toBeNull();
  });
});
