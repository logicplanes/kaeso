import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { TimeseriesChart } from "./timeseries.js";

const hours = (n: number) => Array.from({ length: n }, (_, i) => Date.UTC(2026, 9, 15) + i * 3_600_000);

describe("a chart's table for screen readers", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("waits for the browser's first idle moment when the browser offers one", () => {
    let idle: (() => void) | undefined;
    const cancel = vi.fn();
    vi.stubGlobal("requestIdleCallback", (cb: () => void) => {
      idle = cb;
      return 7;
    });
    vi.stubGlobal("cancelIdleCallback", cancel);
    const points = hours(300).map((t, i) => ({ t, v: i }));
    render(<TimeseriesChart aria-label="Requests" kind="area" series={[{ id: "requests", label: "Requests", points }]} hideLegend />);
    expect(screen.queryAllByRole("row", { hidden: true })).toHaveLength(0);
    act(() => idle?.());
    expect(screen.getAllByRole("row", { hidden: true }).length).toBeGreaterThan(300);
    expect(cancel).toHaveBeenCalledWith(7);
  });
});
