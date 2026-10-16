import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { BarChart } from "./bar.js";
import { axisLabelIndexes, labelWidth, placeTimeLabels, yearStarts, type AxisDate } from "./core.js";

const MONTH = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const iso = (d: Date) => d.toISOString().slice(0, 10);

function monthDays(fromYear: number, fromMonth: number, count: number) {
  return Array.from({ length: count }, (_, i) => iso(new Date(Date.UTC(fromYear, fromMonth + i, 1))));
}
function months(fromYear: number, fromMonth: number, count: number) {
  return Array.from({ length: count }, (_, i) => {
    const m = (fromMonth + i) % 12;
    return m === 0 ? String(fromYear + Math.floor((fromMonth + i) / 12)) : MONTH[m]!;
  });
}
function weekDays(from: string, count: number) {
  const t0 = Date.parse(`${from}T00:00:00Z`);
  return Array.from({ length: count }, (_, i) => iso(new Date(t0 + i * 7 * 86_400_000)));
}
const weekLabels = (days: string[]) => days.map((d) => `${Number(d.slice(8))} ${MONTH[Number(d.slice(5, 7)) - 1]}`);

const intl = (locale: string, options: Intl.DateTimeFormatOptions) => {
  const f = new Intl.DateTimeFormat(locale, { ...options, timeZone: "UTC" });
  return (day: string) => f.format(new Date(`${day}T00:00:00Z`));
};
const monthLabelsIn = (locale: string, days: string[]) =>
  days.map((d) => (d.slice(5, 7) === "01" ? intl(locale, { year: "numeric" })(d) : intl(locale, { month: "short" })(d)));
const weekLabelsIn = (locale: string, days: string[]) => days.map(intl(locale, { day: "numeric", month: "short" }));

const fifteenDays = monthDays(2025, 7, 15);
const fifteen = months(2025, 7, 15);
const fiftyThreeDays = weekDays("2025-10-13", 53);
const fiftyThree = weekLabels(fiftyThreeDays);

const geometry = (n: number, width: number) => {
  const step = (width - 52) / n;
  return { step, edges: { start: step / 2, end: step / 2 + 52 } };
};
const shownAt = (labels: string[], width: number, dates: readonly AxisDate[]) => {
  const { step, edges } = geometry(labels.length, width);
  return axisLabelIndexes(labels, step, edges, dates);
};
const at = (labels: string[], width: number, dates: readonly AxisDate[]) => shownAt(labels, width, dates).map((i) => labels[i]);

function expectReadable(labels: string[], width: number, dates: readonly AxisDate[]) {
  const { step, edges } = geometry(labels.length, width);
  const shown = shownAt(labels, width, dates);
  const gaps = new Set(shown.slice(1).map((i, k) => i - shown[k]!));
  expect(gaps.size).toBeLessThanOrEqual(1);
  for (let k = 1; k < shown.length; k++) {
    const [a, b] = [shown[k - 1]!, shown[k]!];
    expect((b - a) * step).toBeGreaterThanOrEqual(48 - 1e-6);
    expect((b - a) * step - labelWidth(labels[a]!) / 2 - labelWidth(labels[b]!) / 2).toBeGreaterThanOrEqual(16 - 1e-6);
  }
  for (const i of shown) {
    expect(edges.start + i * step - labelWidth(labels[i]!) / 2).toBeGreaterThanOrEqual(-1e-6);
    expect(edges.end + (labels.length - 1 - i) * step - labelWidth(labels[i]!) / 2).toBeGreaterThanOrEqual(-1e-6);
  }
  return shown;
}

