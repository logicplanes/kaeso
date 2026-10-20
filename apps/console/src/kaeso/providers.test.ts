import { describe, expect, it } from "vitest";
import { DIALECT_IDS } from "~/format/labels";
import { providers } from "~/test/fixtures";
import { credential } from "~/test/fixtures/shell";
import { providerCatalog, providerOptions } from "./providers";

const catalog = providerCatalog(providers);

describe("the provider catalog", () => {
  it("names each provider as the gateway does and one it doesn't list by its id", () => {
    expect(catalog.name("xai")).toBe("SpaceX");
    expect(catalog.name("togetherai")).toBe("Together AI");
    expect(catalog.name("cohere")).toBe("Cohere");
    expect(catalog.get("cohere")).toBeUndefined();
  });

  it("offers a provider's own formats, and every format for one it doesn't list", () => {
    expect(catalog.dialects("anthropic")).toEqual(["anthropic_messages"]);
    expect(catalog.dialects("openai")).toEqual(["openai_responses", "openai_chat", "openai_embeddings"]);
    expect(catalog.dialects("cohere")).toEqual(DIALECT_IDS);
  });

  it("sorts by name, not by id", () => {
    expect(["xai", "togetherai", "openrouter"].sort(catalog.byName)).toEqual(["openrouter", "xai", "togetherai"]);
  });

  it("offers every provider the gateway lists and those the organization still has keys for", () => {
    const keys = [credential({ provider: "openai" }), credential({ id: "cred_two", provider: "openai" }), credential({ id: "cred_old", provider: "cohere" })];
    const options = providerOptions(catalog, keys);
    expect(options.map((o) => o.id)).toEqual([...providers.map((p) => p.id), "cohere"]);
    expect(options[0]).toEqual({ id: "openai", name: "OpenAI", connected: true, hint: "2 keys" });
    expect(options.at(-1)).toMatchObject({ id: "cohere", name: "Cohere", connected: true });
    expect(providerOptions(catalog, keys, ["groq"])).toEqual([{ id: "groq", name: "Groq" }]);
    expect(providerOptions(providerCatalog([]), [])).toEqual([]);
  });
});
