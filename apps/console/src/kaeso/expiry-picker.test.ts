import { describe, expect, it } from "vitest";
import { expiryDayDisabled } from "./expiry-picker";

describe("the expiry picker's days", () => {
  const now = Date.parse("2026-11-15T04:00:00Z");

  it("offers no day whose start in UTC has passed, wherever the browser is", () => {
    expect(expiryDayDisabled("2026-11-14", now, null)).toBe(true);
    expect(expiryDayDisabled("2026-11-15", now, null)).toBe(true);
    expect(expiryDayDisabled("2026-11-16", now, null)).toBe(false);
  });

  it("measures the policy's maximum to the day's start in UTC", () => {
    const latest = now + 10 * 86_400_000;
    expect(expiryDayDisabled("2026-11-25", now, latest)).toBe(false);
    expect(expiryDayDisabled("2026-11-26", now, latest)).toBe(true);
  });
});
