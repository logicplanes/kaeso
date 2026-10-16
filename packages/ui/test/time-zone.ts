import { afterAll, beforeAll, describe, expect, it } from "vitest";

export const TIME_ZONES = ["Europe/Berlin", "America/Los_Angeles"] as const;

export function describeInTimeZone(timeZone: string, suite: () => void) {
  describe(`in ${timeZone}`, () => {
    let previous: string | undefined;
    beforeAll(() => {
      previous = process.env.TZ;
      process.env.TZ = timeZone;
    });
    afterAll(() => {
      if (previous === undefined) delete process.env.TZ;
      else process.env.TZ = previous;
    });
    it("runs off UTC", () => {
      expect(Intl.DateTimeFormat().resolvedOptions().timeZone).toBe(timeZone);
      expect(new Date(2027, 0, 1).getTimezoneOffset()).not.toBe(0);
    });
    suite();
  });
}
