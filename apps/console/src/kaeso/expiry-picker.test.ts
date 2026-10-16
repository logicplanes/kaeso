import { describe, expect, it } from "vitest";
import { defaultExpiry, expiryRequest } from "./expiry-picker";

describe("key expiry", () => {
  it("asks for the chosen number of days, a day, or nothing", () => {
    expect(expiryRequest("30", null)).toEqual({ expires_in_days: 30 });
    expect(expiryRequest("date", new Date(2026, 10, 3))).toEqual({ expires_on: "2026-11-03" });
    expect(expiryRequest("date", null)).toEqual({});
    expect(expiryRequest("never", null)).toEqual({});
  });

  it("picks the longest default the limit allows", () => {
    expect(defaultExpiry(null)).toBe("90");
    expect(defaultExpiry(120)).toBe("90");
    expect(defaultExpiry(45)).toBe("30");
    expect(defaultExpiry(3)).toBe("1");
  });
});
