import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { limitAmount, limitUse } from "./limit-use";

const of = ({ used, amount }: { used: string; amount: string }) => `${used} of ${amount}`;
const nbsp = (s: string) => s.replace(/[\u00a0\u202f]/g, " ");

describe("limitUse", () => {
  afterEach(() => setFormatLocale("en"));

  it("writes a large count short, as tokens are written elsewhere, and keeps the exact count", () => {
    expect(limitUse({ kind: "token_budget", used: "10624184", amount: "100000000" }, of)).toEqual({
      short: "10.6M of 100M",
      exact: "10,624,184 of 100,000,000",
    });
    expect(limitUse({ kind: "request_rate", used: null, amount: "3000" }, of)).toEqual({ short: "0 of 3K", exact: "0 of 3,000" });
  });

  it("leaves a small count as it is", () => {
    expect(limitUse({ kind: "tool_call_rate", used: "12", amount: "30" }, of)).toEqual({ short: "12 of 30", exact: "12 of 30" });
  });

  it("keeps money exact", () => {
    const money = limitUse({ kind: "spend_budget", used: "123456789.12", amount: "1000000000" }, of);
    expect(money).toEqual({ short: "$123,456,789.12 of $1,000,000,000.00", exact: "$123,456,789.12 of $1,000,000,000.00" });
    expect(limitUse({ kind: "spend_budget", used: "0", amount: "5" }, of, { decimals: 2 }).short).toBe("$0.00 of $5.00");
  });

  it("writes the short form as German and Chinese count", () => {
    setFormatLocale("de");
    const de = limitUse({ kind: "token_budget", used: "10624184", amount: "100000000" }, of);
    expect(nbsp(de.short)).toBe("10,6 Mio. of 100 Mio.");
    expect(de.exact).toBe("10.624.184 of 100.000.000");
    setFormatLocale("zh-CN");
    expect(limitUse({ kind: "token_budget", used: "10624184", amount: "100000000" }, of).short).toBe("1062.4万 of 1亿");
  });
});

describe("limitAmount", () => {
  it("rounds a forecast count to whole units before shortening it", () => {
    expect(limitAmount("token_budget", "805188994.140000")).toEqual({ short: "805.2M", exact: "805,188,994" });
    expect(limitAmount("token_budget", "412.5")).toEqual({ short: "413", exact: "413" });
  });
});
