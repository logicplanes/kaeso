import { describe, expect, it } from "vitest";
import { outputOf } from "./tool-calls";

describe("what the model reads back from a tool", () => {
  it("takes the text of the result, the whole result without text, and the error's message", () => {
    expect(outputOf({ result: { content: [{ type: "text", text: "Ada" }, { type: "image" }, { type: "text" }], isError: false } })).toEqual({
      output: "Ada\n\n",
      failed: false,
    });
    expect(outputOf({ result: { structuredContent: { id: 1 }, isError: true } })).toEqual({
      output: '{"structuredContent":{"id":1},"isError":true}',
      failed: true,
    });
    expect(outputOf({ result: null })).toEqual({ output: "{}", failed: false });
    expect(outputOf({ error: { code: -32602, message: "No such tool." } })).toEqual({ output: "No such tool.", failed: true });
    expect(outputOf({ error: { code: -32000 } })).toEqual({ output: '{"code":-32000}', failed: true });
  });
});
