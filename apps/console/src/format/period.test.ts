import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { expiryRequest } from "~/kaeso/expiry-picker";
import { dayLabel, daysBetween, fullDayLabel, lastDays, monthLabel, parseDay, periodSearch, previousPeriod, utcDay } from "./period";
import { pickedDay, pickerDate } from "./picked-day";

const ZONES = [
  { tz: "Europe/Berlin", offset: -60 },
  { tz: "Pacific/Kiritimati", offset: -840 },
  { tz: "America/Los_Angeles", offset: 480 },
  { tz: "Pacific/Pago_Pago", offset: 660 },
];
const DAYS = ["2027-01-01", "2026-10-13", "2026-03-29", "2026-11-01", "2028-02-29"];

const original = process.env.TZ;
afterAll(() => {
  if (original === undefined) delete process.env.TZ;
  else process.env.TZ = original;
});

describe.each(ZONES)("in $tz", ({ tz, offset }) => {
  beforeAll(() => {
    process.env.TZ = tz;
  });

  it("runs in that time zone", () => {
    expect(new Date(2027, 0, 1).getTimezoneOffset()).toBe(offset);
  });

  it.each(DAYS)("a picked %s round-trips unchanged", (day) => {
    const date = pickerDate(day);
    expect([date.getFullYear(), date.getMonth() + 1, date.getDate(), date.getHours()]).toEqual([...day.split("-").map(Number), 0]);
    expect(pickedDay(date)).toBe(day);
  });

  it("asks the API for the picked day, where the UTC day would be another", () => {
    const picked = new Date(2027, 0, 1);
    expect(pickedDay(picked)).toBe("2027-01-01");
    expect(utcDay(picked)).toBe(offset < 0 ? "2026-12-31" : "2027-01-01");
    expect(expiryRequest("date", picked)).toEqual({ expires_on: "2027-01-01" });
  });

  it("shows the day in the address, where UTC midnight would show another west of UTC", () => {
    expect(pickerDate("2027-01-01").getDate()).toBe(1);
    expect(new Date("2027-01-01T00:00:00Z").getDate()).toBe(offset > 0 ? 31 : 1);
  });
});

describe("parseDay", () => {
  it("takes real days only", () => {
    expect(parseDay("2027-01-01")).toBe("2027-01-01");
    expect(parseDay("2028-02-29")).toBe("2028-02-29");
    expect(parseDay("2027-02-30")).toBeUndefined();
    expect(parseDay("2027-13-01")).toBeUndefined();
    expect(parseDay("1 Jan 2027")).toBeUndefined();
    expect(parseDay(20270101)).toBeUndefined();
  });
});

describe("days", () => {
  it("count back from the UTC day, today included", () => {
    expect(lastDays(7, new Date("2026-10-15T23:30:00Z"))).toEqual({ from: "2026-10-09", to: "2026-10-15" });
    expect(lastDays(1, new Date("2026-03-01T00:10:00Z"))).toEqual({ from: "2026-03-01", to: "2026-03-01" });
    expect(lastDays(30).to).toBe(utcDay(new Date()));
  });

  it("run from the first to the last day, across a month and a leap day", () => {
    expect(daysBetween("2028-02-27", "2028-03-01")).toEqual(["2028-02-27", "2028-02-28", "2028-02-29", "2028-03-01"]);
    expect(daysBetween("2026-10-15", "2026-10-15")).toEqual(["2026-10-15"]);
    expect(daysBetween("2026-10-16", "2026-10-15")).toEqual([]);
  });

  it("compare with the same number of days just before", () => {
    expect(previousPeriod("2026-10-09", "2026-10-15")).toEqual({ from: "2026-10-02", to: "2026-10-08", days: 7 });
    expect(previousPeriod("2026-03-01", "2026-03-31")).toEqual({ from: "2026-01-29", to: "2026-02-28", days: 31 });
    expect(previousPeriod("2027-01-01", "2027-01-01")).toEqual({ from: "2026-12-31", to: "2026-12-31", days: 1 });
  });
});

describe("day labels", () => {
  it("are written the English way, whatever the time zone", () => {
    expect(dayLabel("2027-01-01")).toBe("Jan 1");
    expect(monthLabel("2026-10-01")).toBe("Oct 2026");
    expect(fullDayLabel("2026-10-15")).toBe("15 Oct 2026");
  });

  it("are written the German and Chinese way", async () => {
    apply(await load("de"));
    expect(dayLabel("2026-01-05")).toBe("5. Jan.");
    expect(monthLabel("2026-01-05")).toBe("Jan. 2026");
    expect(fullDayLabel("2026-01-05")).toBe("5. Jan. 2026");
    apply(await load("zh-CN"));
    expect(dayLabel("2026-01-05")).toBe("1月5日");
    expect(monthLabel("2026-01-05")).toBe("2026年1月");
    expect(fullDayLabel("2026-01-05")).toBe("2026年1月5日");
  });
});

describe("the period in the address, when it makes no sense", () => {
  it("is dropped", () => {
    expect(periodSearch({ period: "3y" })).toEqual({});
    expect(periodSearch({ from: "2026-01-01" })).toEqual({});
    expect(periodSearch({ from: "2026-01-01", to: "someday" })).toEqual({});
    expect(periodSearch({ from: "2026-01-01", to: "2026-01-01" })).toEqual({ from: "2026-01-01", to: "2026-01-01" });
  });
});
