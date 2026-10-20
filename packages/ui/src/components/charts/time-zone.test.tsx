import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TimeseriesChart } from "./timeseries.js";
import { BarChart } from "./bar.js";
import { fromWall, wallClock, wallDay, zoneTicks } from "./time-axis.js";
import { setFormatTimeZone } from "../../format/index.js";

const zoneBefore = process.env.TZ;
const DAY = 86_400_000;
const utcDays = (from: string, n: number) => Array.from({ length: n }, (_, i) => new Date(Date.parse(`${from}T00:00:00Z`) + i * DAY));
const iso = (d: Date) => d.toISOString().slice(0, 10);

let width = 640;
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => width);
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    () => ({ left: 0, top: 0, width, height: 240, right: width, bottom: 240, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect,
  );
});
afterEach(() => {
  vi.restoreAllMocks();
  setFormatTimeZone(undefined);
  width = 640;
});
afterAll(() => {
  process.env.TZ = zoneBefore;
});

const axisText = (container: HTMLElement) => [...container.querySelectorAll("svg text[text-anchor]")].map((t) => t.textContent);
const tooltip = (container: HTMLElement) => container.querySelector("div[role=presentation]")?.textContent ?? "";
const hoverAt = (container: HTMLElement, clientX: number) =>
  fireEvent.pointerMove(container.querySelector("rect[fill=transparent]")!, { clientX, clientY: 100 });

describe.each(["Europe/Berlin", "America/Los_Angeles", "Asia/Tokyo", "UTC"])("in a browser in %s", (zone) => {
  beforeEach(() => {
    process.env.TZ = zone;
  });

  it("names UTC days by their UTC date on the axis, in the tooltip and in the table", () => {
    const times = utcDays("2026-09-21", 30);
    const { container } = render(
      <TimeseriesChart
        aria-label="Requests"
        timeZone="UTC"
        series={[{ id: "r", label: "Requests", points: times.map((t, i) => ({ t, v: i === 0 ? 2249 : 100 })) }]}
      />,
    );
    expect(axisText(container).every((l) => /^(Sep|Oct) \d+$/.test(l ?? ""))).toBe(true);
    expect(axisText(container)).not.toContain("Sep 20");
    hoverAt(container, 0);
    expect(tooltip(container)).toMatch(/^Mon, Sep 21Requests2,249/);
    hoverAt(container, width - 52);
    expect(tooltip(container)).toMatch(/^Tue, Oct 20/);
    expect(screen.getByRole("table", { hidden: true })).toHaveTextContent("Mon, Sep 21");
  });

  it("puts each month on its own tick over a long span", () => {
    const times = Array.from({ length: 13 }, (_, i) => new Date(Date.UTC(2025, 9 + i, 1)));
    const { container } = render(
      <TimeseriesChart aria-label="Requests" timeZone="UTC" series={[{ id: "r", label: "Requests", points: times.map((t) => ({ t, v: 5 })) }]} />,
    );
    const labels = axisText(container);
    expect(labels).toContain("2026");
    const ticks = [...container.querySelectorAll("svg text[text-anchor]")];
    const oct = ticks.find((t) => t.textContent === "Oct");
    expect(oct?.getAttribute("x")).toBe("0");
  });

  it("labels the points themselves like a bar chart over the same days", () => {
    const times = utcDays("2026-09-21", 30);
    const names = times.map((d) => `d${iso(d).slice(8)}`);
    const { container } = render(
      <div>
        <TimeseriesChart
          aria-label="Requests"
          timeZone="UTC"
          tickAt="points"
          formatTick={(d) => `d${iso(d).slice(8)}`}
          series={[{ id: "r", label: "Requests", points: times.map((t) => ({ t, v: 5 })) }]}
        />
        <BarChart aria-label="Tokens" categories={names} dates={times.map(iso)} series={[{ id: "t", label: "Tokens", values: times.map(() => 5) }]} />
      </div>,
    );
    const [line, bars] = [...container.querySelectorAll("figure")].map((f) =>
      [...f.querySelectorAll("svg text[text-anchor=middle]")].map((t) => t.textContent),
    );
    const [lineAt, barsAt] = [...container.querySelectorAll("figure")].map((f) =>
      [...f.querySelectorAll("svg text[text-anchor=middle]")].map((t) => Math.round(Number(t.getAttribute("x")))),
    );
    expect(line).toEqual(bars);
    expect(lineAt!.every((at, i) => Math.abs(at - barsAt![i]!) <= 1)).toBe(true);
    expect(line).toContain("d20");
    const gaps = new Set(line!.slice(1).map((l, i) => names.indexOf(l!) - names.indexOf(line![i]!)));
    expect(gaps.size).toBe(1);
  });
});

