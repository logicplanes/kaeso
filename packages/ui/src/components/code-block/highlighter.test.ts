import { describe, expect, it } from "vitest";
import { CODE_LANGS, highlight, type CodeLang } from "./highlighter.js";

describe("highlight", () => {
  it("leaves plain text alone and refuses an unknown language", async () => {
    expect(await highlight("hello", "text")).toBeNull();
    expect(await highlight("puts 1", "ruby" as CodeLang)).toBeNull();
  });

  it("loads every language it offers on demand", async () => {
    for (const lang of CODE_LANGS) {
      const lines = await highlight("x", lang);
      expect(
        lines
          ?.flat()
          .map((t) => t.content)
          .join(""),
      ).toBe("x");
    }
    expect(await highlight('{"a":1}', "json")).toEqual([
      [
        { content: "{", className: "text-fg-tertiary" },
        { content: '"', className: "text-fg-tertiary" },
        { content: "a", className: "" },
        { content: '"', className: "text-fg-tertiary" },
        { content: ":", className: "text-fg-tertiary" },
        { content: "1", className: "text-warning-text" },
        { content: "}", className: "text-fg-tertiary" },
      ],
    ]);
  }, 20_000);
});
