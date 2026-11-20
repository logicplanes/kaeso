import { describe, expect, it } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load } from "~/i18n";
import { usage } from "~/test/fixtures/shell";
import { moneyIn } from "~/format/currency";
import { totals } from "./usage-panels";

const periodUSD = (amount: string | null) => moneyIn("USD").period(amount);

const twoDays = [usage({ key: "2026-10-13", requests: 40, errors: 0, cost: "0.5" }), usage()];

describe("usage sums", () => {
  it("shows a period's cost in cents, and tiny costs as under a cent", () => {
    expect(periodUSD(null)).toBe("$0.00");
    expect(periodUSD("0.000")).toBe("$0.00");
    expect(periodUSD("0.001")).toBe("<$0.01");
    expect(periodUSD("12.345")).toBe("$12.35");
  });

  it("says under a cent in a whole phrase in German and Chinese", async () => {
    apply(await load("de"));
    setFormatLocale("de");
    expect(periodUSD("0.001")).toBe("unter 0,01\u00a0$");
    apply(await load("zh-CN"));
    setFormatLocale("zh-CN");
    expect(periodUSD("0.001")).toBe("低于 $0.01");
    setFormatLocale("en");
  });

  it("totals requests, errors, tokens and cost in the organization's currency", () => {
    expect(totals(twoDays)).toEqual({ requests: 140, errors: 5, tokens: 3500, cost: "1.75", toolCalls: 0, unpriced: 0 });
    expect(totals([usage({ cost: null })]).cost).toBeNull();
    expect(totals([usage({ cost: "1", converted_cost: "0.9" }), usage({ cost: "0.75", converted_cost: "0.675" })]).cost).toBe("1.575");
  });
});
