import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BarChart } from "./bar.js";
import { TimeseriesChart } from "./timeseries.js";
import { Donut } from "./donut.js";
import { Sparkline } from "./sparkline.js";
import { Sankey } from "./sankey.js";
import { Legend, Swatch, assignSeries } from "./core.js";
import { valueExtent } from "./domain.js";

let width = 640;
beforeEach(() => {
  width = 640;
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => width);
});
afterEach(() => vi.restoreAllMocks());

const texts = (el: Element) => [...el.querySelectorAll("text")].map((t) => t.textContent);
const day = (d: number) => new Date(Date.UTC(2026, 9, d)).toISOString();
const live = () => document.querySelector("[aria-live=polite]")?.textContent ?? "";

describe("the shared rule for edge cases", () => {
  it("spans zero and every finite value, and is [0, 1] with nothing to show", () => {
    expect(valueExtent([3, -2, Number.NaN])).toEqual([-2 * 1.08, 3 * 1.08]);
    expect(valueExtent([5], [10])).toEqual([0, 10 * 1.08]);
    expect(valueExtent([])).toEqual([0, 1]);
    expect(valueExtent([0, 0])).toEqual([0, 1]);
  });

  it("gives a lone series series 1", () => {
    expect([...assignSeries(["input"]).values()]).toEqual([0]);
  });
});

describe("BarChart edge cases", () => {
  const week = ["Mon", "Tue", "Wed"];

  it("grows a negative bar down from the zero line, which is the strong rule", () => {
    const { container } = render(<BarChart aria-label="Change" categories={week} series={[{ id: "c", label: "Change", values: [4, -3, 2] }]} />);
    const baseline = [...container.querySelectorAll("line")].find((l) => l.getAttribute("stroke") === "var(--k-line-strong)")!;
    const zero = Number(baseline.getAttribute("y1")) - 0.5;
    const bars = [...container.querySelectorAll("path")].map((p) => p.getAttribute("d")!);
    const ends = bars.map((d) => [...d.matchAll(/V([\d.]+)/g)].map((m) => Number(m[1])));
    expect(Math.min(...ends[0]!)).toBeLessThan(zero);
    expect(Math.max(...ends[1]!)).toBeGreaterThan(zero);
    expect(texts(container)).toContain("-4");
  });

  it("treats zeros as data with a single 0 on the axis, and nothing at all as empty", () => {
    const { container, rerender } = render(<BarChart aria-label="Requests" categories={week} series={[{ id: "r", label: "Requests", values: [0, 0, 0] }]} />);
    expect(screen.queryByText("No data for this period")).toBeNull();
    expect(texts(container).filter((t) => /^\d/.test(t ?? ""))).toEqual(["0"]);
    rerender(<BarChart aria-label="Requests" categories={[]} series={[]} />);
    expect(screen.getByText("No data for this period")).toBeInTheDocument();
  });

  it("writes huge values on the axis in short form and makes room for the widest label", () => {
    const { container } = render(<BarChart aria-label="Tokens" categories={week} series={[{ id: "t", label: "Tokens", values: [1e12, 2e12, 3e12] }]} />);
    expect(texts(container)).toContain("3T");
    expect(screen.getByRole("table", { hidden: true })).toHaveTextContent("3,000,000,000,000");
  });

  it("ends a long category name in an ellipsis inside its margin, keeping the full name", () => {
    width = 320;
    const name = "checkout-web-production-eu-west-1";
    const { container } = render(
      <BarChart aria-label="Spend" layout="horizontal" categories={[name, "staging"]} series={[{ id: "s", label: "Spend", values: [12, 3] }]} />,
    );
    const label = [...container.querySelectorAll("text")].find((t) => t.querySelector("title")?.textContent === name)!;
    expect(label.firstChild?.textContent).toMatch(/…$/);
    expect(Number(label.getAttribute("x"))).toBe(-12);
  });

  it("sets two series across side by side within the row, each with its own value", () => {
    const { container } = render(
      <BarChart
        aria-label="Tokens by key"
        layout="horizontal"
        categories={["a", "b"]}
        series={[
          { id: "i", label: "Input", values: [10, 20] },
          { id: "o", label: "Output", values: [5, 8] },
        ]}
      />,
    );
    const ys = [...container.querySelectorAll("path")].map((p) => Number(/^M[\d.]+ ([\d.]+)/.exec(p.getAttribute("d")!)?.[1]));
    expect(new Set(ys).size).toBe(4);
    expect(texts(container)).toEqual(expect.arrayContaining(["10", "5", "20", "8"]));
  });

  it("toggles series from the legend, and says so when every series is hidden", async () => {
    render(
      <BarChart
        aria-label="Tokens"
        categories={week}
        series={[
          { id: "i", label: "Input", values: [1, 2, 3] },
          { id: "o", label: "Output", values: [3, 2, 1] },
        ]}
      />,
    );
    const legend = screen.getByRole("list", { name: "Legend" });
    await userEvent.click(within(legend).getByRole("button", { name: "Input" }));
    expect(within(legend).getByRole("button", { name: "Input" })).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(within(legend).getByRole("button", { name: "Output" }));
    expect(screen.getByText("Every series is hidden. Choose one in the legend to show it.")).toBeInTheDocument();
    expect(legend).toBeInTheDocument();
  });

  it("shows the table at the chart's height, with one label and no pressed state on the toggle", async () => {
    render(<BarChart aria-label="Tokens" categories={week} series={[{ id: "t", label: "Tokens", values: [1, 2, 3] }]} />);
    const toggle = screen.getByRole("button", { name: "View as table" });
    expect(toggle).not.toHaveAttribute("aria-pressed");
    await userEvent.click(toggle);
    expect(screen.getByRole("button", { name: "View as chart" })).toBeInTheDocument();
    const region = screen.getByRole("region", { name: "Tokens, table" });
    expect(region.closest("figure > div")).toHaveStyle({ height: "240px" });
    expect(within(region).getAllByRole("columnheader")[1]).toHaveClass("text-right", "sticky");
  });
});

