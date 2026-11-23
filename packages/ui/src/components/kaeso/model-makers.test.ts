import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { MODEL_MAKERS, modelMaker } from "./model-makers.js";
import { PROVIDER_MARKS } from "./provider-marks.js";
import { VENDOR_MARKS } from "./vendor-marks.js";

const repo = join(__dirname, "../../../../..");
interface CatalogueModel {
  id: string;
  family: string | null;
}
const catalogue = JSON.parse(readFileSync(join(repo, "crates/kaeso-core/data/kpmr.json"), "utf8")) as {
  providers: Record<string, { name: string; models: Record<string, CatalogueModel> }>;
};
const providers = Object.keys(catalogue.providers);
const models = Object.entries(catalogue.providers).flatMap(([provider, p]) => Object.values(p.models).map((m) => ({ provider, ...m })));
const servers = [...readFileSync(join(repo, "crates/kaeso-core/src/servers.rs"), "utf8").matchAll(/suggested_id: "([^"]+)"/g)].map((m) => m[1]!);

// The namespaces in the catalogue's model ids that name no maker with a published logo, and why each shows its host's mark.
const UNMARKED_NAMESPACES: Record<string, string> = {
  aisingapore: "publishes its logo only as a raster too detailed to redraw",
  convaiinnovations: "publishes its logo only as a raster with small text",
  scaledown: "publishes its logo only as a raster of gradient halos",
  "sentence-transformers": "an open-source library without a logo",
  hexgrad: "a developer's own account",
  jaredpalmer: "a developer's own account",
  shibing624: "a developer's own account",
  whereisai: "a developer's own account",
  "anthracite-org": "a community group that fine-tunes other makers' models",
  gryphe: "a community group that fine-tunes other makers' models",
  mancer: "a community group that fine-tunes other makers' models",
  sao10k: "a community group that fine-tunes other makers' models",
  thedrummer: "a community group that fine-tunes other makers' models",
  undi95: "a community group that fine-tunes other makers' models",
  stealth: "a code name for an unannounced model",
  tbc: "a code name for an unannounced model",
};

// Providers and makers whose owners' terms don't allow their logos here (NOTICE.md): their tiles show the provider icon, and
// a row about one of their models the provider's mark.
const UNMARKED_PROVIDERS: Record<string, string> = {
  "amazon-bedrock": "AWS's trademark guidelines allow its marks in plain text only",
  fal: "fal's terms forbid using its logos without its written permission",
  qianfan: "Baidu's terms forbid copying its logos without its permission",
  sglang: "SGLang's logo is licensed CC BY-NC-ND 4.0, which allows no altered copies",
  "mlx-lm": "MLX LM has no logo of its own, and Apple's guidelines don't allow its logo for compatibility references",
};
const UNMARKED_MAKERS: Record<string, string> = {
  Amazon: UNMARKED_PROVIDERS["amazon-bedrock"]!,
  Baidu: UNMARKED_PROVIDERS.qianfan!,
};

// Namespaces that are part of how a host writes ids, not a maker.
const HOST_SEGMENTS = new Set(["cf", "accounts", "models", "routers", "fal-ai"]);

