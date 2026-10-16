import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { axisLabelIndexes } from "@logicplanes/kaeso-ui";
import { apply, load } from "~/i18n";
import { bucketAxisLabel, bucketLabel, bucketsBetween, granularity, mondayOf } from "./buckets";
import { periodSearch } from "./period";
import { comparedWith, presetDays, resolvePeriod } from "./presets";

describe("granularity", () => {
  it("charts a quarter by day, a year by week, longer by month", () => {
    expect(granularity("2026-10-14", "2026-10-14")).toBe("day");
    expect(granularity("2026-07-17", "2026-10-14")).toBe("day");
    expect(granularity("2026-04-15", "2026-10-14")).toBe("week");
    expect(granularity("2025-10-15", "2026-10-14")).toBe("week");
    expect(granularity("2024-01-01", "2026-10-14")).toBe("month");
  });
});

describe("buckets", () => {
  it("name weeks by their Monday and months by their first day, as the API does", () => {
    expect(mondayOf("2026-10-14")).toBe("2026-10-12");
    expect(mondayOf("2026-10-12")).toBe("2026-10-12");
    expect(mondayOf("2026-10-18")).toBe("2026-10-12");
    expect(bucketsBetween("2026-08-05", "2026-08-24", "week")).toEqual(["2026-08-03", "2026-08-10", "2026-08-17", "2026-08-24"]);
    expect(bucketsBetween("2025-11-20", "2026-02-02", "month")).toEqual(["2025-11-01", "2025-12-01", "2026-01-01", "2026-02-01"]);
    expect(bucketsBetween("2026-10-12", "2026-10-14", "day")).toEqual(["2026-10-12", "2026-10-13", "2026-10-14"]);
  });
  it("label a bucket by what it stands for", () => {
    expect(bucketLabel("2026-08-03", "week")).toBe("Week of 3 Aug");
    expect(bucketLabel("2026-08-01", "month")).toBe("Aug 2026");
    expect(bucketLabel("2026-09-14", "week")).toBe("Week of 14 Sep");
    expect(bucketLabel("2026-09-01", "month")).toBe("Sep 2026");
    expect(bucketLabel("2026-10-12", "day")).toBe("Oct 12");
    expect(bucketAxisLabel("2026-08-03", "week")).toBe("3 Aug");
    expect(bucketAxisLabel("2026-08-01", "month")).toBe("Aug");
    expect(bucketAxisLabel("2027-01-01", "month")).toBe("2027");
  });
});

const ZONES = [
  { tz: "Europe/Berlin", offset: -60 },
  { tz: "America/Los_Angeles", offset: 480 },
  { tz: "Pacific/Kiritimati", offset: -840 },
];
const originalTz = process.env.TZ;

describe.each(ZONES)("a new year's first bucket in $tz", ({ tz, offset }) => {
  beforeAll(() => {
    process.env.TZ = tz;
  });
  afterAll(() => {
    if (originalTz === undefined) delete process.env.TZ;
    else process.env.TZ = originalTz;
  });

  it("runs in that time zone", () => {
    expect(new Date(2026, 0, 1).getTimezoneOffset()).toBe(offset);
  });

  it("names the year of the week of its first Monday, on the axis and in the tooltip", () => {
    const weeks = bucketsBetween("2025-12-10", "2026-01-20", "week");
    expect(weeks.map((w) => bucketAxisLabel(w, "week"))).toEqual(["8 Dec", "15 Dec", "22 Dec", "29 Dec", "5 Jan 2026", "12 Jan", "19 Jan"]);
    expect(weeks.map((w) => bucketLabel(w, "week"))).toEqual([
      "Week of 8 Dec",
      "Week of 15 Dec",
      "Week of 22 Dec",
      "Week of 29 Dec",
      "Week of 5 Jan 2026",
      "Week of 12 Jan",
      "Week of 19 Jan",
    ]);
    expect(bucketAxisLabel("2027-01-04", "week")).toBe("4 Jan 2027");
    expect(bucketAxisLabel("2029-01-01", "week")).toBe("1 Jan 2029");
    expect(bucketAxisLabel("2029-01-08", "week")).toBe("8 Jan");
  });

  it("names the year of 1 January, and of January by month", () => {
    const days = bucketsBetween("2026-12-30", "2027-01-02", "day");
    expect(days.map((d) => bucketAxisLabel(d, "day"))).toEqual(["Dec 30", "Dec 31", "Jan 1, 2027", "Jan 2"]);
    expect(days.map((d) => bucketLabel(d, "day"))).toEqual(["Dec 30", "Dec 31", "Jan 1, 2027", "Jan 2"]);
    const months = bucketsBetween("2026-11-15", "2027-02-01", "month");
    expect(months.map((m) => bucketAxisLabel(m, "month"))).toEqual(["Nov", "Dec", "2027", "Feb"]);
    expect(months.map((m) => bucketLabel(m, "month"))).toEqual(["Nov 2026", "Dec 2026", "Jan 2027", "Feb 2027"]);
  });

  it("keeps that label on the bar chart's axis when the labels don't all fit", () => {
    const weeks = bucketsBetween("2025-10-15", "2026-10-14", "week");
    const labels = weeks.map((w) => bucketAxisLabel(w, "week"));
    const shown = axisLabelIndexes(labels, 12);
    expect(shown.length).toBeLessThan(labels.length);
    expect(shown.map((i) => labels[i])).toContain("5 Jan 2026");
  });
});

