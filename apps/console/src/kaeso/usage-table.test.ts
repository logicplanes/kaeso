import { describe, expect, it } from "vitest";
import { usageRow } from "~/test/fixtures/project";
import { moneyIn } from "~/format/currency";
import { costColumns, sortUsage, sortsUsage, usageCsv } from "./usage-table";

const rows = [
  usageRow({ key: "a", requests: 1, cache_hit_rate: 0.5, cost: "3" }),
  usageRow({ key: "b", requests: 3, cache_hit_rate: null, cost: null }),
  usageRow({ key: "c", requests: 2, cache_hit_rate: 0.1, cost: "0.5" }),
];
const keys = (list: { key: string }[]) => list.map((r) => r.key);

describe("usage tables", () => {
  it("sorts by the numeric columns either way, what isn't known last", () => {
    expect(keys(sortUsage(rows, []))).toEqual(["a", "b", "c"]);
    expect(keys(sortUsage(rows, [{ id: "requests", desc: true }]))).toEqual(["b", "c", "a"]);
    expect(keys(sortUsage(rows, [{ id: "cache", desc: false }]))).toEqual(["c", "a", "b"]);
    expect(keys(sortUsage(rows, [{ id: "cache", desc: true }]))).toEqual(["a", "c", "b"]);
    expect(keys(sortUsage(rows, [{ id: "cost", desc: false }]))).toEqual(["c", "a", "b"]);
    expect(keys(sortUsage(rows, [{ id: "errors", desc: false }]))).toEqual(["a", "b", "c"]);
    expect(keys(sortUsage(rows, [{ id: "tokens", desc: true }]))).toEqual(["a", "b", "c"]);
    expect(keys(sortUsage(rows, [{ id: "key", desc: false }]))).toEqual(["a", "b", "c"]);
    expect([sortsUsage("cost"), sortsUsage("key"), sortsUsage("media")]).toEqual([true, false, false]);
  });

  it("writes CSV a spreadsheet reads as written", () => {
    const csv = usageCsv(
      [
        { header: "Name, full", value: (r) => (r.key === "a" ? 'Say "hi"' : r.key === "b" ? "-1 or @me" : "line\nbreak") },
        { header: "Requests", value: (r) => r.requests },
        { header: "Cost", value: (r) => r.cost },
      ],
      rows,
    );
    expect(csv).toBe('"Name, full",Requests,Cost\r\n"Say ""hi""",1,3\r\n\'-1 or @me,3,\r\n"line\nbreak",2,0.5\r\n');
  });

  it("names the cost by its currency, with US dollars beside it for accounting in another one", () => {
    const row = usageRow({ key: "a", cost: "1.1", converted_cost: "1", currency: "EUR" });
    expect(usageCsv(costColumns(moneyIn("EUR")), [row])).toBe("Cost (EUR),Cost (USD)\r\n1,1.1\r\n");
    expect(usageCsv(costColumns(moneyIn("USD")), [usageRow({ cost: "1.1" })])).toBe("Cost (USD)\r\n1.1\r\n");
  });
});
