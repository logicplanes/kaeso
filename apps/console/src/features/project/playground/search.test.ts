import { describe, expect, it } from "vitest";
import { parametersOf, parseModels, searchOf, validatePlaygroundSearch } from "./search";

describe("validatePlaygroundSearch", () => {
  it("keeps models and parameters that are valid", () => {
    expect(
      validatePlaygroundSearch({
        models: "anthropic/claude-sonnet-4-5,openai/gpt-5.4-mini",
        system: "Be brief.",
        temperature: 0.7,
        top_p: "0.9",
        max_tokens: 1024,
        effort: "high",
        summary: "concise",
      }),
    ).toEqual({
      models: "anthropic/claude-sonnet-4-5,openai/gpt-5.4-mini",
      system: "Be brief.",
      temperature: 0.7,
      top_p: 0.9,
      max_tokens: 1024,
      effort: "high",
      summary: "concise",
    });
  });

  it("drops what is out of range or unknown, never failing", () => {
    expect(
      validatePlaygroundSearch({ temperature: 3, top_p: -1, max_tokens: 1.5, effort: "turbo", summary: "auto", system: "  ", models: "not a model", extra: 1 }),
    ).toEqual({});
    expect(validatePlaygroundSearch({ system: "x".repeat(8001) })).toEqual({});
  });

  it("reads a number the router parsed as the system instructions", () => {
    expect(validatePlaygroundSearch({ system: 42 })).toEqual({ system: "42" });
  });
});

describe("parseModels", () => {
  it("gives at most four columns, an invalid or empty entry as a column without a model", () => {
    expect(parseModels("openai/gpt-5,,bad model,anthropic/claude-opus-5-5,openai/gpt-6")).toEqual(["openai/gpt-5", "", "", "anthropic/claude-opus-5-5"]);
    expect(parseModels(undefined)).toEqual([]);
    expect(parseModels("openai/ft:gpt-4o-mini:org:custom:abc")).toEqual(["openai/ft:gpt-4o-mini:org:custom:abc"]);
  });
});

describe("parametersOf and searchOf", () => {
  it("go between the address and the parameter panel, dropping what is out of range", () => {
    const search = { system: "Be brief.", temperature: 0.7, top_p: 0.9, max_tokens: 512, effort: "high" as const, summary: "off" as const };
    const p = parametersOf(search);
    expect(p).toEqual({ instructions: "Be brief.", temperature: 0.7, topP: 0.9, maxOutputTokens: 512, reasoningEffort: "high", reasoningSummary: "off" });
    expect(searchOf(p)).toEqual(search);
    expect(searchOf({ temperature: 5, reasoningSummary: "auto", instructions: "" })).toEqual({});
  });
});
