import { describe, expect, it } from "vitest";
import { providers } from "~/test/fixtures";
import { credential } from "~/test/fixtures/shell";
import { providerCatalog, providerOptions, routedDialect } from "./providers";

const catalog = providerCatalog(providers);

describe("the provider catalog", () => {
  it("names each provider as the gateway does and one it doesn't list by its id", () => {
    expect(catalog.name("xai")).toBe("SpaceX");
    expect(catalog.name("togetherai")).toBe("Together AI");
    expect(catalog.name("vercel")).toBe("Vercel");
    expect(catalog.name("cohere")).toBe("Cohere");
    expect(catalog.name("acme")).toBe("acme");
    expect(catalog.get("acme")).toBeUndefined();
  });

  it("names the product of a provider's accounts, and the provider when it has none", () => {
    expect(catalog.product("openai")).toBe("ChatGPT");
    expect(catalog.product("anthropic")).toBe("Anthropic");
    expect(catalog.product("acme")).toBe("acme");
  });

  it("offers a provider's own formats, and none for one it doesn't list", () => {
    expect(catalog.dialects("anthropic")).toEqual(["anthropic_messages"]);
    expect(catalog.dialects("openai")).toEqual(["openai_responses", "openai_chat", "openai_embeddings"]);
    expect(catalog.dialects("vercel")).toEqual(["openai_chat", "openai_responses", "anthropic_messages", "openai_embeddings"]);
    expect(catalog.dialects("cohere")).toEqual(["openai_chat", "openai_embeddings"]);
    expect(catalog.dialects("acme")).toEqual([]);
  });

  it("sorts by name, not by id", () => {
    expect(["xai", "vercel", "togetherai", "openrouter"].sort(catalog.byName)).toEqual(["openrouter", "xai", "togetherai", "vercel"]);
  });

  it("offers only the providers the gateway lists, with the organization's key counts", () => {
    const keys = [credential({ provider: "openai" }), credential({ id: "cred_two", provider: "openai" }), credential({ id: "cred_old", provider: "acme" })];
    const options = providerOptions(catalog, keys);
    expect(options.map((o) => o.id)).toEqual(providers.map((p) => p.id));
    expect(options[0]).toEqual({ id: "openai", name: "OpenAI", connected: true, hint: "2 keys" });
    expect(providerOptions(catalog, keys, ["acme"])).toEqual([{ id: "acme", name: "acme", connected: true, hint: "1 key" }]);
    expect(providerOptions(catalog, keys, ["groq"])).toEqual([{ id: "groq", name: "Groq" }]);
    expect(providerOptions(providerCatalog([]), [])).toEqual([]);
  });
});

describe("routedDialect", () => {
  const platform = providers.find((p) => p.id === "gemini-agent-platform");

  it("sends a model id in the format of the provider's first matching pattern", () => {
    expect(routedDialect(platform, "claude-opus-5-5@default")).toBe("anthropic_messages");
    expect(routedDialect(platform, "meta/llama-4-maverick-17b-128e-instruct-maas")).toBe("openai_chat");
    expect(routedDialect(platform, "gemini-3.5-flash")).toBeUndefined();
    expect(
      routedDialect(
        providers.find((p) => p.id === "openai"),
        "claude-x",
      ),
    ).toBeUndefined();
    expect(routedDialect(undefined, "claude-x")).toBeUndefined();
    const only = (pattern: string) => (platform ? { ...platform, model_routes: [{ pattern, dialect: "openai_chat" }] } : undefined);
    expect(routedDialect(only("exact"), "exact")).toBe("openai_chat");
    expect(routedDialect(only("exact"), "exactly")).toBeUndefined();
    expect(routedDialect(only("a*b*c"), "aXbYc")).toBe("openai_chat");
    expect(routedDialect(only("a*b*c"), "aXcYd")).toBeUndefined();
    expect(routedDialect(only("ab*ba"), "aba")).toBeUndefined();
  });
});
