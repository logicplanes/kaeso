import { describe, expect, it } from "vitest";
import type { PlaygroundModel } from "~/api/queries/playground";
import { replyCost } from "./cost";
import { requestFor } from "./request";
import { newResponse } from "./responses";

const model = (patch: Partial<PlaygroundModel> = {}): PlaygroundModel => ({
  id: "model_1",
  availability: "available",
  aliases: [],
  input_modalities: ["text"],
  model: "gpt-5.4-mini",
  provider: "openai",
  model_id: "gpt-5.4-mini",
  display_name: "GPT-5.4 mini",
  reasoning: false,
  efforts: [],
  temperature: true,
  input_price: "0.75",
  output_price: "4.5",
  cache_read_price: "0.075",
  cache_write_price: null,
  provider_key: true,
  ...patch,
});

describe("requestFor", () => {
  it("sends the prompt after the earlier turns, with the parameters the model takes", () => {
    const earlier = { id: "t1", prompt: "Hi", response: { ...newResponse(0), status: "completed" as const, text: "Hello." } };
    expect(
      requestFor(model(), [earlier], "Again", { instructions: "Be brief.", temperature: 0.2, topP: 0.9, maxOutputTokens: 256, reasoningEffort: "high" }),
    ).toEqual({
      model: "gpt-5.4-mini",
      input: [
        { role: "user", content: "Hi" },
        { role: "assistant", content: "Hello." },
        { role: "user", content: "Again" },
      ],
      instructions: "Be brief.",
      temperature: 0.2,
      top_p: 0.9,
      max_output_tokens: 256,
      stream: true,
    });
  });

  it("asks a reasoning model for a summary, and for an effort it lists", () => {
    const reasoner = model({ reasoning: true, efforts: ["low", "high"], temperature: false });
    expect(requestFor(reasoner, [], "Q", { temperature: 1, reasoningEffort: "high" })).toMatchObject({ reasoning: { effort: "high", summary: "auto" } });
    expect(requestFor(reasoner, [], "Q", { temperature: 1 })).not.toHaveProperty("temperature");
    expect(requestFor(reasoner, [], "Q", { reasoningEffort: "max", reasoningSummary: "detailed" }).reasoning).toEqual({ summary: "detailed" });
    expect(requestFor(reasoner, [], "Q", { reasoningSummary: "off" })).not.toHaveProperty("reasoning");
    expect(requestFor(model({ reasoning: true }), [], "Q", { reasoningEffort: "max" }).reasoning).toEqual({ effort: "max", summary: "auto" });
    expect(requestFor(reasoner, [], "Q", { reasoningEffort: "high", reasoningSummary: "off" }).reasoning).toEqual({ effort: "high" });
  });
});

describe("replyCost", () => {
  const usage = { input_tokens: 1200, output_tokens: 300, reasoning_tokens: 100, cached_tokens: 200, cache_write_tokens: 0 };

  it("prices the usage at the catalogue's prices, cached input at the cache-read price", () => {
    expect(replyCost(usage, model())).toBe("0.002115");
  });

  it("prefers the gateway's cost, and has none without prices", () => {
    expect(replyCost({ ...usage, cost: "0.0042" }, model())).toBe("0.0042");
    expect(replyCost(usage, model({ input_price: null }))).toBeNull();
    expect(replyCost(usage, null)).toBeNull();
  });

  it("reads prices without a whole or a fraction part, and prices cache traffic at the input price when it has none", () => {
    const traffic = { input_tokens: 1000, output_tokens: 2000, reasoning_tokens: 0, cached_tokens: 100, cache_write_tokens: 200, cost: "n/a" };
    expect(replyCost(traffic, model({ input_price: "3", output_price: ".5", cache_read_price: null, cache_write_price: "" }))).toBe("0.004000");
    expect(replyCost(traffic, model({ input_price: "3", output_price: "free" }))).toBeNull();
    expect(replyCost(traffic, model({ input_price: ".", output_price: "1" }))).toBeNull();
  });
});
