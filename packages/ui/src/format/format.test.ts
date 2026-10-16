import { describe, expect, it } from "vitest";
import { formatDate, formatDateRange, formatDuration, formatISODate, formatPercent, formatRelative, formatTokens, formatUSD, parseISODate } from "./index.js";
import { describeInTimeZone, TIME_ZONES } from "../../test/time-zone.js";

describe("formatUSD", () => {
  it("keeps exact decimal strings and rounds half up to cents", () => {
    expect(formatUSD("12.4")).toBe("$12.40");
    expect(formatUSD("1234.565")).toBe("$1,234.57");
    expect(formatUSD("0.995")).toBe("$1.00");
    expect(formatUSD("-3.5")).toBe("−$3.50");
  });
  it("keeps significant digits below one cent", () => {
    expect(formatUSD("0.00042")).toBe("$0.000420");
    expect(formatUSD("0.008218")).toBe("$0.00822");
    expect(formatUSD("0.0099996")).toBe("$0.01");
  });
  it("fixes the places when asked, so a column aligns", () => {
    expect(formatUSD("0.008218", { decimals: 4 })).toBe("$0.0082");
    expect(formatUSD("0.02", { decimals: 4 })).toBe("$0.0200");
    expect(formatUSD("1234.5", { decimals: 2 })).toBe("$1,234.50");
  });
  it("never goes through floats", () => {
    expect(formatUSD("9007199254740993.10")).toBe("$9,007,199,254,740,993.10");
  });
});

describe("other formatters", () => {
  it("formats tokens, durations and percentages", () => {
    expect(formatTokens(18_420_000)).toBe("18.4M");
    expect(formatDuration(840)).toBe("840 ms");
    expect(formatDuration(2400)).toBe("2.4 s");
    expect(formatPercent(0.4567, 0)).toBe("46%");
  });
  it("formats relative times against a given now", () => {
    const now = new Date("2026-10-12T14:20:00Z");
    expect(formatRelative(new Date("2026-10-12T14:19:50Z"), now)).toBe("just now");
    expect(formatRelative(new Date("2026-10-12T14:12:00Z"), now)).toBe("8 min ago");
    expect(formatRelative(new Date("2026-10-11T10:00:00Z"), now)).toBe("yesterday");
  });
});

describe("ISO dates", () => {
  for (const tz of TIME_ZONES) {
    describeInTimeZone(tz, () => {
      it("names the local day at any hour", () => {
        expect(formatISODate(new Date(2027, 0, 1))).toBe("2027-01-01");
        expect(formatISODate(new Date(2027, 0, 1, 0, 30))).toBe("2027-01-01");
        expect(formatISODate(new Date(2027, 0, 1, 23, 30))).toBe("2027-01-01");
        expect(formatISODate(new Date(2026, 9, 25, 2, 30))).toBe("2026-10-25");
      });
      it("agrees with formatDate and formatDateRange", () => {
        const d = new Date(2027, 0, 1);
        expect(formatDate(d)).toBe("1 Jan 2027");
        expect(formatDateRange(new Date(2026, 11, 31), d)).toBe("Dec 31, 2026 – Jan 1, 2027");
        expect(formatDateRange(d, new Date(2027, 0, 1, 18))).toBe("Jan 1");
        expect(formatISODate(d)).toBe("2027-01-01");
      });
      it("names the UTC day on request", () => {
        expect(formatISODate(new Date("2027-01-01T00:00:00Z"), { utc: true })).toBe("2027-01-01");
        expect(formatISODate(new Date("2026-12-31T23:30:00Z"), { utc: true })).toBe("2026-12-31");
      });
      it("reads a typed day as the start of that local day", () => {
        const d = parseISODate("2027-01-01");
        expect(d).not.toBeNull();
        expect([d?.getFullYear(), d?.getMonth(), d?.getDate(), d?.getHours(), d?.getMinutes()]).toEqual([2027, 0, 1, 0, 0]);
        expect(d?.getTime()).toBe(new Date(2027, 0, 1).getTime());
        expect(parseISODate(" 2026-03-29 ")?.getTime()).toBe(new Date(2026, 2, 29).getTime());
      });
      it("round-trips every day of a year, across both clock changes", () => {
        for (let day = new Date(2026, 0, 1); day.getFullYear() === 2026; day = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1)) {
          const iso = formatISODate(day);
          expect(parseISODate(iso)?.getTime()).toBe(day.getTime());
        }
      });
    });
  }
  it("refuses what isn't a real date", () => {
    for (const text of ["", "2027-1-1", "2027/01/01", "2027-02-29", "2026-02-30", "2026-13-01", "2026-00-10", "2026-04-31", "01-01-2027"])
      expect(parseISODate(text)).toBeNull();
    expect(parseISODate("2028-02-29")).not.toBeNull();
  });
  it("keeps a year before 100 as written", () => {
    expect(parseISODate("0099-05-01")?.getFullYear()).toBe(99);
    expect(formatISODate(parseISODate("0099-05-01") as Date)).toBe("0099-05-01");
  });
});

describe("formatDateRange across a year", () => {
  it("names both years, so twelve months never read as a day apart", () => {
    expect(formatDateRange(new Date(2025, 9, 14), new Date(2026, 9, 13))).toBe("Oct 14, 2025 – Oct 13, 2026");
    expect(formatDateRange(new Date(2025, 9, 13), new Date(2026, 9, 13))).toBe("Oct 13, 2025 – Oct 13, 2026");
    expect(formatDateRange(new Date(2026, 3, 14), new Date(2026, 9, 13))).toBe("Apr 14 – Oct 13");
  });
});