describe("presets", () => {
  const now = new Date("2026-10-14T10:00:00Z");
  it("end on the API's today and count back", () => {
    expect(presetDays("today", now)).toEqual({ from: "2026-10-14", to: "2026-10-14" });
    expect(presetDays("7d", now)).toEqual({ from: "2026-10-08", to: "2026-10-14" });
    expect(presetDays("90d", now)).toEqual({ from: "2026-07-17", to: "2026-10-14" });
    expect(presetDays("6m", now)).toEqual({ from: "2026-04-15", to: "2026-10-14" });
    expect(presetDays("12m", now)).toEqual({ from: "2025-10-15", to: "2026-10-14" });
  });
  it("keep the end of a short month", () => {
    expect(presetDays("6m", new Date("2026-08-31T12:00:00Z"))).toEqual({ from: "2026-03-01", to: "2026-08-31" });
    expect(presetDays("12m", new Date("2028-02-29T12:00:00Z"))).toEqual({ from: "2027-03-01", to: "2028-02-29" });
  });
  it("start All time on the first day with usage, or today without one", () => {
    expect(presetDays("all", now, "2026-08-15")).toEqual({ from: "2026-08-15", to: "2026-10-14" });
    expect(presetDays("all", now, null)).toEqual({ from: "2026-10-14", to: "2026-10-14" });
    expect(presetDays("all", now)).toEqual({ from: "2026-10-14", to: "2026-10-14" });
  });
  it("compare with the prior period, except All time", () => {
    expect(comparedWith("6m", 183)).toBe("vs. prior 6 months");
    expect(comparedWith("12m", 365)).toBe("vs. prior 12 months");
    expect(comparedWith("7d", 7)).toBe("vs. prior 7 days");
    expect(comparedWith(undefined, 1)).toBe("vs. prior 1 day");
    expect(comparedWith("all", 60)).toBeUndefined();
  });
});

describe("the period in the address", () => {
  it("keeps a known preset and drops days beside it", () => {
    expect(periodSearch({ period: "6m", from: "2026-01-01", to: "2026-02-01" })).toEqual({ period: "6m" });
    expect(periodSearch({ period: "all" })).toEqual({ period: "all" });
  });
  it("keeps custom days only when both are real and in order", () => {
    expect(periodSearch({ from: "2026-01-01", to: "2026-02-01" })).toEqual({ from: "2026-01-01", to: "2026-02-01" });
    expect(periodSearch({ from: "2026-02-01", to: "2026-01-01" })).toEqual({});
    expect(periodSearch({ from: "2026-01-01" })).toEqual({});
    expect(periodSearch({ period: "5y" })).toEqual({});
    expect(periodSearch({ from: "2027-02-30", to: "2027-03-01" })).toEqual({});
  });
  it("resolves to days: a preset, custom days, or Last 30 days", () => {
    const now = new Date("2026-10-14T10:00:00Z");
    expect(resolvePeriod({ period: "12m" }, null, now)).toEqual({ from: "2025-10-15", to: "2026-10-14", period: "12m" });
    expect(resolvePeriod({ from: "2026-01-01", to: "2026-02-01" }, null, now)).toEqual({ from: "2026-01-01", to: "2026-02-01", period: undefined });
    expect(resolvePeriod({}, null, now)).toEqual({ from: "2026-09-15", to: "2026-10-14", period: "30d" });
    expect(resolvePeriod({ period: "5y" } as never, null, now)).toEqual({ from: "2026-09-15", to: "2026-10-14", period: "30d" });
  });
});

describe("buckets in German and Chinese", () => {
  it("label days, weeks and months in the language's own form", async () => {
    apply(await load("de"));
    expect(bucketLabel("2026-08-03", "week")).toBe("Woche vom 3. Aug.");
    expect(bucketLabel("2026-01-05", "week")).toBe("Woche vom 5. Jan. 2026");
    expect(bucketLabel("2026-08-01", "month")).toBe("Aug. 2026");
    expect(bucketLabel("2026-10-12", "day")).toBe("12. Okt.");
    expect(bucketLabel("2027-01-01", "day")).toBe("1. Jan. 2027");
    expect(bucketAxisLabel("2026-08-03", "week")).toBe("3. Aug.");
    expect(bucketAxisLabel("2027-01-01", "month")).toBe("2027");
    apply(await load("zh-CN"));
    expect(bucketLabel("2026-08-03", "week")).toBe("8月3日 起的一周");
    expect(bucketLabel("2026-08-01", "month")).toBe("2026年8月");
    expect(bucketAxisLabel("2026-01-05", "week")).toBe("2026年1月5日");
    expect(bucketAxisLabel("2026-08-01", "month")).toBe("8月");
  });
});