describe("the time axis", () => {
  it("keeps at least two evenly fitted labels on a phone", () => {
    process.env.TZ = "Europe/Berlin";
    width = 327;
    const times = utcDays("2026-09-21", 30);
    const { container } = render(
      <TimeseriesChart aria-label="Requests" timeZone="UTC" series={[{ id: "r", label: "Requests", points: times.map((t) => ({ t, v: 5 })) }]} />,
    );
    expect(axisText(container).length).toBeGreaterThanOrEqual(2);
  });

  it("reads hours in the formatter's zone by default", () => {
    process.env.TZ = "America/Los_Angeles";
    setFormatTimeZone("Europe/Berlin");
    const start = Date.UTC(2026, 9, 15, 22);
    const times = Array.from({ length: 24 }, (_, i) => start + i * 3_600_000);
    const { container } = render(<TimeseriesChart aria-label="Requests" series={[{ id: "r", label: "Requests", points: times.map((t) => ({ t, v: 5 })) }]} />);
    const labels = axisText(container);
    expect(labels.length).toBeGreaterThanOrEqual(2);
    expect(labels.every((l) => /^\d{2}:00$/.test(l ?? ""))).toBe(true);
    expect(labels).toContain("06:00");
  });

  it("finds wall-clock times across a daylight saving change", () => {
    const berlin = wallClock("Europe/Berlin");
    const autumn = new Date("2026-10-25T00:30:00Z");
    expect(new Date(berlin(autumn)).toISOString()).toBe("2026-10-25T02:30:00.000Z");
    expect(fromWall(Date.parse("2026-10-26T00:00:00Z"), berlin).toISOString()).toBe("2026-10-25T23:00:00.000Z");
    expect(wallDay(new Date("2026-10-25T22:30:00Z"), berlin)).toBe("2026-10-25");
    const ticks = zoneTicks(new Date("2026-10-24T22:00:00Z"), new Date("2026-10-27T23:00:00Z"), 4, berlin);
    expect(ticks.map((t) => t.toISOString())).toEqual([
      "2026-10-24T22:00:00.000Z",
      "2026-10-25T23:00:00.000Z",
      "2026-10-26T23:00:00.000Z",
      "2026-10-27T23:00:00.000Z",
    ]);
    process.env.TZ = "America/Los_Angeles";
    expect(wallDay(new Date("2026-10-25T03:00:00Z"), wallClock(undefined))).toBe("2026-10-24");
    expect(wallDay(new Date("2026-10-25T03:00:00Z"), wallClock("UTC"))).toBe("2026-10-25");
  });
});

describe("partial points and shares", () => {
  it("dashes the line to an unfinished last point and says so in the tooltip", () => {
    process.env.TZ = "Europe/Berlin";
    const times = utcDays("2026-10-14", 7);
    const { container } = render(
      <TimeseriesChart
        aria-label="Requests"
        timeZone="UTC"
        partial={{ ongoing: true, first: true }}
        series={[{ id: "r", label: "Requests", points: times.map((t) => ({ t, v: 5 })) }]}
      />,
    );
    expect(container.querySelectorAll("path[data-partial]")).toHaveLength(2);
    hoverAt(container, width - 52);
    expect(tooltip(container)).toMatch(/^Tue, Oct 20, so far/);
    hoverAt(container, 0);
    expect(tooltip(container)).toMatch(/^Wed, Oct 14, partial/);
  });

  it("leaves shares out when one series is part of another", () => {
    const times = utcDays("2026-10-14", 7);
    const series = [
      { id: "r", label: "Requests", points: times.map((t) => ({ t, v: 90 })) },
      { id: "e", label: "Errors", points: times.map((t) => ({ t, v: 10 })) },
    ];
    const { container, rerender } = render(<TimeseriesChart aria-label="Requests" timeZone="UTC" series={series} />);
    hoverAt(container, 100);
    expect(tooltip(container)).toContain("90%");
    rerender(<TimeseriesChart aria-label="Requests" timeZone="UTC" series={series} shares={false} />);
    hoverAt(container, 100);
    expect(tooltip(container)).not.toContain("%");
  });

  it("draws an unfinished last bar lighter, says so, and can leave shares out", () => {
    const series = [
      { id: "r", label: "Requests", values: [90, 90, 30] },
      { id: "e", label: "Errors", values: [10, 10, 3] },
    ];
    const { container } = render(
      <BarChart aria-label="Requests" categories={["18 Oct", "19 Oct", "20 Oct"]} series={series} partial={{ ongoing: true }} shares={false} />,
    );
    const opacities = [...container.querySelectorAll("path")].map((p) => p.getAttribute("opacity"));
    expect(opacities.slice(-2)).toEqual(["0.5", "0.5"]);
    expect(opacities[0]).toBe("1");
    fireEvent.keyDown(screen.getByRole("group", { name: "Requests" }), { key: "End" });
    expect(tooltip(container)).toMatch(/^20 Oct, so far/);
    expect(tooltip(container)).not.toContain("%");
    expect(screen.getByRole("table", { hidden: true })).toHaveTextContent("20 Oct, so far");
  });

  it("calls a cut-off last point partial when it isn't still counting", () => {
    const { container } = render(
      <TimeseriesChart
        aria-label="Requests"
        timeZone="UTC"
        partial={{ last: true }}
        series={[{ id: "r", label: "Requests", points: utcDays("2026-10-14", 3).map((t) => ({ t, v: 5 })) }]}
      />,
    );
    expect(container.querySelectorAll("path[data-partial]")).toHaveLength(1);
    expect(screen.getByRole("table", { hidden: true })).toHaveTextContent("Fri, Oct 16, partial");
    render(
      <BarChart
        aria-label="Weeks"
        categories={["Week of 5 Oct", "Week of 12 Oct"]}
        series={[{ id: "r", label: "Requests", values: [3, 1] }]}
        partial={{ last: true }}
      />,
    );
    expect(screen.getAllByRole("table", { hidden: true })[1]).toHaveTextContent("Week of 12 Oct, partial");
  });
});
