import { describe, expect, it } from "vitest";
import { splitWords } from "./words.js";

describe("splitWords", () => {
  it("joins a word's highlighted parts and keeps the spaces between words as they are", () => {
    expect(
      splitWords([
        { content: "KAESO_URL", className: "text-code-variable" },
        { content: "=https://kaeso.example.com  ", className: "" },
        { content: "kaeso", className: "text-code-function" },
        { content: "\tup", className: "" },
      ]),
    ).toEqual([
      [
        { content: "KAESO_URL", className: "text-code-variable" },
        { content: "=https://kaeso.example.com", className: "" },
      ],
      "  ",
      [{ content: "kaeso", className: "text-code-function" }],
      "\t",
      [{ content: "up", className: "" }],
    ]);
  });

  it("joins spaces that span parts and leaves an empty line empty", () => {
    expect(
      splitWords([
        { content: "a ", className: "x" },
        { content: " b", className: "y" },
      ]),
    ).toEqual([[{ content: "a", className: "x" }], "  ", [{ content: "b", className: "y" }]]);
    expect(splitWords([{ content: "", className: "" }])).toEqual([]);
  });
});
