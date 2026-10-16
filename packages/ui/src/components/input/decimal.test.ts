import { describe, expect, it } from "vitest";
import { addDecimal, compareDecimal, decimalSeparators, groupDecimal, isDecimal, localizeDecimal, ungroup } from "./decimal";

describe("decimal strings", () => {
  it("adds exactly, at the larger scale, across signs", () => {
    expect(addDecimal("0.1", "0.2")).toBe("0.3");
    expect(addDecimal("1.25", "-3")).toBe("-1.75");
    expect(addDecimal("", "0.005")).toBe("0.005");
    expect(addDecimal("007", "")).toBe("7");
    expect(addDecimal("-0.5", "0.5")).toBe("0.0");
    expect(addDecimal("abc", "2")).toBe("2");
  });

  it("compares two amounts", () => {
    expect(compareDecimal("10", "9.99")).toBe(1);
    expect(compareDecimal("9.99", "10")).toBe(-1);
    expect(compareDecimal("1.50", "1.5")).toBe(0);
    expect(compareDecimal("-2", "-3")).toBe(1);
  });

  it("knows what a decimal looks like while it is typed", () => {
    expect(isDecimal("-")).toBe(true);
    expect(isDecimal("12.")).toBe(true);
    expect(isDecimal("1e3")).toBe(false);
  });

  it("writes and reads each language's separators", () => {
    const de = decimalSeparators("de");
    expect(de).toEqual({ group: ".", decimal: "," });
    expect(decimalSeparators("en")).toEqual({ group: ",", decimal: "." });
    expect(groupDecimal("-1234567.5", de)).toBe("-1.234.567,5");
    expect(groupDecimal("1234")).toBe("1,234");
    expect(groupDecimal("abc")).toBe("abc");
    expect(groupDecimal(" ")).toBe(" ");
    expect(localizeDecimal("12.5", de)).toBe("12,5");
    expect(localizeDecimal("12.5")).toBe("12.5");
    expect(ungroup("1234,5", de)).toBe("1234.5");
    expect(ungroup("1,234.5")).toBe("1234.5");
    expect(ungroup("1\u202f234,5", decimalSeparators("fr"))).toBe("1234.5");
  });
});
