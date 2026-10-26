import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BarChart } from "./bar.js";
import { TimeseriesChart } from "./timeseries.js";
import { chartKeyIndex } from "./core.js";

const days = ["Oct 10", "Oct 11", "Oct 12"];
const bars = [
  { id: "input", label: "Input", values: [1200, 900, 1500] },
  { id: "output", label: "Output", values: [300, 250, 410] },
];
const live = () => document.querySelector("[aria-live=polite]")?.textContent ?? "";

describe("chartKeyIndex", () => {
  it("steps through the categories and stops at the ends", () => {
    expect(chartKeyIndex("ArrowLeft", 2, 3)).toBe(1);
    expect(chartKeyIndex("ArrowLeft", 0, 3)).toBe(0);
    expect(chartKeyIndex("ArrowRight", 2, 3)).toBe(2);
    expect(chartKeyIndex("Home", 2, 3)).toBe(0);
    expect(chartKeyIndex("End", 0, 3)).toBe(2);
  });
  it("starts from the newest point, or from where it's told", () => {
    expect(chartKeyIndex("ArrowLeft", null, 3)).toBe(2);
    expect(chartKeyIndex("ArrowDown", null, 3, { vertical: true, start: 0 })).toBe(0);
  });
  it("answers the arrows of its own direction only", () => {
    expect(chartKeyIndex("ArrowLeft", 1, 3, { vertical: true })).toBeUndefined();
    expect(chartKeyIndex("ArrowDown", 1, 3)).toBeUndefined();
    expect(chartKeyIndex("Tab", 1, 3)).toBeUndefined();
  });
  it("clears with Escape, and lets an Escape with nothing highlighted carry on", () => {
    expect(chartKeyIndex("Escape", 1, 3)).toBeNull();
    expect(chartKeyIndex("Escape", null, 3)).toBeUndefined();
  });
  it("does nothing without data", () => {
    expect(chartKeyIndex("ArrowLeft", null, 0)).toBeUndefined();
  });
});

describe("BarChart from the keyboard", () => {
  it("is one tab stop whose arrow keys move through the days and say each one", async () => {
    render(<BarChart aria-label="Tokens per day" categories={days} series={bars} />);
    await userEvent.tab();
    const plot = screen.getByRole("group", { name: "Tokens per day" });
    expect(plot).toHaveFocus();
    await userEvent.keyboard("{End}{ArrowLeft}");
    expect(live()).toBe("Oct 11: Input 900, Output 250");
    await userEvent.keyboard("{Home}");
    expect(live()).toBe("Oct 10: Input 1,200, Output 300");
    await userEvent.keyboard("{Escape}");
    expect(live()).toBe("");
  });
  it("moves a ranking with the up and down arrows", async () => {
    render(
      <BarChart
        aria-label="Spend by key"
        layout="horizontal"
        categories={["checkout-web", "ci-runner"]}
        series={[{ id: "spend", label: "Spend", values: [84, 12] }]}
      />,
    );
    await userEvent.tab();
    await userEvent.keyboard("{Home}{ArrowDown}");
    expect(live()).toBe("ci-runner: Spend 12");
  });
  it("clears the highlight when the focus leaves", async () => {
    render(
      <>
        <BarChart aria-label="Tokens per day" categories={days} series={bars} />
        <button type="button">After</button>
      </>,
    );
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    expect(live()).not.toBe("");
    await userEvent.tab();
    expect(live()).toBe("");
  });
});

describe("TimeseriesChart from the keyboard", () => {
  const t = (d: number) => new Date(Date.UTC(2026, 9, d)).toISOString();
  const series = [{ id: "gpt-5", label: "GPT-5", points: [10, 11, 12].map((d, i) => ({ t: t(d), v: [4, 7, 5][i]! })) }];
  it("moves the crosshair a point at a time and says its values", async () => {
    render(<TimeseriesChart aria-label="Spend" series={series} />);
    await userEvent.tab();
    expect(screen.getByRole("group", { name: "Spend" })).toHaveFocus();
    await userEvent.keyboard("{Home}{ArrowRight}");
    expect(live()).toMatch(/11 Oct: GPT-5 7$/);
  });
  it("has nothing to step through while empty", async () => {
    render(<TimeseriesChart aria-label="Spend" series={series.map((s) => ({ ...s, points: [] }))} />);
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    expect(live()).toBe("");
  });
});

describe("charts of weeks and months", () => {
  it("BarChart says the full category where the axis has a short one", async () => {
    render(
      <BarChart
        aria-label="Tokens per week"
        categories={["Week of 3 Aug", "Week of 10 Aug"]}
        axisLabels={["3 Aug", "10 Aug"]}
        series={[{ id: "input", label: "Input", values: [5, 7] }]}
      />,
    );
    await userEvent.tab();
    await userEvent.keyboard("{Home}");
    expect(live()).toBe("Week of 3 Aug: Input 5");
  });
  it("TimeseriesChart names a point by formatTime", async () => {
    const weeks = [new Date(2026, 7, 3), new Date(2026, 7, 10)];
    render(
      <TimeseriesChart
        aria-label="Requests per week"
        series={[{ id: "requests", label: "Requests", points: weeks.map((t, i) => ({ t, v: i + 1 })) }]}
        formatTime={(t) => `Week of ${t.getDate()} Aug`}
      />,
    );
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    expect(live()).toBe("Week of 10 Aug: Requests 2");
  });
});
