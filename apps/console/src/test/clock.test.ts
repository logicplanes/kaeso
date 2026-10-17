import { describe, expect, it, vi } from "vitest";
import { atFixtureTime } from "./clock";
import { NOW } from "./fixtures";

describe("atFixtureTime", () => {
  atFixtureTime();
  it("runs every test at the fixtures' time, so dates in the fixtures never fall into the past", () => {
    expect(new Date().toISOString()).toBe(new Date(NOW).toISOString());
    expect(Date.now()).toBe(Date.parse(NOW));
  });
  it("leaves the timers real", () => {
    expect(vi.isFakeTimers()).toBe(false);
  });
});
