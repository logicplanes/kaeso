import { describe, expect, it } from "vitest";
import type { PlaygroundModel } from "~/api/queries/playground";
import { replyCost } from "./cost";
import { requestFor } from "./request";
import { newResponse } from "./responses";

const model = (patch: Partial<PlaygroundModel> = {}): PlaygroundModel => ({
  id: "model_1",
  availability: "available",
  max_request_bytes: 33_554_432,
  mode: "text",
  voices: [],
  translations: false,
  durations: false,
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
  price_tiers: [],
  provider_key: true,
  needs_api_key: false,
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

  it("prices a prompt longer than a tier at that tier's prices, a missing one at the model's own", () => {
    const tiered = model({
      price_tiers: [
        { context_above: 1000, input_price: "1.5", output_price: "9" },
        { context_above: 100_000, input_price: "3" },
      ],
    });
    expect(replyCost(usage, tiered)).toBe("0.004215");
    expect(replyCost({ ...usage, input_tokens: 1000, cached_tokens: 0 }, tiered)).toBe("0.002100");
    expect(replyCost({ ...usage, input_tokens: 100_001, cached_tokens: 0, output_tokens: 0 }, tiered)).toBe("0.300003");
    const bare = { input_price: "1", output_price: "2", cache_read_price: null, cache_write_price: null };
    expect(replyCost(usage, bare)).toBe("0.001800");
    const cacheless = { ...bare, price_tiers: [{ context_above: 1000, output_price: "4" }] };
    expect(replyCost({ ...usage, cache_write_tokens: 100 }, cacheless)).toBe("0.002400");
    expect(replyCost(usage, { ...cacheless, input_price: null })).toBeNull();
    expect(replyCost(usage, { ...cacheless, output_price: null, price_tiers: [{ context_above: 1000 }] })).toBeNull();
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