describe("yearStarts", () => {
  it("finds the month after December", () => {
    expect(fifteen).toEqual(["Aug", "Sep", "Oct", "Nov", "Dec", "2026", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct"]);
    expect(yearStarts(fifteenDays)).toEqual([5]);
  });
  it("finds the first week in January, and 1 January", () => {
    expect(fiftyThree.slice(11, 13)).toEqual(["29 Dec", "5 Jan"]);
    expect(yearStarts(fiftyThreeDays)).toEqual([12]);
    expect(yearStarts(["2025-12-30", "2025-12-31", "2026-01-01", "2026-01-02"])).toEqual([2]);
  });
  it("counts the first where the step before it is in another year", () => {
    expect(yearStarts(monthDays(2026, 0, 3))).toEqual([0]);
    expect(yearStarts(["2026-01-05", "2026-01-12"])).toEqual([0]);
    expect(yearStarts(["2026-01-12", "2026-01-19"])).toEqual([]);
  });
  it("reads a Date in the viewer's time zone and an ISO day as that day", () => {
    expect(yearStarts([new Date(2025, 11, 31), new Date(2026, 0, 1, 23, 30)])).toEqual([1]);
    expect(yearStarts(["2025-12-31T23:00:00Z", "2026-01-01T00:30:00+01:00"])).toEqual([1]);
  });
  it("finds none in a year's run, or where there are no days", () => {
    expect(yearStarts(["2026-10-06", "2026-10-07", "2026-10-08"])).toEqual([]);
    expect(yearStarts(["2026-08-01", "2026-09-01"])).toEqual([]);
    expect(yearStarts(["checkout-web", "may-batch", "jan-import"])).toEqual([]);
    expect(yearStarts([])).toEqual([]);
  });
});

describe("axisLabelIndexes", () => {
  it("shows every label where they fit", () => {
    expect(at(fifteen, 1100, fifteenDays)).toEqual(fifteen);
  });
  it("keeps where the year begins on fifteen monthly bars, at every width", () => {
    expect(at(fifteen, 652, fifteenDays)).toEqual(["Sep", "Nov", "2026", "Mar", "May", "Jul", "Sep"]);
    expect(at(fifteen, 390, fifteenDays)).toEqual(["Oct", "2026", "Apr", "Jul", "Oct"]);
    expect(at(fifteen, 240, fifteenDays)).toEqual(["Sep", "2026", "May", "Sep"]);
    for (let width = 200; width <= 1440; width += 10) {
      const shown = expectReadable(fifteen, width, fifteenDays);
      expect(shown).toContain(5);
    }
  });
  it("keeps where the year begins on fifty-three weekly bars, at every width", () => {
    expect(at(fiftyThree, 1100, fiftyThreeDays)).toEqual([
      "10 Nov",
      "8 Dec",
      "5 Jan",
      "2 Feb",
      "2 Mar",
      "30 Mar",
      "27 Apr",
      "25 May",
      "22 Jun",
      "20 Jul",
      "17 Aug",
      "14 Sep",
      "12 Oct",
    ]);
    expect(at(fiftyThree, 390, fiftyThreeDays)).toEqual(["5 Jan", "16 Mar", "25 May", "3 Aug", "12 Oct"]);
    for (let width = 200; width <= 1440; width += 10) {
      const shown = expectReadable(fiftyThree, width, fiftyThreeDays);
      expect(shown).toContain(12);
    }
  });
  it("keeps a year written out under weekly bars", () => {
    const labels = fiftyThree.map((l, i) => (i === 12 ? "5 Jan 2026" : l));
    expect(at(labels, 390, fiftyThreeDays)).toEqual(["5 Jan 2026", "13 Apr", "20 Jul"]);
    for (let width = 210; width <= 1440; width += 10) expect(expectReadable(labels, width, fiftyThreeDays)).toContain(12);
  });
  it("widens the step so every January keeps its year, as a time axis steps months by 1, 2, 3, 6 or 12", () => {
    const labels = months(2024, 7, 27);
    const step = 48 / 5 + 0.01;
    const shown = axisLabelIndexes(labels, step, undefined, monthDays(2024, 7, 27));
    expect(shown).toEqual([5, 11, 17, 23]);
    expect(shown.map((i) => labels[i])).toEqual(["2025", "Jul", "2026", "Jul"]);
  });
  it("never reads a year from the labels: without days, it counts back from the newest bar", () => {
    const shown = axisLabelIndexes(fifteen, 652 / 15);
    expect(shown.at(-1)).toBe(14);
    expect(shown).not.toContain(5);
  });
  it("keeps the labels English keeps in German and Chinese, at every width", () => {
    for (const locale of ["de", "zh-CN"]) {
      const monthly = monthLabelsIn(locale, fifteenDays);
      const weekly = weekLabelsIn(locale, fiftyThreeDays);
      expect(monthly[5]).toBe(locale === "de" ? "2026" : "2026年");
      expect(weekly[12]).toBe(locale === "de" ? "5. Jan." : "1月5日");
      for (let width = 200; width <= 1440; width += 10) {
        expect(expectReadable(monthly, width, fifteenDays)).toContain(5);
        expect(expectReadable(weekly, width, fiftyThreeDays)).toContain(12);
      }
    }
    for (const width of [652, 1100]) {
      expect(shownAt(monthLabelsIn("de", fifteenDays), width, fifteenDays)).toEqual(shownAt(fifteen, width, fifteenDays));
      expect(shownAt(monthLabelsIn("zh-CN", fifteenDays), width, fifteenDays)).toEqual(shownAt(fifteen, width, fifteenDays));
    }
  });
  it("counts back from the newest bar where no label says more", () => {
    const days = Array.from({ length: 30 }, (_, i) => `Sep ${i + 1}`);
    const shown = axisLabelIndexes(days, 20);
    expect(shown.at(-1)).toBe(29);
    expect(new Set(shown.slice(1).map((i, k) => i - shown[k]!)).size).toBe(1);
  });
  it("spaces long labels by their width", () => {
    const labels = Array.from({ length: 12 }, (_, i) => `Week of ${i + 1} Aug`);
    const shown = axisLabelIndexes(labels, 40);
    expect(shown[1]! - shown[0]!).toBe(3);
  });
  it("never runs a label past the chart's edges", () => {
    expect(axisLabelIndexes(["13 Oct", "20 Oct", "27 Oct", "3 Nov"], 70, { start: 6, end: 60 })).toEqual([1, 2, 3]);
    expect(axisLabelIndexes(["13 Oct", "20 Oct", "27 Oct", "3 Nov"], 70, { start: 6, end: 4 })).toEqual([1, 2]);
  });
  it("keeps one label where only one fits", () => {
    expect(axisLabelIndexes(fifteen, 2, undefined, fifteenDays)).toEqual([5]);
    expect(axisLabelIndexes([], 20)).toEqual([]);
  });
});

describe("BarChart's axis", () => {
  let width = 0;
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => width);
  });
  afterEach(() => vi.restoreAllMocks());
  const axis = (container: HTMLElement) => [...container.querySelectorAll("svg text[text-anchor=middle]")].map((t) => t.textContent);
  const draw = (labels: string[], dates: readonly AxisDate[]) => (
    <BarChart
      aria-label="Tokens by kind"
      categories={labels.map((l, i) => `Bucket ${i + 1}, ${l}`)}
      axisLabels={labels}
      dates={dates}
      layout="stacked"
      series={[{ id: "input", label: "Input", values: labels.map((_, i) => 1000 + i * 10) }]}
    />
  );
  it("names the year under fifteen monthly bars where it thins them", () => {
    width = 652;
    const { container } = render(draw(fifteen, fifteenDays));
    expect(axis(container)).toEqual(["Sep", "Nov", "2026", "Mar", "May", "Jul", "Sep"]);
  });
  it("marks the year's first week under fifty-three weekly bars on a phone", () => {
    width = 390;
    const { container } = render(draw(fiftyThree, fiftyThreeDays));
    expect(axis(container)).toEqual(["5 Jan", "16 Mar", "25 May", "3 Aug", "12 Oct"]);
  });
  it("names the year in German and Chinese where English does", () => {
    width = 652;
    const german = monthLabelsIn("de", fifteenDays);
    const de = render(draw(german, fifteenDays));
    expect(axis(de.container)).toEqual([1, 3, 5, 7, 9, 11, 13].map((i) => german[i]));
    expect(axis(de.container)).toContain("2026");
    de.unmount();
    width = 390;
    const zh = render(draw(weekLabelsIn("zh-CN", fiftyThreeDays), fiftyThreeDays));
    expect(axis(zh.container)).toEqual(["1月5日", "3月30日", "6月22日", "9月14日"]);
  });
});

