import { describe, expect, it } from "vitest";
import {
  dateFormatter,
  formatContext,
  formatDate,
  formatDateRange,
  formatDuration,
  formatISODate,
  formatPercent,
  formatPrice,
  formatRelative,
  formatTokens,
  formatUSD,
  parseISODate,
} from "./index.js";
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
        expect(formatDateRange(new Date(2026, 11, 31), d)).toBe("31 Dec 2026 – 1 Jan 2027");
        expect(formatDateRange(d, new Date(2027, 0, 1, 18))).toBe("1 Jan");
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

describe("English dates", () => {
  it("write the day first with three-letter months everywhere", () => {
    const sep = new Date(Date.UTC(2026, 8, 21, 14, 5));
    expect(dateFormatter({ day: "numeric", month: "short", timeZone: "UTC" })(sep)).toBe("21 Sep");
    expect(dateFormatter({ weekday: "short", day: "numeric", month: "short", timeZone: "UTC" })(sep)).toBe("Mon 21 Sep");
    expect(dateFormatter({ month: "short", year: "numeric", timeZone: "UTC" })(sep)).toBe("Sep 2026");
    expect(formatDate(sep, { utc: true, time: true })).toBe("21 Sep 2026, 14:05 UTC");
    expect(dateFormatter({ day: "numeric", month: "short", timeZone: "UTC" }, "de")(sep)).toBe("21. Sept.");
  });
});

describe("token, context and price formats", () => {
  it("name context sizes as models do", () => {
    expect(formatContext(131_072)).toBe("128K");
    expect(formatContext(1_048_576)).toBe("1M");
    expect(formatContext(200_000)).toBe("200K");
    expect(formatContext(1_000_000)).toBe("1M");
    expect(formatContext(8_000)).toBe("8K");
    expect(formatContext(32_768)).toBe("32K");
    expect(formatContext(128_000)).toBe("128K");
    expect(formatContext(8_192)).toBe("8K");
    expect(formatContext(1_500)).toBe("1.5K");
    expect(formatContext(512)).toBe("512");
  });
  it("write prices exactly, with at least two decimals", () => {
    expect(formatPrice("0.075")).toBe("$0.075");
    expect(formatPrice("4")).toBe("$4.00");
    expect(formatPrice("20.000000")).toBe("$20.00");
    expect(formatPrice("1.25")).toBe("$1.25");
    expect(formatPrice("0.0000375")).toBe("$0.000038");
  });
  it("keep tokens short in English", () => {
    expect(formatTokens(793_640)).toBe("793.6K");
    expect(formatTokens(131_072)).toBe("131.1K");
  });
});

describe("formatDateRange across a year", () => {
  it("names both years, so twelve months never read as a day apart", () => {
    expect(formatDateRange(new Date(2025, 9, 14), new Date(2026, 9, 13))).toBe("14 Oct 2025 – 13 Oct 2026");
    expect(formatDateRange(new Date(2025, 9, 13), new Date(2026, 9, 13))).toBe("13 Oct 2025 – 13 Oct 2026");
    expect(formatDateRange(new Date(2026, 3, 14), new Date(2026, 9, 13))).toBe("14 Apr – 13 Oct");
  });
});
