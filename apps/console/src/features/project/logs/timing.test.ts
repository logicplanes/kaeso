import { describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { seconds, startedLabel } from "./timing";

describe("seconds", () => {
  it("reads as the timeline does", () => {
    expect(seconds(445)).toBe("445 ms");
    expect(seconds(1054)).toBe("1.1 s");
    expect(seconds(15_532)).toBe("15.5 s");
    expect(seconds(65_000)).toBe("1 min 5 s");
  });
});

describe("startedLabel", () => {
  const now = new Date(2026, 9, 14, 12, 0, 0);
  it("shows the time for a request from today", () => {
    expect(startedLabel(new Date(2026, 9, 14, 11, 3, 11).toISOString(), now)).toBe("Started 11:03:11");
  });
  it("shows the day for an older one", () => {
    expect(startedLabel(new Date(2026, 9, 13, 19, 49, 25).toISOString(), now)).toBe("Started 13 Oct");
  });
  it("writes the day the German way in German", async () => {
    apply(await load("de"));
    expect(startedLabel(new Date(2026, 9, 13, 19, 49, 25).toISOString(), now)).toBe("Beginn 13. Okt.");
  });
});
