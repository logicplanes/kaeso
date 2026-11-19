import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { approxMoney, currencyDigits, currencyName, currencySymbol, formatMoney } from "./currency";

const nbsp = (s: string) => s.replace(/[\u00a0\u202f]/g, " ");

describe("formatMoney", () => {
  afterEach(() => setFormatLocale("en"));

  it("writes the symbol, its place and the separators as English does", () => {
    expect(formatMoney("12.4", "EUR")).toBe("€12.40");
    expect(formatMoney("1234.5", "GBP")).toBe("£1,234.50");
    expect(formatMoney("1234.5", "JPY")).toBe("¥1,235");
  });

  it("writes them as German does", () => {
    setFormatLocale("de");
    expect(nbsp(formatMoney("12.4", "EUR"))).toBe("12,40 €");
    expect(nbsp(formatMoney("1234.5", "USD"))).toBe("1.234,50 $");
  });

  it("writes them as Chinese does", () => {
    setFormatLocale("zh-CN");
    expect(formatMoney("12.4", "EUR")).toBe("€12.40");
    expect(formatMoney("1234.5", "CNY")).toBe("¥1,234.50");
  });

  it("keeps up to four decimals below one unit, so a request's cost isn't 0", () => {
    expect(formatMoney("0.000419", "EUR")).toBe("€0.0004");
    expect(formatMoney("0.5", "JPY")).toBe("¥0.5");
    expect(formatMoney("0", "EUR")).toBe("€0.00");
  });
});

describe("approxMoney", () => {
  it("marks an amount in another currency as an estimate", () => {
    expect(approxMoney("12.4", "EUR")).toBe("≈ €12.40");
  });
});

describe("currency helpers", () => {
  afterEach(() => setFormatLocale("en"));

  it("knows each currency's decimals, symbol and name", () => {
    expect([currencyDigits("EUR"), currencyDigits("JPY")]).toEqual([2, 0]);
    expect([currencySymbol("EUR"), currencySymbol("USD"), currencySymbol("CHF")]).toEqual(["€", "$", "CHF"]);
    expect(currencyName("EUR")).toBe("Euro");
    setFormatLocale("de");
    expect(currencyName("JPY")).toBe("Japanischer Yen");
  });

  it("names an unknown code by the code", () => {
    expect(currencyName("not a code")).toBe("not a code");
    expect(currencyName("XXQ")).toBe("XXQ");
  });
});
