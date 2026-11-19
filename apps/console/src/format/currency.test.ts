import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { currencyDigits, moneyIn, sumAmounts } from "./currency";
import { currencyName, currencySymbol, pricesIn } from "./prices";

const nbsp = (s: string) => s.replace(/[\u00a0\u202f]/g, " ");
const usd = moneyIn("USD");
const eur = moneyIn("EUR");

afterEach(() => setFormatLocale("en"));

describe("a request's cost in a column", () => {
  it("keeps six places so a column lines up", () => {
    expect(usd.request("0.00041925")).toBe("$0.000419");
    expect(usd.request("0.0000027")).toBe("$0.000003");
    expect(usd.request("1.25")).toBe("$1.250000");
    expect(eur.request("0.00041925")).toBe("€0.000419");
  });
  it("says nothing was spent as 0", () => {
    expect(usd.request("0")).toBe("$0");
    expect(usd.request("0.00000000")).toBe("$0");
    expect(eur.request("0")).toBe("€0");
  });
  it("says a cost too small for six places is below a millionth, not 0.000000", () => {
    expect(usd.request("0.0000004")).toBe("<\u00a0$0.000001");
    expect(eur.request("0.0000004")).toBe("<\u00a0€0.000001");
  });
});

describe("a request's cost on its own", () => {
  it("shows three significant figures, cents at least and six places at most", () => {
    expect(usd.cost("0.0273")).toBe("$0.0273");
    expect(usd.cost("0.003892")).toBe("$0.00389");
    expect(usd.cost("0.0000616")).toBe("$0.000062");
    expect(usd.cost("1.25")).toBe("$1.25");
    expect(usd.cost("12.3456")).toBe("$12.35");
    expect(usd.cost("2")).toBe("$2.00");
    expect(usd.cost("-0.0273")).toBe("−$0.0273");
    expect(eur.cost("0.003892")).toBe("€0.00389");
    expect(eur.cost("-0.0273")).toBe("−€0.0273");
  });
  it("keeps cents on an amount too large for plain decimals", () => {
    expect(usd.cost("1000000000000000000000")).toBe("$1,000,000,000,000,000,000,000.00");
    expect(eur.cost("1000000000000000000000")).toBe("€1,000,000,000,000,000,000,000.00");
  });
  it("says nothing was spent as 0, and a sliver below the sixth place as such", () => {
    expect(usd.cost("0")).toBe("$0");
    expect(usd.cost("0.00000004")).toBe("<\u00a0$0.000001");
  });
});

describe("a period's spend", () => {
  it("shows cents", () => {
    expect(usd.period("0.08185245")).toBe("$0.08");
    expect(usd.period("1234.5")).toBe("$1,234.50");
    expect(eur.period("1234.5")).toBe("€1,234.50");
  });
  it("marks less than a cent", () => {
    expect(usd.period("0.00078")).toBe("<$0.01");
    expect(usd.period("0.0000027")).toBe("<$0.01");
    expect(eur.period("0.001")).toBe("<€0.01");
  });
  it("rounds half a cent up to a cent", () => {
    expect(usd.period("0.005")).toBe("$0.01");
  });
  it("shows nothing spent as 0.00", () => {
    expect(usd.period("0")).toBe("$0.00");
    expect(usd.period(null)).toBe("$0.00");
    expect(eur.period(undefined)).toBe("€0.00");
  });
  it("counts whole units of a currency without cents", () => {
    const yen = moneyIn("JPY");
    expect(yen.period("1234.5")).toBe("¥1,235");
    expect(yen.period("0.2")).toBe("<¥1");
    expect(yen.period("0")).toBe("¥0");
  });
});

