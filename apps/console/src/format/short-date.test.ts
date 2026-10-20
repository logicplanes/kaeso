import { afterEach, describe, expect, it } from "vitest";
import { setFormatLocale, setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import type { Locale } from "~/i18n";
import { dayWithinYear } from "./short-date";

const NOW = new Date("2026-10-19T12:00:00Z");

afterEach(() => setFormatLocale("en"));

describe("a day in this year", () => {
  it.each<[Locale, string, string]>([
    ["en", "22 Oct", "3 Jan 2027"],
    ["de", "22. Okt.", "3. Jan. 2027"],
    ["zh-CN", "10月22日", "2027年1月3日"],
  ])("leaves the year out in %s, and writes it for another year", (lang, thisYear, nextYear) => {
    setFormatLocale(lang);
    expect(dayWithinYear("2026-10-22T09:00:00Z", NOW)).toBe(thisYear);
    expect(dayWithinYear("2027-01-03T09:00:00Z", NOW)).toBe(nextYear);
  });

  it("decides the year in the account's time zone", () => {
    setFormatLocale("en");
    setFormatTimeZone("Pacific/Auckland");
    expect(dayWithinYear("2026-12-31T20:00:00Z", NOW)).toBe("1 Jan 2027");
  });
});
