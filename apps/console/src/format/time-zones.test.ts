import { afterEach, describe, expect, it } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { browserTimeZone, clockIn, offsetOf, timeZoneNames } from "./time-zones";

afterEach(() => setFormatLocale("en"));

describe("time zone words", () => {
  it("lists UTC first, then every zone the browser knows, and keeps the current one", () => {
    const names = timeZoneNames("Etc/GMT+5");
    expect(names[0]).toBe("UTC");
    expect(names).toContain("Europe/Berlin");
    expect(names).toContain("Etc/GMT+5");
    expect(new Set(names).size).toBe(names.length);
  });

  it("names a zone's offset and the time there", () => {
    const summer = new Date("2026-07-01T12:00:00Z");
    expect(offsetOf("Europe/Berlin", summer)).toBe("GMT+2");
    expect(offsetOf("UTC", summer)).toBe("GMT+0");
    expect(offsetOf("Mars/Olympus", summer)).toBe("Mars/Olympus");
    expect(clockIn("Asia/Tokyo", summer)).toBe("Wed 1 Jul, 21:00");
    expect(clockIn("Mars/Olympus", summer)).toBe("");
    expect(browserTimeZone()).toBeTruthy();
  });

  it("writes the time there in the console's language", () => {
    setFormatLocale("de");
    expect(clockIn("Asia/Tokyo", new Date("2026-07-01T12:00:00Z"))).toBe("Mi., 1. Juli, 21:00");
  });
});
