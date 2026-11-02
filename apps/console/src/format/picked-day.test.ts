import { afterEach, describe, expect, it, vi } from "vitest";
import type { DateRangePreset } from "@logicplanes/kaeso-ui";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { afterToday, afterUtcToday, DAY_PRESETS, pickedDay, pickedPeriod, periodPresets as presetsOf, pickerDate, UTC_DAY_PRESETS } from "./picked-day";
import { presetDays } from "./presets";

const periodPresets = (first: string | null) => presetsOf((id) => presetDays(id, new Date(), first));

const preset = (label: string, presets: DateRangePreset[] = DAY_PRESETS) => {
  const r = presets.find((p) => p.label === label)?.range();
  return [r?.from && pickedDay(r.from), r?.to && pickedDay(r.to)];
};

describe("day presets", () => {
  afterEach(() => vi.useRealTimers());
  it("end on today in the install's time zone, whatever the viewer's clock says", () => {
    setFormatTimeZone("Pacific/Auckland");
    vi.useFakeTimers({ now: new Date("2026-10-14T13:30:00Z") });
    expect(preset("Today")).toEqual(["2026-10-15", "2026-10-15"]);
    expect(preset("Last 7 days")).toEqual(["2026-10-09", "2026-10-15"]);
    expect(preset("This month")).toEqual(["2026-10-01", "2026-10-15"]);
    expect(preset("Last month")).toEqual(["2026-09-01", "2026-09-30"]);
    expect(preset("Last 30 days")).toEqual(["2026-09-16", "2026-10-15"]);
    expect(preset("Last 90 days")).toEqual(["2026-07-18", "2026-10-15"]);
  });
  it("end on the UTC day for whole UTC days", () => {
    setFormatTimeZone("Pacific/Auckland");
    vi.useFakeTimers({ now: new Date("2026-10-14T13:30:00Z") });
    expect(preset("Today", UTC_DAY_PRESETS)).toEqual(["2026-10-14", "2026-10-14"]);
    expect(preset("Last 7 days", UTC_DAY_PRESETS)).toEqual(["2026-10-08", "2026-10-14"]);
    expect(preset("Last month", UTC_DAY_PRESETS)).toEqual(["2026-09-01", "2026-09-30"]);
  });
  it("are named in order", () => {
    for (const presets of [DAY_PRESETS, UTC_DAY_PRESETS])
      expect(presets.map((p) => p.label)).toEqual(["Today", "Last 7 days", "Last 30 days", "Last 90 days", "This month", "Last month"]);
  });
  it("disable the days after today, in the install's time zone or in UTC", () => {
    setFormatTimeZone("America/Los_Angeles");
    const now = new Date("2026-10-14T03:30:00Z");
    expect(afterToday(new Date(2026, 9, 13), now)).toBe(false);
    expect(afterToday(new Date(2026, 9, 14), now)).toBe(true);
    expect(afterUtcToday(new Date(2026, 9, 14), now)).toBe(false);
    expect(afterUtcToday(new Date(2026, 9, 15), now)).toBe(true);
  });
});

describe("picker date", () => {
  it("rolls a day the calendar does not have over into the next month", () => {
    expect(pickedDay(pickerDate("2026-02-30"))).toBe("2026-03-02");
    expect(pickedDay(pickerDate("2026-02-28"))).toBe("2026-02-28");
  });
});

describe("period presets", () => {
  afterEach(() => vi.useRealTimers());
  const ZONES = ["Europe/Berlin", "America/Los_Angeles", "Pacific/Kiritimati"];
  const original = process.env.TZ;
  afterEach(() => {
    if (original === undefined) delete process.env.TZ;
    else process.env.TZ = original;
  });
  it.each(ZONES)("are the API's days in %s", (tz) => {
    process.env.TZ = tz;
    vi.useFakeTimers({ now: new Date("2026-10-14T03:30:00Z") });
    const presets = periodPresets("2026-08-15");
    const days = (label: string) => {
      const r = presets.find((p) => p.label === label)?.range();
      return [r?.from && pickedDay(r.from), r?.to && pickedDay(r.to)];
    };
    expect(presets.map((p) => p.label)).toEqual(["Today", "Last 7 days", "Last 30 days", "Last 90 days", "Last 6 months", "Last 12 months", "All time"]);
    expect(days("Last 6 months")).toEqual(["2026-04-15", "2026-10-14"]);
    expect(days("All time")).toEqual(["2026-08-15", "2026-10-14"]);
    expect(pickedPeriod(presets, "12m")).toBe("12m");
    expect(pickedPeriod(presets, null)).toBeUndefined();
    const empty = periodPresets(null);
    expect(pickedPeriod(empty, "all")).toBe("all");
    expect(pickedPeriod(empty, "today")).toBe("today");
  });
});