describe("TimeseriesChart edge cases", () => {
  it("labels each of seven days once", () => {
    width = 900;
    const points = Array.from({ length: 7 }, (_, i) => ({ t: day(6 + i), v: i + 1 }));
    const { container } = render(<TimeseriesChart aria-label="Spend" timeZone="UTC" series={[{ id: "s", label: "Spend", points }]} />);
    const days = texts(container).filter((t) => /Oct/.test(t ?? ""));
    expect(days.length).toBeGreaterThan(2);
    expect(new Set(days).size).toBe(days.length);
  });

  it("draws a lone point as a dot under the day, and a negative value inside the plot", () => {
    const { container, rerender } = render(
      <TimeseriesChart aria-label="Requests" timeZone="UTC" series={[{ id: "r", label: "Requests", points: [{ t: day(12), v: 4812 }] }]} />,
    );
    expect(container.querySelectorAll("circle[data-point]")).toHaveLength(1);
    expect(texts(container)).toContain("12 Oct");
    rerender(
      <TimeseriesChart aria-label="Change" timeZone="UTC" series={[{ id: "c", label: "Change", points: [4, -6, 2].map((v, i) => ({ t: day(6 + i), v })) }]} />,
    );
    expect(texts(container)).toEqual(expect.arrayContaining(["0", "-5"]));
  });

  it("keeps the dashed partial line and a dot for a point between gaps", () => {
    const { container } = render(
      <TimeseriesChart
        aria-label="Errors"
        timeZone="UTC"
        series={[{ id: "e", label: "Errors", points: [1, Number.NaN, 3, Number.NaN, 2].map((v, i) => ({ t: day(6 + i), v })) }]}
      />,
    );
    expect(container.querySelectorAll("circle[data-point]")).toHaveLength(3);
    expect(screen.getByRole("table", { hidden: true })).toHaveTextContent("–");
  });
});

