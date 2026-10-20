import { afterEach, describe, expect, it } from "vitest";
import {
  formatBytes,
  formatContext,
  formatDate,
  formatDuration,
  formatLocale,
  formatPrice,
  formatRelative,
  formatTimeZone,
  formatTokens,
  formatUSD,
  setFormatLocale,
  setFormatTimeZone,
} from "./index.js";

afterEach(() => setFormatLocale("en"));

describe("formatBytes", () => {
  it("climbs the units and keeps one decimal below 100", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(250 * 1024 * 1024)).toBe("250 MB");
    expect(formatBytes(2 ** 60)).toBe("1024 PB");
  });
});

describe("formatDuration", () => {
  it("reads from milliseconds to hours", () => {
    expect(formatDuration(240)).toBe("240 ms");
    expect(formatDuration(1500)).toBe("1.5 s");
    expect(formatDuration(42_000)).toBe("42 s");
    expect(formatDuration(120_000)).toBe("2 min");
    expect(formatDuration(150_000)).toBe("2 min 30 s");
    expect(formatDuration(3 * 3_600_000 + 5 * 60_000)).toBe("3 h 5 min");
  });
  it("rounds before it splits, so no unit ever reads its own maximum", () => {
    expect(formatDuration(999.6)).toBe("1 s");
    expect(formatDuration(9_960)).toBe("10 s");
    expect(formatDuration(59_600)).toBe("1 min");
    expect(formatDuration(119_600)).toBe("2 min");
    expect(formatDuration(119_400)).toBe("1 min 59 s");
    expect(formatDuration(3_599_999)).toBe("1 h 0 min");
    expect(formatDuration(7_199_999)).toBe("2 h 0 min");
    expect(formatDuration(5_430_000)).toBe("1 h 31 min");
  });
  it("keeps a fixed number of places when asked to be precise", () => {
    expect(formatDuration(999.4, { precise: true })).toBe("999 ms");
    expect(formatDuration(1000, { precise: true })).toBe("1.00 s");
    expect(formatDuration(9_996, { precise: true })).toBe("10.0 s");
    expect(formatDuration(12_345, { precise: true })).toBe("12.3 s");
    expect(formatDuration(59_960, { precise: true })).toBe("1 min");
  });
});

describe("formatRelative", () => {
  const now = new Date(2026, 9, 15, 12, 0, 0);
  it("covers the future, days and older dates", () => {
    expect(formatRelative(new Date(2026, 9, 15, 12, 10), now)).toBe("in 10 min");
    expect(formatRelative(new Date(2026, 9, 15, 15, 0), now)).toBe("in 3 h");
    expect(formatRelative(new Date(2026, 9, 15, 9, 0), now)).toBe("3 h ago");
    expect(formatRelative(new Date(2026, 9, 17, 12, 0), now)).toBe("in 2 days");
    expect(formatRelative(new Date(2026, 9, 12, 12, 0), now)).toBe("3 days ago");
    expect(formatRelative(new Date(2026, 8, 1), now)).toBe("1 Sep");
    expect(formatRelative(new Date(2025, 8, 1), now)).toBe("1 Sep 2025");
  });
});

describe("in other languages", () => {
  it("writes dollars with the language's separators and symbol position", () => {
    setFormatLocale("de");
    expect(formatLocale()).toBe("de");
    expect(formatUSD("1234.5")).toBe("1.234,50\u00a0$");
    expect(formatUSD("-0.5")).toBe("−0,50\u00a0$");
    setFormatLocale("zh-CN");
    expect(formatUSD("1234.5")).toBe("$1,234.50");
    setFormatLocale("en");
    expect(formatUSD("not money")).toBe("not money");
  });

  it("uses translated words and the language's dates", () => {
    setFormatLocale("de", { "format.minutes": "{min} Min.", "format.thousands": "{value}\u00a0Tsd." });
    expect(formatDuration(120_000)).toBe("2 Min.");
    expect(formatTokens(12_400)).toBe("12,4\u00a0Tsd.");
    expect(formatTokens(793_640)).toBe("793,6\u00a0Tsd.");
    expect(formatTokens(3_801)).toBe("3,8\u00a0Tsd.");
    expect(formatTokens(2_890_000)).toBe("2,89\u00a0Mio.");
    expect(formatContext(131_072)).toBe("128K");
    expect(formatContext(8_000)).toBe("8K");
    expect(formatPrice("0.075")).toBe("0,075\u00a0$");
    setFormatLocale("zh-CN", { "format.thousands": "{value}千" });
    expect(formatTokens(3_801)).toBe("3.8千");
    expect(formatTokens(793_640)).toBe("79.4万");
    setFormatLocale("de");
    expect(formatDate(Date.UTC(2026, 9, 15, 8, 5, 9), { time: true, seconds: true, utc: true })).toBe("15. Okt. 2026, 08:05:09 UTC");
  });
});

describe("formatUSD rounding", () => {
  it("rounds to fixed places, carrying into the next digit", () => {
    expect(formatUSD("0.995", { decimals: 2 })).toBe("$1.00");
    expect(formatUSD("1999.995")).toBe("$2,000.00");
    expect(formatUSD("-0.000", { decimals: 2 })).toBe("$0.00");
    expect(formatUSD("12", { decimals: 0 })).toBe("$12");
  });

  it("keeps significant digits that round up into a longer cent", () => {
    expect(formatUSD("0.009999")).toBe("$0.01");
    expect(formatUSD("0.0009996")).toBe("$0.00100");
  });
});