describe("a tile's total", () => {
  it("keeps two significant digits below a cent", () => {
    expect(usd.tile("0.007")).toBe("$0.007");
    expect(usd.tile("0.00723")).toBe("$0.0072");
    expect(usd.tile("12.4")).toBe("$12.40");
    expect(usd.tile("0")).toBe("$0.00");
    expect(usd.tile("0.00999")).toBe("$0.01");
    expect(usd.tile("0.000000123")).toBe("$0.00000012");
    expect(eur.tile("0.00723")).toBe("€0.0072");
    expect(eur.tile("12.4")).toBe("€12.40");
    expect(moneyIn("JPY").tile("1234.5")).toBe("¥1,235");
  });
});

describe("in German and Chinese", () => {
  it("writes every amount in the language's own form, small and none included", () => {
    setFormatLocale("de");
    expect(usd.period("0.00078")).toBe("<0,01\u00a0$");
    expect(usd.period("0")).toBe("0,00\u00a0$");
    expect(usd.tile("0.00723")).toBe("0,0072\u00a0$");
    expect(usd.request("0")).toBe("0\u00a0$");
    expect(usd.cost("0.0270")).toBe("0,027\u00a0$");
    expect(usd.cost("0.00000004")).toBe("<\u00a00,000001\u00a0$");
    expect(nbsp(eur.period("1234.5"))).toBe("1.234,50 €");
    expect(nbsp(eur.period("0.001"))).toBe("<0,01 €");
    expect(nbsp(eur.cost("0.0270"))).toBe("0,027 €");
    setFormatLocale("zh-CN");
    expect(eur.period("12.4")).toBe("€12.40");
    expect(usd.period("12.4")).toBe("$12.40");
    expect(moneyIn("CNY").period("1234.5")).toBe("¥1,234.50");
  });
});

describe("prices", () => {
  it("are dollars as they were set, in US dollars", () => {
    const prices = pricesIn(usd, undefined);
    expect(prices.price("0.075")).toBe("$0.075");
    expect(prices.price("4")).toBe("$4.00");
    expect(prices.unit("0.04")).toBe("$0.04");
    expect(prices.short("3")).toBe("$3");
    expect(prices.short("2.5")).toBe("$2.50");
    expect(prices.convert("2.5")).toBe("2.5");
  });
  it("turn into another currency at the latest rate", () => {
    const prices = pricesIn(eur, "0.9");
    expect(prices.convert("2.5")).toBe("2.25");
    expect(prices.price("2.5")).toBe("€2.25");
    expect(prices.price("0.075")).toBe("€0.0675");
    expect(prices.short("15")).toBe("€13.50");
    expect(prices.unit("0.04")).toBe("€0.036");
  });
  it("show no amount in a currency without a rate rather than a wrong one", () => {
    const prices = pricesIn(eur, undefined);
    expect(prices.convert("2.5")).toBeNull();
    expect(prices.price("2.5")).toBe("—");
  });
  it("keep one set per currency and rate", () => {
    expect(pricesIn(eur, "0.9")).toBe(pricesIn(eur, "0.9"));
    expect(moneyIn("EUR")).toBe(eur);
  });
});

describe("sums", () => {
  it("add amounts without floating point drift", () => {
    expect(sumAmounts(["0.1", "0.2", null, undefined])).toBe("0.3");
    expect(sumAmounts(["1", "2"])).toBe("3.00");
    expect(sumAmounts([])).toBe("0.00");
  });
});

describe("currency helpers", () => {
  it("know each currency's decimals, symbol and name", () => {
    expect([currencyDigits("EUR"), currencyDigits("JPY")]).toEqual([2, 0]);
    expect([currencySymbol("EUR"), currencySymbol("USD"), currencySymbol("CHF")]).toEqual(["€", "$", "CHF"]);
    expect(currencyName("EUR")).toBe("Euro");
    setFormatLocale("de");
    expect(currencyName("JPY")).toBe("Japanischer Yen");
  });

  it("name an unknown code by the code", () => {
    expect(currencyName("not a code")).toBe("not a code");
    expect(currencyName("XXQ")).toBe("XXQ");
  });
});
