import { describe, expect, it, vi } from "vitest";

const engine = vi.hoisted(() => ({ fail: true }));
vi.mock("shiki/engine/javascript", async (importOriginal) => {
  const real = await importOriginal<typeof import("shiki/engine/javascript")>();
  return {
    ...real,
    createJavaScriptRegexEngine: (...args: Parameters<typeof real.createJavaScriptRegexEngine>) => {
      if (engine.fail) throw new Error("offline");
      return real.createJavaScriptRegexEngine(...args);
    },
  };
});

const { getHighlighter } = await import("./highlighter.js");

describe("getHighlighter", () => {
  it("tries again after a failed load instead of keeping the failure", async () => {
    await expect(getHighlighter()).rejects.toThrow("offline");
    engine.fail = false;
    await expect(getHighlighter()).resolves.toHaveProperty("codeToTokens");
  }, 20_000);
});
