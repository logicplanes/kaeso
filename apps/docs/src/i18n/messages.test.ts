import { describe, expect, it } from "vitest";
import { LOCALES } from "./locales";
import { MESSAGES } from "./messages";

describe("the site's own text", () => {
  const english = MESSAGES.en;

  it.each(LOCALES)("%s has every text, none empty", (locale) => {
    const messages = MESSAGES[locale];
    expect(Object.keys(messages).sort()).toEqual(Object.keys(english).sort());
    for (const [key, value] of Object.entries(messages)) expect(value.trim(), key).not.toBe("");
  });

  it.each(["de", "zh-CN"] as const)("%s is translated, apart from names", (locale) => {
    const same = Object.entries(MESSAGES[locale]).filter(([key, value]) => value === english[key as keyof typeof english]);
    expect(same).toEqual([]);
  });

  it("writes Chinese with full-width punctuation", () => {
    for (const value of Object.values(MESSAGES["zh-CN"])) expect(value).not.toMatch(/[一-鿿][,.;:!?]/);
  });
});
