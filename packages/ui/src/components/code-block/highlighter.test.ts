import { describe, expect, it } from "vitest";
import { CODE_LANGS, highlight, type CodeLang } from "./highlighter";

describe("highlight", () => {
  it("leaves plain text alone and refuses an unknown language", async () => {
    expect(await highlight("hello", "text")).toBeNull();
    expect(await highlight("puts 1", "ruby" as CodeLang)).toBeNull();
  });

  it("loads every language it offers on demand", async () => {
    for (const lang of CODE_LANGS) {
      const html = await highlight("x", lang);
      expect(html).toContain("<pre");
    }
    expect(await highlight('{"a":1}', "json")).toContain("shiki");
  }, 20_000);
});
