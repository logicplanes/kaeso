import { afterEach, describe, expect, it } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { dayEnd, dayKey, dayStart, inZone, yearOf } from "./zone";

afterEach(() => setFormatTimeZone(undefined));

describe("the instance time zone", () => {
  it("decides the day and the year of a moment", () => {
    const at = new Date("2026-12-31T23:30:00Z");
    setFormatTimeZone("Asia/Tokyo");
    expect(inZone()).toEqual({ timeZone: "Asia/Tokyo" });
    expect(dayKey(at)).toBe("2027-01-01");
    expect(yearOf(at)).toBe("2027");
    setFormatTimeZone("America/New_York");
    expect(dayKey(at)).toBe("2026-12-31");
  });

  it("adds nothing without a zone", () => {
    expect(inZone()).toEqual({});
  });

  it("starts and ends a day at midnight in the zone, across a change of offset", () => {
    setFormatTimeZone("Asia/Tokyo");
    expect([dayStart("2026-10-14"), dayEnd("2026-10-14")]).toEqual(["2026-10-13T15:00:00.000Z", "2026-10-14T15:00:00.000Z"]);
    setFormatTimeZone("America/New_York");
    expect([dayStart("2026-11-01"), dayEnd("2026-11-01")]).toEqual(["2026-11-01T04:00:00.000Z", "2026-11-02T05:00:00.000Z"]);
    setFormatTimeZone("Europe/Berlin");
    expect([dayStart("2026-03-29"), dayEnd("2026-03-29")]).toEqual(["2026-03-28T23:00:00.000Z", "2026-03-29T22:00:00.000Z"]);
    setFormatTimeZone("UTC");
    expect([dayStart("2026-12-31"), dayEnd("2026-12-31")]).toEqual(["2026-12-31T00:00:00.000Z", "2027-01-01T00:00:00.000Z"]);
  });

  it("starts a day at the viewer's midnight without a zone", () => {
    expect(dayStart("2026-10-14")).toBe(new Date(2026, 9, 14).toISOString());
    expect(dayEnd("2026-10-14")).toBe(new Date(2026, 9, 15).toISOString());
  });
});
