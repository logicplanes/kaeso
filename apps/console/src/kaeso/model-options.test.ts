import { describe, expect, it } from "vitest";
import type { CatalogueModelView } from "~/api/generated/types";
import type { PlaygroundModel } from "~/api/queries/playground";
import { catalogueModelOption, playgroundModelOption, recentModels, rememberModel } from "./model-options";

const entry = (effective: Partial<CatalogueModelView["effective"]>, more: Partial<CatalogueModelView> = {}) =>
  ({ id: "cm_01", provider: "openai", model_id: "gpt-5-mini", deprecated: false, effective, ...more }) as unknown as CatalogueModelView;

describe("model picker options", () => {
  it("carry a catalogue model's name, id, capabilities, context and price", () => {
    expect(
      catalogueModelOption(
        entry({ display_name: "GPT-5 mini", tool_call: true, reasoning: true, context_limit: 400_000, input_price: "0.25", output_price: "2" }),
      ),
    ).toEqual({
      value: "cm_01",
      provider: "openai",
      model: "gpt-5-mini",
      name: "GPT-5 mini",
      capabilities: ["tools", "reasoning"],
      context: 400_000,
      price: "$0.25 / $2",
    });
  });

  it("leave out a name that repeats the id, and mark deprecated models", () => {
    expect(catalogueModelOption(entry({ display_name: "gpt-5-mini" }, { deprecated: true }))).toEqual({
      value: "cm_01",
      provider: "openai",
      model: "gpt-5-mini",
      capabilities: [],
      deprecated: true,
    });
  });

  it("disable a Playground model without a provider key and say why", () => {
    const m = {
      provider: "anthropic",
      model_id: "claude-sonnet-4-5",
      display_name: "Claude Sonnet 4.5",
      reasoning: true,
      input_modalities: ["text", "image", "pdf"],
      context_limit: 200_000,
      input_price: "3",
      output_price: "15",
      provider_key: false,
    } as unknown as PlaygroundModel;
    expect(playgroundModelOption(m, "anthropic/claude-sonnet-4-5", "No Anthropic key")).toEqual({
      value: "anthropic/claude-sonnet-4-5",
      provider: "anthropic",
      model: "claude-sonnet-4-5",
      name: "Claude Sonnet 4.5",
      capabilities: ["reasoning", "vision", "pdf"],
      context: 200_000,
      price: "$3 / $15",
      disabled: true,
      note: "No Anthropic key",
    });
  });

  it("leave out what a Playground model doesn't list", () => {
    const m = {
      provider: "openai",
      model_id: "gpt-5-mini",
      display_name: "gpt-5-mini",
      reasoning: false,
      input_modalities: ["text"],
      context_limit: null,
      input_price: null,
      output_price: null,
      provider_key: true,
    } as unknown as PlaygroundModel;
    expect(playgroundModelOption(m, "gpt-5-mini", "No OpenAI key")).toEqual({ value: "gpt-5-mini", provider: "openai", model: "gpt-5-mini", capabilities: [] });
  });

  it("remember the five most recent models per project", () => {
    for (const v of ["a", "b", "c", "d", "e", "f", "b"]) rememberModel("proj_recent", v);
    expect(recentModels("proj_recent")).toEqual(["b", "f", "e", "d", "c"]);
    expect(recentModels("proj_other")).toEqual([]);
  });
});