// The maker of each model family the catalogue names, where the family says it.
const FAMILY_MAKERS: readonly (readonly [RegExp, string])[] = [
  [/^(gpt|o|o-mini|o-pro|whisper)(-|$)/, "OpenAI"],
  [/^claude-/, "Anthropic"],
  [/^deepseek/, "DeepSeek"],
  [/^grok/, "SpaceX"],
  [/^(mistral-|magistral|ministral|devstral|pixtral|voxtral|codestral|leanstral)/, "Mistral"],
  [/^command-/, "Cohere"],
  [/^kimi/, "Moonshot"],
  [/^(qwen|qvq|wan$|gte$)/, "Alibaba"],
  [/^glm/, "Z.ai"],
  [/^minimax/, "MiniMax"],
  [/^(gemini|gemma|veo|lyria)/, "Google"],
  [/^nemotron/, "NVIDIA"],
  [/^step$/, "StepFun"],
  [/^(sonar|pplx-decider)/, "Perplexity"],
  [/^solar/, "Upstage"],
  [/^jina-/, "Jina AI"],
  [/^voyage/, "Voyage AI"],
  [/^(llama|muse)$/, "Meta"],
  [/^nova/, "Amazon"],
  [/^(phi|mai|e5)$/, "Microsoft"],
  [/^granite$/, "IBM"],
  [/^palmyra$/, "Writer"],
  [/^ernie$/, "Baidu"],
  [/^bge$/, "BAAI"],
  [/^flux$/, "Black Forest Labs"],
  [/^(seed|seedream)$/, "ByteDance"],
  [/^kling$/, "Kling AI"],
  [/^(Hy|hunyuan)$/, "Tencent"],
  [/^mimo$/, "Xiaomi"],
  [/^longcat$/, "Meituan"],
  [/^liquid$/, "Liquid AI"],
  [/^hermes$/, "Nous Research"],
  [/^reka$/, "Reka"],
  [/^(fugu|sakana-namazu)$/, "Sakana AI"],
  [/^recraft$/, "Recraft"],
  [/^morph$/, "Morph"],
  [/^mercury$/, "Inception"],
  [/^laguna$/, "poolside"],
  [/^beam$/, "Reflection AI"],
  [/^inkling$/, "Thinking Machines"],
  [/^rnj$/, "Essential AI"],
  [/^baichuan$/, "Baichuan"],
  [/^canopylabs$/, "Canopy Labs"],
  [/^mxbai$/, "Mixedbread"],
  [/^nomic$/, "Nomic AI"],
];
// Models whose catalogue family names a maker that didn't make them: another company's model of the same name, and
// fine-tunes of another maker's model by a group whose namespace the id names.
const NOT_THE_FAMILY_MAKER = new Set([
  "deepgram/nova-3",
  "openrouter/aion-labs/aion-rp-llama-3.1-8b",
  "openrouter/sao10k/l3-lunaris-8b",
  "openrouter/sao10k/l3.1-euryale-70b",
  "venice/olafangensan-glm-4.7-flash-heretic",
]);
const familyMaker = (family: string | null) => (family ? FAMILY_MAKERS.find(([pattern]) => pattern.test(family))?.[1] : undefined);

