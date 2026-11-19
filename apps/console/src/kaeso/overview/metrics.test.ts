import { describe, expect, it } from "vitest";
import { usageRow } from "~/test/fixtures/org";
import { moneyIn } from "~/format/currency";
import { forecast, forecastFrom, formatWith } from "./metrics";

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

  it("takes the pace from the seven days before today in the month's first week too", () => {
    const every = (from: string, to: string) => {
      const rows = [];
      for (let day = from; day <= to; day = new Date(Date.parse(`${day}T00:00:00Z`) + 86_400_000).toISOString().slice(0, 10))
        rows.push(usageRow(day, { cost: "10" }));
      return rows;
    };
    for (const [today, soFar] of [
      ["2026-11-01", 10],
      ["2026-11-02", 20],
      ["2026-11-03", 30],
      ["2026-11-07", 70],
      ["2026-11-08", 80],
      ["2026-11-20", 200],
    ] as const) {
      const from = forecastFrom(today);
      expect(forecast(every(from, today), from, today)).toEqual({ soFar, projected: 300, month: "2026-11-01" });
    }
    expect(forecastFrom("2026-11-03")).toBe("2026-10-27");
    expect(forecastFrom("2026-11-08")).toBe("2026-11-01");
    expect(forecastFrom("2026-11-20")).toBe("2026-11-01");
  });
});

describe("a chart's formatter", () => {
  it("writes spend in the organization's currency and counts as numbers", () => {
    expect(formatWith("cost", moneyIn("EUR"))(12.4)).toBe("€12.40");
    expect(formatWith("requests", moneyIn("EUR"))(1200)).toBe("1,200");
  });
});
