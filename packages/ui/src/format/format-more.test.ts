import { afterEach, describe, expect, it } from "vitest";
import { formatBytes, formatDate, formatDuration, formatLocale, formatRelative, formatTokens, formatUSD, setFormatLocale } from "./index";

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
});

describe("formatRelative", () => {
  const now = new Date(2026, 9, 15, 12, 0, 0);
  it("covers the future, days and older dates", () => {
    expect(formatRelative(new Date(2026, 9, 15, 12, 10), now)).toBe("in 10 min");
    expect(formatRelative(new Date(2026, 9, 15, 15, 0), now)).toBe("in 3 h");
    expect(formatRelative(new Date(2026, 9, 15, 9, 0), now)).toBe("3 h ago");
    expect(formatRelative(new Date(2026, 9, 17, 12, 0), now)).toBe("in 2 days");
    expect(formatRelative(new Date(2026, 9, 12, 12, 0), now)).toBe("3 days ago");
    expect(formatRelative(new Date(2026, 8, 1), now)).toBe("1 Sept");
    expect(formatRelative(new Date(2025, 8, 1), now)).toBe("1 Sept 2025");
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
    setFormatLocale("de", { "format.minutes": "{min} Min." });
    expect(formatDuration(120_000)).toBe("2 Min.");
    expect(formatTokens(12_400)).toBe("12.400");
    expect(formatDate(Date.UTC(2026, 9, 15, 8, 5, 9), { time: true, seconds: true, utc: true })).toBe("15. Okt. 2026, 08:05:09 UTC");
  });
});

describe("formatUSD rounding", () => {
  it("rounds to fixed places, carrying into the next digit", () => {
    expect(formatUSD("0.995", { decimals: 2 })).toBe("$1.00");
    expect(formatUSD("1999.995")).toBe("$2,000.00");
    expect(formatUSD("-0.001", { decimals: 2 })).toBe("$0.00");
    expect(formatUSD("12", { decimals: 0 })).toBe("$12");
  });

  it("keeps significant digits that round up into a longer cent", () => {
    expect(formatUSD("0.009999")).toBe("$0.01");
    expect(formatUSD("0.0009996")).toBe("$0.00100");
  });
});
