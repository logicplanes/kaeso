import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { periodUSD, tileUSD } from "~/kaeso/usage-panels";
import { costUSD, requestUSD } from "./money";

describe("requestUSD", () => {
  it("keeps six places so a column lines up", () => {
    expect(requestUSD("0.00041925")).toBe("$0.000419");
    expect(requestUSD("0.0000027")).toBe("$0.000003");
    expect(requestUSD("1.25")).toBe("$1.250000");
  });
  it("says nothing was spent as $0", () => {
    expect(requestUSD("0")).toBe("$0");
    expect(requestUSD("0.00000000")).toBe("$0");
  });
  it("says a cost too small for six places is below a millionth of a dollar, not $0.000000", () => {
    expect(requestUSD("0.0000004")).toBe("<\u00a0$0.000001");
  });
});

describe("costUSD", () => {
  it("shows three significant figures, cents at least and six places at most", () => {
    expect(costUSD("0.0273")).toBe("$0.0273");
    expect(costUSD("0.003892")).toBe("$0.00389");
    expect(costUSD("0.0000616")).toBe("$0.000062");
    expect(costUSD("1.25")).toBe("$1.25");
    expect(costUSD("12.3456")).toBe("$12.35");
    expect(costUSD("2")).toBe("$2.00");
    expect(costUSD("-0.0273")).toBe("\u2212$0.0273");
  });
  it("keeps cents on an amount too large for plain decimals", () => {
    expect(costUSD("1000000000000000000000")).toBe("$1,000,000,000,000,000,000,000.00");
  });
  it("says nothing was spent as $0, and a sliver below the sixth place as such", () => {
    expect(costUSD("0")).toBe("$0");
    expect(costUSD("0.00000004")).toBe("<\u00a0$0.000001");
  });
});

describe("periodUSD", () => {
  it("shows cents", () => {
    expect(periodUSD("0.08185245")).toBe("$0.08");
    expect(periodUSD("1234.5")).toBe("$1,234.50");
  });
  it("marks less than a cent", () => {
    expect(periodUSD("0.00078")).toBe("<$0.01");
    expect(periodUSD("0.0000027")).toBe("<$0.01");
  });
  it("rounds half a cent up to a cent", () => {
    expect(periodUSD("0.005")).toBe("$0.01");
  });
  it("shows nothing spent as $0.00", () => {
    expect(periodUSD("0")).toBe("$0.00");
    expect(periodUSD(null)).toBe("$0.00");
  });
});

describe("in German", () => {
  afterEach(() => setFormatLocale("en"));
  it("writes every amount in the language's own form, small and none included", () => {
    setFormatLocale("de");
    expect(periodUSD("0.00078")).toBe("<0,01\u00a0$");
    expect(periodUSD("0")).toBe("0,00\u00a0$");
    expect(tileUSD("0.00723")).toBe("0,0072\u00a0$");
    expect(requestUSD("0")).toBe("0\u00a0$");
    expect(costUSD("0.0270")).toBe("0,027\u00a0$");
    expect(costUSD("0.00000004")).toBe("<\u00a00,000001\u00a0$");
  });
});

describe("tileUSD", () => {
  it("keeps two significant digits below a cent", () => {
    expect(tileUSD("0.007")).toBe("$0.007");
    expect(tileUSD("0.00723")).toBe("$0.0072");
    expect(tileUSD("12.4")).toBe("$12.40");
  });
});