describe("formatTokens in every language", () => {
  it("keeps at least three significant digits, whatever unit the language counts in", () => {
    expect(formatTokens(101_700_000)).toBe("101.7M");
    expect(formatTokens(1_234_567)).toBe("1.23M");
    expect(formatTokens(18_420_000)).toBe("18.4M");
    setFormatLocale("de");
    expect(formatTokens(101_700_000)).toBe("101,7\u00a0Mio.");
    expect(formatTokens(1_234_567)).toBe("1,23\u00a0Mio.");
    setFormatLocale("zh-CN");
    expect(formatTokens(101_700_000)).toBe("1.02亿");
    expect(formatTokens(1_234_567)).toBe("123.5万");
    expect(formatTokens(18_420_000)).toBe("1842万");
    expect(formatTokens(15_000)).toBe("1.5万");
  });
});

const COMPACT_UNITS: Record<string, [string, number][]> = {
  en: [
    ["T", 1e12],
    ["B", 1e9],
    ["M", 1e6],
    ["K", 1e3],
  ],
  de: [
    ["Bio.", 1e12],
    ["Mrd.", 1e9],
    ["Mio.", 1e6],
  ],
  "zh-CN": [
    ["万亿", 1e12],
    ["亿", 1e8],
    ["万", 1e4],
  ],
};
function readCompact(locale: string, text: string): number {
  let rest = text.replace(/\u00a0/g, " ").trim();
  let unit = 1;
  for (const [suffix, value] of COMPACT_UNITS[locale]!) {
    if (!rest.endsWith(suffix)) continue;
    unit = value;
    rest = rest.slice(0, -suffix.length).trim();
    break;
  }
  return Number(locale === "de" ? rest.replace(/\./g, "").replace(",", ".") : rest.replace(/,/g, "")) * unit;
}

describe("compact token counts in every language", () => {
  it.each([
    ["en", "103M", "1.03B", "100.3M"],
    ["de", "103\u00a0Mio.", "1,03\u00a0Mrd.", "100,3\u00a0Mio."],
    ["zh-CN", "1.03亿", "10.3亿", "1亿"],
  ])("%s: writes 103 million as %s, to three significant digits", (locale, million, billion, close) => {
    setFormatLocale(locale);
    expect(formatTokens(103_000_000)).toBe(million);
    expect(formatTokens(1_034_000_000)).toBe(billion);
    expect(formatTokens(100_300_000)).toBe(close);
  });
  it.each(["en", "de", "zh-CN"])("%s: is within half a percent of the count, from thousands to trillions", (locale) => {
    setFormatLocale(locale);
    const misses: string[] = [];
    for (let exponent = 3; exponent <= 13; exponent++) {
      for (let mantissa = 1; mantissa < 10; mantissa += 0.0173) {
        const count = Math.round(mantissa * 10 ** exponent);
        const shown = formatTokens(count);
        if (Math.abs(readCompact(locale, shown) - count) / count > 0.005) misses.push(`${count} → ${shown}`);
      }
    }
    expect(misses).toEqual([]);
  });
});

describe("formatUSD below the smallest place shown", () => {
  it("says the amount is under the smallest place instead of showing zero", () => {
    expect(formatUSD("0.0000001", { decimals: 6 })).toBe("<\u00a0$0.000001");
    expect(formatUSD("-0.0000001", { decimals: 6 })).toBe(">\u00a0−$0.000001");
    expect(formatUSD("0.004", { decimals: 2 })).toBe("<\u00a0$0.01");
    expect(formatUSD("0.4", { decimals: 0 })).toBe("<\u00a0$1");
    expect(formatUSD("0", { decimals: 6 })).toBe("$0.000000");
    expect(formatUSD("0.0000006", { decimals: 6 })).toBe("$0.000001");
  });
  it("writes the smallest place the language's way", () => {
    setFormatLocale("de");
    expect(formatUSD("0.0000001", { decimals: 6 })).toBe("<\u00a00,000001\u00a0$");
    setFormatLocale("zh-CN");
    expect(formatUSD("0.0000001", { decimals: 6 })).toBe("<\u00a0$0.000001");
  });
});

describe("the format time zone", () => {
  const at = new Date("2026-12-31T23:30:00Z");
  afterEach(() => setFormatTimeZone(undefined));

  it("shows every moment in the chosen zone", () => {
    setFormatTimeZone("Asia/Tokyo");
    expect(formatTimeZone()).toBe("Asia/Tokyo");
    expect(formatDate(at, { time: true })).toBe("1 Jan 2027, 08:30");
    expect(formatRelative(at, new Date("2027-01-20T00:00:00Z"))).toBe("1 Jan");
    setFormatTimeZone("America/New_York");
    expect(formatDate(at, { time: true })).toBe("31 Dec 2026, 18:30");
    expect(formatRelative(at, new Date("2027-01-20T00:00:00Z"))).toBe("31 Dec 2026");
  });

  it("keeps UTC when asked and ignores an unknown zone", () => {
    setFormatTimeZone("Asia/Tokyo");
    expect(formatDate(at, { time: true, utc: true })).toBe("31 Dec 2026, 23:30 UTC");
    setFormatTimeZone("Mars/Olympus");
    expect(formatTimeZone()).toBeUndefined();
  });
});
