import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChartFrame, ChartLive, ChartTooltip, Legend, Swatch } from "./core.js";

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

describe("chart parts", () => {
  it("forward their refs to their root elements", () => {
    const frame = createRef<HTMLElement>();
    const legend = createRef<HTMLUListElement>();
    const swatch = createRef<HTMLSpanElement>();
    const tooltip = createRef<HTMLDivElement>();
    const live = createRef<HTMLDivElement>();
    render(
      <ChartFrame
        ref={frame}
        label="Requests per day"
        height={120}
        legend={<Legend ref={legend} items={[{ id: "ok", label: "Succeeded", color: "var(--k-viz-1)" }]} />}
      >
        <Swatch ref={swatch} color="var(--k-viz-2)" />
        <ChartTooltip
          ref={tooltip}
          x={10}
          y={10}
          width={400}
          heading="12 Oct"
          rows={[{ id: "ok", label: "Succeeded", color: "var(--k-viz-1)", value: "1,204" }]}
          visible
        />
        <ChartLive ref={live}>12 Oct: Succeeded 1,204</ChartLive>
      </ChartFrame>,
    );
    expect(frame.current).toBe(screen.getByRole("figure", { name: "Requests per day" }));
    expect(legend.current).toBe(screen.getByRole("list", { name: "Legend" }));
    expect(swatch.current).toHaveAttribute("aria-hidden");
    expect(swatch.current).toHaveClass("size-10", "rounded-bar");
    expect(tooltip.current).toHaveTextContent("12 Oct");
    expect(tooltip.current).toHaveAttribute("role", "presentation");
    expect(live.current).toHaveAttribute("aria-live", "polite");
    expect(live.current).toHaveTextContent("12 Oct: Succeeded 1,204");
  });
});
