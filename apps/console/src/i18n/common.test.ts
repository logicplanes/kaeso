import { afterEach, describe, expect, it } from "vitest";
import { apply, load } from "./index";
import { common } from "./common";
import english from "./locales/en/common.json";
import german from "./locales/de/common.json";

afterEach(async () => {
  apply(await load("en"));
});

describe("the common words", () => {
  it("read as the English source", () => {
    const source: Record<string, string> = english;
    for (const [name, word] of Object.entries(common)) expect(word(), name).toBe(source[`common.${name}`]);
  });

  it("read in German in a German session", async () => {
    apply(await load("de"));
    const catalog: Record<string, string> = german;
    for (const [name, word] of Object.entries(common)) expect(word(), name).toBe(catalog[`common.${name}`]);
    expect(common.cancel()).toBe("Abbrechen");
  });
});