describe("Donut edge cases", () => {
  it("fills its width, groups past the eighth slice and hides the center when empty", () => {
    const slices = Array.from({ length: 11 }, (_, i) => ({ id: `m${i}`, label: `model-${i}`, value: 20 - i }));
    const { container, rerender } = render(<Donut aria-label="Spend" slices={slices} />);
    expect(container.firstElementChild).toHaveClass("w-full");
    const legend = screen.getByRole("list", { name: "Legend" });
    expect(within(legend).getAllByRole("listitem")).toHaveLength(8);
    expect(legend).toHaveTextContent("Other");
    rerender(<Donut aria-label="Spend" slices={slices.map((s) => ({ ...s, value: 0 }))} />);
    expect(screen.getByText("No data for this period")).toBeInTheDocument();
    expect(screen.queryByText("Total")).toBeNull();
  });
});

describe("Sparkline edge cases", () => {
  it("draws a quiet line with no values, a dot at the newest end for one, and centres a flat series", () => {
    const { container, rerender } = render(<Sparkline aria-label="Requests" values={[]} />);
    expect(container.querySelector("line")).toHaveAttribute("stroke", "var(--k-line-strong)");
    expect(container.querySelectorAll("circle")).toHaveLength(0);
    rerender(<Sparkline aria-label="Requests" values={[5]} />);
    expect(container.querySelector("[data-dot] path")?.getAttribute("d")).toBe("M76 12h0");
    rerender(<Sparkline aria-label="Requests" values={[5, 5, 5]} />);
    expect(container.querySelector("[data-dot] path")?.getAttribute("d")).toBe("M76 12h0");
  });
});

describe("Sankey edge cases", () => {
  const nodes = [
    { id: "web", label: "checkout-web" },
    { id: "gpt", label: "gpt-5" },
    { id: "openai", label: "OpenAI" },
  ];
  const links = [
    { source: "web", target: "gpt", value: 4200 },
    { source: "gpt", target: "openai", value: 4200 },
  ];

  it("is empty without flows, and walks its nodes from the keyboard", async () => {
    const { rerender } = render(<Sankey aria-label="Routes" nodes={nodes} links={[]} />);
    expect(screen.getByText("No data for this period")).toBeInTheDocument();
    rerender(<Sankey aria-label="Routes" nodes={nodes} links={links} />);
    await userEvent.tab();
    expect(live()).toBe("checkout-web: 4,200");
    await userEvent.keyboard("{ArrowRight}");
    expect(live()).toBe("gpt-5: 4,200");
  });

  it("drops the values before it shortens names when narrow, keeping the full name", () => {
    width = 260;
    const { container } = render(<Sankey aria-label="Routes" nodes={nodes} links={links} />);
    expect(container.querySelectorAll("tspan")).toHaveLength(0);
    expect(container.querySelector("text title")).toHaveTextContent(/4,200$/);
  });
});

describe("legend and swatch", () => {
  it("draws an off swatch as an outline in its color, keeps data colors in forced colors, and edges a soft tint", () => {
    const { container } = render(
      <>
        <Swatch color="var(--k-viz-2)" off />
        <Swatch color="var(--k-viz-1-soft)" />
      </>,
    );
    const [off, soft] = [...container.querySelectorAll<HTMLElement>("span")];
    expect(off).toHaveClass("data-mark");
    expect(off?.style.boxShadow).toBe("inset 0 0 0 1px var(--k-viz-2)");
    expect(off?.style.background).toBe("");
    expect(soft?.style.boxShadow).toBe("inset 0 0 0 1px var(--k-viz-1)");
  });

  it("is plain text without onToggle, so it never looks like a control", () => {
    render(<Legend items={[{ id: "a", label: "OpenAI", color: "var(--k-viz-1)" }]} />);
    const item = screen.getByText("OpenAI");
    expect(item).not.toHaveClass("rounded-pill");
    expect(screen.queryByRole("button")).toBeNull();
  });
});

describe("tooltip", () => {
  it("stays inside the chart at its bottom edge", () => {
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(80);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(160);
    const { container } = render(
      <BarChart aria-label="Tokens" height={120} categories={["a"]} series={[{ id: "t", label: "Tokens", values: [3] }]} highlight={0} />,
    );
    const tip = container.querySelector<HTMLElement>("div[role=presentation]")!;
    act(() => void fireEvent.pointerMove(container.querySelector("rect[fill=transparent]")!, { clientY: 118 }));
    expect(Number.parseFloat(tip.style.top)).toBeLessThanOrEqual(120);
  });
});
