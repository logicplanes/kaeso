import { describe, expect, it, vi } from "vitest";

const engine = vi.hoisted(() => ({ fail: true }));
const grammar = vi.hoisted(() => ({ fail: true }));
vi.mock("shiki/dist/langs/lua.mjs", async (importOriginal) => {
  const real = await importOriginal<{ default: unknown }>();
  return {
    get default() {
      if (grammar.fail) throw new Error("chunk failed to load");
      return real.default;
    },
  };
});
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

const { highlight } = await import("./highlighter.js");
const { getHighlighter } = await import("./shiki.js");

describe("getHighlighter", () => {
  it("tries again after a failed load instead of keeping the failure", async () => {
    await expect(getHighlighter()).rejects.toThrow("offline");
    engine.fail = false;
    await expect(getHighlighter()).resolves.toHaveProperty("codeToTokens");
  }, 20_000);
});

describe("highlight", () => {
  it("loads a language again after its chunk failed to load", async () => {
    engine.fail = false;
    await expect(highlight("local M = {}", "lua")).rejects.toThrow("chunk failed to load");
    grammar.fail = false;
    expect((await highlight("local M = {}", "lua"))?.[0]?.[0]).toEqual({ content: "local", className: "text-code-keyword" });
  }, 20_000);
});