describe("placeTimeLabels", () => {
  const ticks = (labels: string[], step: number) => labels.map((label, i) => ({ key: i, at: i * step, label }));

  it("aligns an edge label to the edge and drops the neighbour it would run into", () => {
    const placed = placeTimeLabels(ticks(["15. Sept.", "17. Sept.", "19. Sept.", "21. Sept."], 74), 300);
    expect(placed[0]).toMatchObject({ label: "15. Sept.", anchor: "start", x: 0 });
    expect(placed.map((p) => p.label)).toEqual(["15. Sept.", "19. Sept.", "21. Sept."]);
  });

  it("keeps every label centred where there is room", () => {
    const placed = placeTimeLabels(
      ticks(["Sep 15", "Sep 17", "Sep 19"], 120).map((t) => ({ ...t, at: t.at + 60 })),
      400,
    );
    expect(placed.map((p) => p.anchor)).toEqual(["middle", "middle", "middle"]);
  });

  it("never lets two labels touch, in any language", () => {
    for (const labels of [
      ["9月15日", "9月17日", "9月19日", "9月21日"],
      ["15. Sept.", "17. Sept.", "19. Sept.", "21. Sept."],
    ]) {
      const placed = placeTimeLabels(ticks(labels, 40), 160);
      for (let i = 1; i < placed.length; i++) expect(placed[i]!.x).toBeGreaterThan(placed[i - 1]!.x);
      expect(placed.length).toBeLessThan(labels.length);
    }
  });
});
