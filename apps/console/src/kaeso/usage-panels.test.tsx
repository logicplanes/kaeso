import { describe, expect, it } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load } from "~/i18n";
import { usage } from "~/test/fixtures/shell";
import { periodUSD, sumUSD, tileUSD, totals } from "./usage-panels";

const twoDays = [usage({ key: "2026-10-13", requests: 40, errors: 0, cost: "0.5" }), usage()];

describe("usage sums", () => {
  it("adds dollar amounts without floating point drift", () => {
    expect(sumUSD(["0.1", "0.2", null, undefined])).toBe("0.3");
    expect(sumUSD(["1", "2"])).toBe("3.00");
    expect(sumUSD([])).toBe("0.00");
  });

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

  it("keeps two significant digits of a tile's sub-cent cost", () => {
    expect(tileUSD("0.004213")).toBe("$0.0042");
    expect(tileUSD("0")).toBe("$0.00");
    expect(tileUSD("3.5")).toBe("$3.50");
    expect(tileUSD("0.00999")).toBe("$0.01");
    expect(tileUSD("0.000099")).toBe("$0.000099");
    expect(tileUSD("0.0000001")).toBe("$0.0000001");
    expect(tileUSD("0.000000123")).toBe("$0.00000012");
  });

  it("totals requests, errors, tokens and cost", () => {
    expect(totals(twoDays)).toEqual({ requests: 140, errors: 5, tokens: 3500, cost: "1.75", toolCalls: 0 });
    expect(totals([usage({ cost: null })]).cost).toBeNull();
  });
});
