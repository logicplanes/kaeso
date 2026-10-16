import { describe, expect, it } from "vitest";
import { alertMarks } from "./alert-marks";

describe("alertMarks", () => {
  it("marks a budget's alert thresholds as fractions of the amount", () => {
    expect(alertMarks({ period: "month", alert_thresholds: [50, 80, 100] })).toEqual([0.5, 0.8, 1]);
  });

  it("marks nothing on a rate, which never sends alerts", () => {
    expect(alertMarks({ period: "minute", alert_thresholds: [50, 80, 100] })).toEqual([]);
  });
});
