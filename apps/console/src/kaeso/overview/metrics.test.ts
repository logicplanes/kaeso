import { describe, expect, it } from "vitest";
import { usageRow } from "~/test/fixtures/org";
import { forecast } from "./metrics";

describe("the month's spend forecast", () => {
  it("adds the month so far and projects the last seven days' pace, counting a day without a known cost as nothing", () => {
    const rows = [
      usageRow("2026-11-01", { cost: "7" }),
      usageRow("2026-11-02", { cost: "7" }),
      usageRow("2026-11-03"),
      usageRow("2026-11-04", { cost: "7" }),
      usageRow("2026-11-05", { cost: "7" }),
      usageRow("2026-11-06", { cost: "7" }),
      usageRow("2026-11-07", { cost: "7" }),
      usageRow("2026-11-08", { cost: "7" }),
      usageRow("2026-11-09", { cost: "1" }),
    ];
    expect(forecast(rows, "2026-10-27", "2026-11-09")).toEqual({ soFar: 50, projected: 49 + 6 * 22, month: "2026-11-01" });
  });

  it("has nothing to say when the range starts after the month began", () => {
    expect(forecast([usageRow("2026-11-09", { cost: "1" })], "2026-11-05", "2026-11-09")).toBeUndefined();
  });
});
