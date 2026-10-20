import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TimeseriesChart } from "./timeseries.js";
import { BarChart } from "./bar.js";
import { Donut, ShareBar } from "./donut.js";
import { ChartTooltip, Legend, seriesColor, seriesColors, tokenKindColor, tokenKindLabel, TOKEN_KIND_ORDER } from "./core.js";
import { formatTokens, setFormatLocale } from "../../format/index.js";

const hours = (n: number, start = Date.UTC(2026, 9, 15)) => Array.from({ length: n }, (_, i) => start + i * 3_600_000);
const days = (n: number, start = Date.UTC(2026, 0, 1)) => Array.from({ length: n }, (_, i) => start + i * 86_400_000);
const series = (times: number[]) => [
  { id: "gpt-5", label: "gpt-5", points: times.map((t, i) => ({ t, v: 100 + i * 10 })) },
  { id: "claude", label: "Claude Sonnet 4.5", points: times.map((t, i) => ({ t, v: 50 + i })) },
];

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(640);
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    width: 640,
    height: 240,
    right: 640,
    bottom: 240,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  } as DOMRect);
});
afterEach(() => vi.restoreAllMocks());

describe("TimeseriesChart", () => {
  it("draws lines, shows a tooltip on hover and hides a series from the legend", async () => {
    const { container } = render(<TimeseriesChart aria-label="Requests" series={series(hours(24))} threshold={{ value: 200, label: "Limit" }} />);
    const plot = screen.getByRole("group", { name: "Requests" });
    expect(container.querySelectorAll("path").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("Limit")).toBeInTheDocument();
    fireEvent.pointerMove(plot.querySelector("rect[fill=transparent]")!, { clientX: 320, clientY: 100 });
    expect(container.querySelectorAll("circle").length).toBe(2);
    fireEvent.pointerLeave(plot.querySelector("rect[fill=transparent]")!);
    const legend = screen.getByRole("list", { name: "Legend" });
    await userEvent.click(within(legend).getByRole("button", { name: /Claude/ }));
    expect(within(legend).getByRole("button", { name: /Claude/ })).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(within(legend).getByRole("button", { name: /Claude/ }));
    expect(within(legend).getByRole("button", { name: /Claude/ })).toHaveAttribute("aria-pressed", "true");
    fireEvent.mouseEnter(within(legend).getByRole("button", { name: /gpt-5/ }));
    fireEvent.focus(within(legend).getByRole("button", { name: /gpt-5/ }));
    fireEvent.blur(within(legend).getByRole("button", { name: /gpt-5/ }));
    fireEvent.mouseLeave(within(legend).getByRole("button", { name: /gpt-5/ }));
  });

  it("stacks areas and fills areas, labels months over a long span, and moves with the keyboard", () => {
    const { container, rerender } = render(<TimeseriesChart aria-label="Tokens" kind="stacked" series={series(days(200))} highlight={3} />);
    expect(container.querySelectorAll(".chart-stack-fill")).toHaveLength(2);
    expect(screen.getByRole("group", { name: "Tokens" })).toHaveTextContent("2026");
    rerender(<TimeseriesChart aria-label="Tokens" kind="area" series={series(days(30))} formatTime={(d) => d.toISOString().slice(0, 10)} hideLegend />);
    expect(container.querySelectorAll("linearGradient")).toHaveLength(2);
    expect(screen.queryByRole("list", { name: "Legend" })).toBeNull();
    const plot = screen.getByRole("group", { name: "Tokens" });
    fireEvent.keyDown(plot, { key: "ArrowLeft" });
    fireEvent.keyDown(plot, { key: "Home" });
    fireEvent.keyDown(plot, { key: "Escape" });
    fireEvent.keyDown(plot, { key: "Escape" });
    fireEvent.keyDown(plot, { key: "x" });
  });

  it("shows loading, empty and failed states", async () => {
    const onRetry = vi.fn();
    const { rerender } = render(<TimeseriesChart aria-label="Spend" series={series(hours(4))} loading />);
    expect(screen.getByRole("group", { name: "Spend" })).not.toHaveTextContent("150");
    rerender(<TimeseriesChart aria-label="Spend" series={[]} loading />);
    expect(screen.queryByRole("group", { name: "Spend" })).not.toBeInTheDocument();
    expect(screen.getByRole("figure", { name: "Spend" }).querySelector("svg")).toBeNull();
    expect(screen.queryByText("No data for this period")).not.toBeInTheDocument();
    rerender(<TimeseriesChart aria-label="Spend" series={[{ id: "a", label: "A", points: [] }]} />);
    expect(screen.getByText("No data for this period")).toBeInTheDocument();
    rerender(<TimeseriesChart aria-label="Spend" series={series(hours(4))} error="down" onRetry={onRetry} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load this chart");
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalled();
  });
});

describe("BarChart", () => {
  const categories = ["Mon", "Tue", "Wed", "Thu"];
  const bars = [
    { id: "input", label: "Input", values: [10, 20, 0, 40] },
    { id: "output", label: "Output", values: [5, 0, 15, 20] },
  ];

  it("groups, stacks and ranks bars, with a tooltip per category", () => {
    const { container, rerender } = render(
      <BarChart aria-label="Tokens" categories={categories} series={bars} highlight={1} axisLabels={["M", "T", "W", "T"]} />,
    );
    const plot = screen.getByRole("group", { name: "Tokens" });
    expect(container.querySelectorAll("path").length).toBeGreaterThan(0);
    const hit = plot.querySelectorAll("rect[fill=transparent]")[2]!;
    fireEvent.pointerMove(hit, { clientY: 80 });
    fireEvent.pointerLeave(hit);
    rerender(<BarChart aria-label="Tokens" layout="stacked" categories={categories} series={bars} />);
    expect(container.querySelectorAll("path").length).toBeGreaterThan(0);
    rerender(<BarChart aria-label="Tokens" layout="horizontal" categories={categories} series={[bars[0]!]} highlight={0} />);
    expect(screen.getByRole("group", { name: "Tokens" })).toHaveTextContent("40");
    fireEvent.keyDown(screen.getByRole("group", { name: "Tokens" }), { key: "ArrowDown" });
    rerender(<BarChart aria-label="Tokens" layout="horizontal" categories={categories} series={bars} highlight={3} />);
    fireEvent.mouseEnter(within(screen.getByRole("list", { name: "Legend" })).getByText("Input"));
  });

  it("keeps quiet while loading and with no data", () => {
    const { rerender } = render(<BarChart aria-label="Tokens" categories={categories} series={bars} loading />);
    expect(screen.getByRole("group", { name: "Tokens" })).not.toHaveTextContent("40");
    rerender(<BarChart aria-label="Tokens" categories={[]} series={[]} empty />);
    expect(screen.getByText("No data for this period")).toBeInTheDocument();
  });
});

describe("Donut and ShareBar", () => {
  const slices = [
    { id: "a", label: "Checkout assistant", value: 990 },
    { id: "b", label: "Docs search", value: 9 },
    { id: "c", label: "Fraud screening", value: 1, color: "var(--k-viz-3)" },
  ];

  it("forwards both kinds of ref, dims other slices on hover and marks tiny shares", () => {
    const ref = createRef<HTMLDivElement>();
    const fnRef = vi.fn();
    const { rerender } = render(<Donut ref={ref} aria-label="Cost" slices={slices} size="sm" total="$1,000.00" />);
    expect(ref.current).not.toBeNull();
    expect(screen.getByText("$1,000.00")).toBeInTheDocument();
    const legend = screen.getByRole("list", { name: "Legend" });
    expect(within(legend).getByText("Other")).toBeInTheDocument();
    fireEvent.mouseEnter(within(legend).getByText("Checkout assistant").closest("li")!);
    expect(within(legend).getByText("Other").closest("li")).toHaveClass("opacity-50");
    fireEvent.mouseLeave(within(legend).getByText("Checkout assistant").closest("li")!);
    rerender(<Donut ref={fnRef} aria-label="Cost" slices={slices} size="lg" />);
    expect(fnRef).toHaveBeenCalled();
  });

  it("splits a bar by share with its own legend, or none", () => {
    const { container, rerender } = render(
      <ShareBar aria-label="Spend by project" slices={[...slices, { id: "d", label: "Unused", value: 0 }]} formatValue={(v) => `$${v}`} />,
    );
    expect(screen.getByRole("img", { name: /Checkout assistant 99%/ })).toBeInTheDocument();
    expect(screen.getByTitle("Checkout assistant: $990")).toBeInTheDocument();
    rerender(<ShareBar aria-label="Spend by project" slices={[]} legend={false} />);
    expect(container.querySelector("ul")).toBeNull();
  });
});

describe("chart helpers", () => {
  it("names token kinds and gives stable colors, thin or full", () => {
    expect(TOKEN_KIND_ORDER.map((k) => tokenKindLabel(k))).toEqual(["Input", "Cache read", "Cache write", "Output", "Reasoning"]);
    expect(tokenKindColor("input", true)).not.toBe(tokenKindColor("input"));
    expect(seriesColor("gpt-5")).toBe(seriesColor("gpt-5"));
    expect(seriesColor("gpt-5", true)).toContain("var(");
    const colors = seriesColors(["a", "b", "a"], true);
    expect(colors.size).toBe(2);
  });

  it("renders a legend without toggles and a tooltip that flips at the right edge", () => {
    render(
      <>
        <Legend items={[{ id: "a", label: "Input", color: "red", value: "12" }]} />
        <ChartTooltip
          x={600}
          xLeft={560}
          y={20}
          width={640}
          visible
          heading="Mon"
          rows={[{ id: "a", label: "Input", value: "12", color: "red", share: "40%" }]}
        />
      </>,
    );
    expect(screen.getAllByText("12")).toHaveLength(2);
    expect(screen.getByText("40%")).toBeInTheDocument();
  });
});

describe("compact token counts on charts, in every language", () => {
  it.each([
    ["en", "103M", "100M"],
    ["de", "103\u00a0Mio.", "100\u00a0Mio."],
    ["zh-CN", "1.03亿", "1亿"],
  ])("%s: labels a 103 million bar %s, and the axis line under it %s", (locale, bar, tick) => {
    setFormatLocale(locale);
    const { container } = render(
      <BarChart
        aria-label="Tokens per day"
        categories={["13 Oct", "14 Oct"]}
        series={[{ id: "tokens", label: "Tokens", values: [103_000_000, 51_000_000] }]}
        formatValue={formatTokens}
      />,
    );
    expect(screen.getByRole("cell", { name: bar })).toBeInTheDocument();
    expect([...container.querySelectorAll("svg text")].map((n) => n.textContent)).toContain(tick);
    setFormatLocale("en");
  });
});