const namespacesOf = (id: string) => {
  const segments = id
    .toLowerCase()
    .replace(/^accounts\/[^/]+\/models\//, "")
    .split("/")
    .slice(0, -1)
    .map((segment) => segment.replace(/^[~@]/, ""));
  return segments.filter((segment) => !HOST_SEGMENTS.has(segment) && !/^v\d/.test(segment));
};

// The id as far as one of its namespaces, with a placeholder name: `fal-ai/flux/dev` up to `flux` is `fal-ai/flux/x`.
const upTo = (id: string, space: string) => {
  const lower = id.toLowerCase();
  return `${lower.slice(0, lower.indexOf(space) + space.length)}/x`;
};

describe("model makers", () => {
  it.each([
    ["global.openai.gpt-6.2-astra", "OpenAI"],
    ["us.openai.gpt-6.2-sol", "OpenAI"],
    ["openai.gpt-6.2-astra", "OpenAI"],
    ["eu.anthropic.claude-sonnet-4-5-20250929-v1:0", "Anthropic"],
    ["apac.amazon.nova-lite-v1:0", "Amazon"],
    ["us-gov.anthropic.claude-sonnet-4-5", "Anthropic"],
    ["us.meta.llama4-maverick-17b-instruct-v1:0", "Meta"],
    ["us.writer.palmyra-x5-v1:0", "Writer"],
    ["moonshotai.kimi-k2.5", "Moonshot"],
    ["qwen.qwen3-coder-480b-a35b-v1:0", "Alibaba"],
    ["alibaba/wan-3.0/text-to-video", "Alibaba"],
    ["fal-ai/veo3.1", "Google"],
    ["fal-ai/flux/dev", "Black Forest Labs"],
    ["kling/v4/text-to-video", "Kling AI"],
    ["bytedance/seedream/v5.5/edit", "ByteDance"],
    ["qwen3.5-397b-a17b", "Alibaba"],
    ["ernie-5.1", "Baidu"],
    ["bge-large-zh", "BAAI"],
    ["glm-5.3-flash", "Z.ai"],
    ["openai/gpt-oss-120b", "OpenAI"],
    ["@cf/qwen/qwen3-30b-a3b-fp8", "Alibaba"],
    ["@cf/meta/llama-3.1-8b-instruct", "Meta"],
    ["accounts/fireworks/models/kimi-k3", "Moonshot"],
    ["~anthropic/claude-latest", "Anthropic"],
    ["openai-gpt-62-astra", "OpenAI"],
    ["zai-org-glm-5-2", "Z.ai"],
    ["phi-4-reasoning", "Microsoft"],
    ["o4-mini", "OpenAI"],
    ["MiniMax-M3", "MiniMax"],
    ["Meta-Llama-3.3-70B-Instruct", "Meta"],
    ["us.stability.sd3-5-large-v1:0", "Stability AI"],
    ["cartesia/sonic-3", "Cartesia"],
  ])("reads %s as a model by %s", (id, maker) => {
    expect(modelMaker(id)?.name).toBe(maker);
  });

  it.each([
    "aura-2",
    "nova-3",
    "universal-3-6-pro",
    "venice-uncensored-1-2",
    "constructor/x",
    "toString",
    "",
    // Someone else's model, whatever its name says.
    "EleutherAI/gpt-j-6b",
    "AI-Sweden-Models/gpt-sw3-20b",
    "thedrummer/glm-4.5-air-steam",
    "thedrummer/mistral-small-ft",
    "mattshumer/reflection-llama-3.1-70b",
  ])("names no maker for %j", (id) => {
    expect(modelMaker(id)).toBeUndefined();
  });

  it("draws every maker with a provider or vendor mark unless its owner's terms forbid it, and keeps no vendor mark nobody uses", () => {
    const unmarked = Object.values(MODEL_MAKERS)
      .filter((maker) => !maker.mark)
      .map((maker) => maker.name);
    expect(unmarked.sort()).toEqual(Object.keys(UNMARKED_MAKERS).sort());
    for (const maker of Object.values(MODEL_MAKERS)) if (maker.mark) expect(PROVIDER_MARKS[maker.mark] ?? VENDOR_MARKS[maker.mark], maker.name).toBeDefined();
    const used = new Set(Object.values(MODEL_MAKERS).map((maker) => maker.mark));
    expect(Object.keys(VENDOR_MARKS).filter((mark) => !used.has(mark))).toEqual([]);
    expect(Object.keys(VENDOR_MARKS).filter((mark) => PROVIDER_MARKS[mark])).toEqual([]);
  });

  it("has a mark for every provider in the shipped catalogue and every inference server a custom provider can run", () => {
    expect(providers.length).toBeGreaterThan(40);
    expect(servers.length).toBeGreaterThan(10);
    expect([...providers, ...servers].filter((id) => !PROVIDER_MARKS[id]).sort()).toEqual(Object.keys(UNMARKED_PROVIDERS).sort());
    expect(Object.keys(UNMARKED_PROVIDERS).filter((id) => PROVIDER_MARKS[id])).toEqual([]);
  });

  it("knows the maker behind every namespace in the shipped catalogue's model ids, or why it has no mark", () => {
    const unknown = new Set<string>();
    for (const m of models)
      for (const space of namespacesOf(m.id))
        if (!modelMaker(upTo(m.id, space)) && !(space in UNMARKED_NAMESPACES) && !providers.includes(space)) unknown.add(space);
    expect([...unknown].sort()).toEqual([]);
    const seen = new Set(models.flatMap((m) => namespacesOf(m.id)));
    expect(Object.keys(UNMARKED_NAMESPACES).filter((space) => !seen.has(space))).toEqual([]);
  });

  it("names a maker for every model of the shipped catalogue whose family names one, and the same one", () => {
    const makerMark = (name: string | undefined) => Object.values(MODEL_MAKERS).find((maker) => maker.name === name)?.mark;
    // A provider's own model whose id names no maker shows the provider's mark: the maker's, or the platform the maker runs.
    const PLATFORM_OF: Record<string, string> = { qianfan: "Baidu" };
    const ownModel = (m: (typeof models)[number]) =>
      !modelMaker(m.id) && (makerMark(familyMaker(m.family)) === m.provider || PLATFORM_OF[m.provider] === familyMaker(m.family));
    const wrong = models
      .filter((m) => !NOT_THE_FAMILY_MAKER.has(`${m.provider}/${m.id}`) && !ownModel(m))
      .filter((m) => familyMaker(m.family) !== undefined && modelMaker(m.id)?.name !== familyMaker(m.family))
      .map((m) => `${m.provider}/${m.id} (${m.family}): ${modelMaker(m.id)?.name ?? "none"}, not ${familyMaker(m.family)}`);
    expect(wrong).toEqual([]);
  });
});
