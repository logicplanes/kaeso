import { afterEach, describe, expect, it } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { dayKey, inZone, yearOf } from "./zone";

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
});
