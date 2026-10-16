import { describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { deletionCaption } from "./org-switcher";

describe("deletion caption", () => {
  const now = new Date("2026-10-13T12:00:00");
  it("names the day without the year within this year", () => {
    expect(deletionCaption("2026-10-20T12:00:00", now)).toBe("Deletion on 20 Oct");
  });
  it("names the year beyond it", () => {
    expect(deletionCaption("2027-01-05T12:00:00", now)).toBe("Deletion on 5 Jan 2027");
  });
  it("writes the day the reader's way in another language", async () => {
    apply(await load("de"));
    expect(deletionCaption("2026-10-20T12:00:00", now)).toBe("Löschung am 20. Okt.");
  });
  it("still says what is happening without a day", () => {
    expect(deletionCaption(null, now)).toBe("Scheduled for deletion");
  });
});
