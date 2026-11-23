/** A company that makes models, and the mark that stands for it. */
export interface ModelMaker {
  name: string;
  /**
   * Its mark: a provider's mark when the maker is a built-in provider, else one of the vendor marks. None when the owner's
   * terms don't allow its logo here (NOTICE.md); a row about its model then shows the provider's mark.
   */
  mark?: string | undefined;
}

interface MakerRule extends ModelMaker {
  /** The namespaces hosts write before the model's own name: `meta-llama/` on DeepInfra, `meta.` on Amazon Bedrock. */
  namespaces: readonly string[];
  /** The model's own name, for hosts that write no namespace: `llama-3.3-70b-versatile` on Groq. */
  names?: RegExp | undefined;
}

const MAKERS = {
  openai: {
    name: "OpenAI",
    mark: "openai",
    namespaces: ["openai"],
    names: /^(gpt|chatgpt|o\d|dall-e|sora|whisper|tts-1|codex|text-embedding-(3|ada)|omni-moderation)/,
  },
  anthropic: { name: "Anthropic", mark: "anthropic", namespaces: ["anthropic"], names: /^claude/ },
  deepseek: { name: "DeepSeek", mark: "deepseek", namespaces: ["deepseek", "deepseek-ai"], names: /^deepseek/ },
  xai: { name: "SpaceX", mark: "xai", namespaces: ["xai", "x-ai", "spacexai"], names: /^grok/ },
  mistral: {
    name: "Mistral",
    mark: "mistral",
    namespaces: ["mistral", "mistralai"],
    names: /^(mistral|mixtral|magistral|ministral|devstral|pixtral|voxtral|codestral|leanstral)/,
  },
  cohere: { name: "Cohere", mark: "cohere", namespaces: ["cohere", "cohereforai", "coherelabs"], names: /^(command|c4ai|aya|tiny-aya)/ },
  moonshotai: { name: "Moonshot", mark: "moonshotai", namespaces: ["moonshot", "moonshotai"], names: /^(kimi|moonshot)/ },
  alibaba: { name: "Alibaba", mark: "alibaba", namespaces: ["qwen", "alibaba", "alibaba-nlp", "thenlper", "wan-ai"], names: /^(qwen|qwq|qvq|wan-?\d)/ },
  zai: { name: "Z.ai", mark: "zai", namespaces: ["zai", "z-ai", "zai-org", "thudm", "zhipuai"], names: /^(glm|chatglm|cogview|cogvideo)/ },
  minimax: { name: "MiniMax", mark: "minimax", namespaces: ["minimax", "minimaxai"], names: /^(minimax|abab)/ },
  google: { name: "Google", mark: "google", namespaces: ["google", "deepmind"], names: /^(gemini|gemma|veo|imagen|lyria)/ },
  nvidia: { name: "NVIDIA", mark: "nvidia", namespaces: ["nvidia"], names: /^nemotron/ },
  stepfun: { name: "StepFun", mark: "stepfun-ai", namespaces: ["stepfun", "stepfun-ai"], names: /^(step-?\d|stepaudio)/ },
  perplexity: { name: "Perplexity", mark: "perplexity", namespaces: ["perplexity"], names: /^(sonar|pplx-)/ },
  upstage: { name: "Upstage", mark: "upstage", namespaces: ["upstage"], names: /^solar/ },
  jina: { name: "Jina AI", mark: "jina", namespaces: ["jinaai", "jina"], names: /^jina-/ },
  voyage: { name: "Voyage AI", mark: "voyage", namespaces: ["voyage", "voyageai"], names: /^voyage-/ },
  togetherai: { name: "Together AI", mark: "togetherai", namespaces: ["togethercomputer", "togetherai"] },
  baseten: { name: "Baseten", mark: "baseten", namespaces: ["baseten"] },
  fireworks: { name: "Fireworks AI", mark: "fireworks-ai", namespaces: ["fireworks"] },
  cloudflare: { name: "Cloudflare", mark: "cloudflare-ai-gateway", namespaces: ["cloudflare"] },
  openrouter: { name: "OpenRouter", mark: "openrouter", namespaces: ["openrouter"] },
  typesafe: { name: "TypeSafe", mark: "typesafe", namespaces: ["typesafe", "typesafe-ai"], names: /^jev-/ },
  cartesia: { name: "Cartesia", mark: "cartesia", namespaces: ["cartesia"] },
  deepgram: { name: "Deepgram", mark: "deepgram", namespaces: ["deepgram"] },
  assemblyai: { name: "AssemblyAI", mark: "assemblyai", namespaces: ["assemblyai"] },
  elevenlabs: { name: "ElevenLabs", mark: "elevenlabs", namespaces: ["elevenlabs"], names: /^eleven_/ },
  // AWS's trademark guidelines allow its marks in plain text only.
  amazon: { name: "Amazon", namespaces: ["amazon"] },
  meta: { name: "Meta", mark: "meta", namespaces: ["meta", "meta-llama", "meta-models", "facebook"], names: /^(llama|muse-)/ },
  microsoft: { name: "Microsoft", mark: "microsoft", namespaces: ["microsoft", "intfloat"], names: /^(phi-\d|mai-|(multilingual-)?e5-)/ },
  ibm: { name: "IBM", mark: "ibm", namespaces: ["ibm-granite", "ibm"], names: /^granite/ },
  writer: { name: "Writer", mark: "writer", namespaces: ["writer"], names: /^palmyra/ },
  // Baidu's terms forbid copying its logos without its permission.
  baidu: { name: "Baidu", namespaces: ["baidu", "paddlepaddle"], names: /^ernie/ },
  baai: { name: "BAAI", mark: "baai", namespaces: ["baai"], names: /^(text-embedding-)?bge-/ },
  bfl: { name: "Black Forest Labs", mark: "bfl", namespaces: ["black-forest-labs", "bfl"], names: /^flux/ },
  bytedance: {
    name: "ByteDance",
    mark: "bytedance",
    namespaces: ["bytedance", "bytedance-seed", "bytedance-research"],
    names: /^(seed-?\d|seedream|seedance|doubao|bytedance-)/,
  },
  kling: { name: "Kling AI", mark: "kling", namespaces: ["kling", "klingai", "kwaivgi"], names: /^kling/ },
  tencent: { name: "Tencent", mark: "tencent", namespaces: ["tencent"], names: /^(hunyuan|hy\d|hy-|youtu-)/ },
  xiaomi: { name: "Xiaomi", mark: "xiaomi", namespaces: ["xiaomi", "xiaomimimo"], names: /^(mimo-|xiaomi-)/ },
  meituan: { name: "Meituan", mark: "meituan", namespaces: ["meituan", "meituan-longcat"], names: /^longcat/ },
  inclusionai: { name: "inclusionAI", mark: "inclusionai", namespaces: ["inclusionai"], names: /^(ling|ring)-/ },
  liquid: { name: "Liquid AI", mark: "liquid", namespaces: ["liquid", "liquidai"], names: /^lfm/ },
  nousresearch: { name: "Nous Research", mark: "nousresearch", namespaces: ["nousresearch"], names: /^hermes-/ },
  reka: { name: "Reka", mark: "reka", namespaces: ["rekaai", "reka"], names: /^reka-/ },
  sakana: { name: "Sakana AI", mark: "sakana", namespaces: ["sakana", "sakanaai"] },
  recraft: { name: "Recraft", mark: "recraft", namespaces: ["recraft"], names: /^recraft/ },
  morph: { name: "Morph", mark: "morph", namespaces: ["morph"], names: /^morph-/ },
  inception: { name: "Inception", mark: "inception", namespaces: ["inception"], names: /^mercury/ },
  poolside: { name: "poolside", mark: "poolside", namespaces: ["poolside"], names: /^laguna/ },
  reflection: { name: "Reflection AI", mark: "reflection", namespaces: ["reflection", "reflection-ai"], names: /^beam(-|$)/ },
  thinkingmachines: { name: "Thinking Machines", mark: "thinkingmachines", namespaces: ["thinkingmachines"], names: /^inkling/ },
  essentialai: { name: "Essential AI", mark: "essentialai", namespaces: ["essentialai"], names: /^rnj-/ },
  perceptron: { name: "Perceptron", mark: "perceptron", namespaces: ["perceptron"] },
  relace: { name: "Relace", mark: "relace", namespaces: ["relace"] },
  runway: { name: "Runway", mark: "runway", namespaces: ["runway", "runwayml"] },
  topaz: { name: "Topaz Labs", mark: "topaz", namespaces: ["topaz", "topazlabs"] },
  fishaudio: { name: "Fish Audio", mark: "fishaudio", namespaces: ["fish-audio", "fishaudio"] },
  dotsstudio: { name: "Dots Studio", mark: "dotsstudio", namespaces: ["dots-studio", "rednote-hilab"] },
  aionlabs: { name: "AionLabs", mark: "aionlabs", namespaces: ["aion-labs"] },
  nomic: { name: "Nomic AI", mark: "nomic", namespaces: ["nomic-ai"], names: /^nomic-/ },
  mixedbread: { name: "Mixedbread", mark: "mixedbread", namespaces: ["mixedbread", "mixedbread-ai"], names: /^mxbai-/ },
  stability: { name: "Stability AI", mark: "stability", namespaces: ["stabilityai", "stability-ai", "stability"] },
  ideogram: { name: "Ideogram", mark: "ideogram", namespaces: ["ideogram"] },
  pixverse: { name: "PixVerse", mark: "pixverse", namespaces: ["pixverse"] },
  vidu: { name: "Vidu", mark: "vidu", namespaces: ["vidu"] },
  heygen: { name: "HeyGen", mark: "heygen", namespaces: ["heygen"] },
  moondream: { name: "Moondream", mark: "moondream", namespaces: ["moondream"] },
  baichuan: { name: "Baichuan", mark: "baichuan", namespaces: ["baichuan", "baichuan-inc"], names: /^baichuan/ },
  canopylabs: { name: "Canopy Labs", mark: "canopylabs", namespaces: ["canopylabs"] },
  swissai: { name: "Swiss AI Initiative", mark: "swiss-ai", namespaces: ["swiss-ai"], names: /^apertus/ },
  pfnet: { name: "Preferred Networks", mark: "pfnet", namespaces: ["pfnet", "pfnet-research"], names: /^plamo/ },
  lighton: { name: "LightOn", mark: "lightonai", namespaces: ["lightonai"], names: /^lightonocr/ },
  sdaia: { name: "SDAIA", mark: "sdaia", namespaces: ["sdaia", "allam-ai"], names: /^allam/ },
  daily: { name: "Daily", mark: "daily-co", namespaces: ["daily-co"] },
  motif: { name: "Motif Technologies", mark: "motif-technologies", namespaces: ["motif-technologies"] },
  parallel: { name: "Parallel Web Systems", mark: "parallel", namespaces: ["parallel"] },
  particle: { name: "Particle AI", mark: "particle", namespaces: ["particle", "particle-ai"] },
  prismml: { name: "PrismML", mark: "prism-ml", namespaces: ["prism-ml"] },
  nexagi: { name: "Nex AGI", mark: "nex-agi", namespaces: ["nex-agi"] },
  kt: { name: "KT", mark: "kt", namespaces: ["kt"] },
  callstack: { name: "Callstack", mark: "callstack", namespaces: ["callstack"] },
  humansand: { name: "Humans&", mark: "humansand", namespaces: ["humansand"] },
  interfaze: { name: "Interfaze", mark: "interfaze", namespaces: ["interfaze"] },
  quiverai: { name: "QuiverAI", mark: "quiverai", namespaces: ["quiverai"] },
  paperinstruments: { name: "Paper Instruments", mark: "paperinstruments", namespaces: ["paperinstruments"] },
  elorian: { name: "Elorian", mark: "elorian", namespaces: ["elorian"] },
  apodex: { name: "Apodex", mark: "apodex", namespaces: ["apodex"] },
  recursive: { name: "Recursive", mark: "recursive", namespaces: ["recursive"] },
  ricursive: { name: "Ricursive Intelligence", mark: "ricursiveai", namespaces: ["ricursiveai"] },
  respan: { name: "Respan", mark: "respan", namespaces: ["respan"] },
  nace: { name: "Nace.AI", mark: "nace-ai", namespaces: ["nace-ai"] },
  teleai: { name: "TeleAI", mark: "xingchen-agi", namespaces: ["xingchen-agi", "teleai"] },
  mindlab: { name: "Mind Lab", mark: "mindai", namespaces: ["mindai"] },
  unbiased: { name: "Unbiased", mark: "unbiased", namespaces: ["unbiased"] },
  dolphin: { name: "Dolphin AI", mark: "cognitivecomputations", namespaces: ["cognitivecomputations", "dphn"] },
  inferencenet: { name: "Inference.net", mark: "inference-net", namespaces: ["inference-net"], names: /^schematron-/ },
} as const satisfies Record<string, MakerRule>;

export type ModelMakerId = keyof typeof MAKERS;

/** Every maker Kaeso recognises in a model id, with its name and mark. */
export const MODEL_MAKERS: Readonly<Record<ModelMakerId, ModelMaker>> = MAKERS;

interface Lookup {
  rules: readonly MakerRule[];
  byNamespace: ReadonlyMap<string, MakerRule>;
  prefixes: readonly string[];
}
let lookup: Lookup | undefined;
function makers(): Lookup {
  if (lookup) return lookup;
  const rules = Object.values(MAKERS) as readonly MakerRule[];
  const byNamespace = new Map(rules.flatMap((rule) => rule.namespaces.map((space) => [space, rule] as const)));
  // Hosts such as Venice write the namespace into the name with a hyphen: `openai-gpt-54`, `zai-org-glm-5`.
  const prefixes = [...byNamespace.keys()].filter((space) => space.length >= 3).sort((a, b) => b.length - a.length);
  return (lookup = { rules, byNamespace, prefixes });
}

// Amazon Bedrock's cross-region inference profiles: `global.`, `us.`, `eu.`, `apac.`, `us-gov.` and the like before the maker.
const REGION = /^(?:global|apac|us-gov|[a-z]{2})\.(?=[a-z0-9-]+\.)/;
// Fireworks writes its own account before every model: `accounts/fireworks/models/kimi-k3`, `accounts/fireworks/routers/glm-latest`.
const ACCOUNT = /^accounts\/[^/]+\/(models|routers)\//;
// Venice writes embedding models as `text-embedding-<model>`.
const EMBEDDING = /^text-embedding-/;
// Folders hosts write before every model: fal's `fal-ai/`, Workers AI's `@cf/`.
const HOST_FOLDERS: ReadonlySet<string> = new Set(["fal-ai", "cf"]);

/** Who made a model, read from its id as a host writes it; `undefined` when the id doesn't say. */
export function modelMaker(model: string): ModelMaker | undefined {
  const id = model.toLowerCase().replace(ACCOUNT, "").replace(REGION, "");
  const segments = id.split("/").map((segment) => segment.replace(/^[~@]/, ""));
  const name = segments.pop() ?? "";
  const { rules, byNamespace, prefixes } = makers();
  const nearest = segments.reverse();
  const dotted = /^([a-z0-9-]+)\./.exec(name)?.[1];
  for (const space of dotted ? [...nearest, dotted] : nearest) {
    const maker = byNamespace.get(space);
    if (maker) return maker;
  }
  const prefix = prefixes.find((space) => name.startsWith(`${space}-`));
  if (prefix) return byNamespace.get(prefix);
  // A namespace no maker owns, such as `EleutherAI/gpt-j-6b`, is someone else's model whatever its name says. Only ids
  // without one, or with a host's own folder before the name (fal writes `fal-ai/flux/dev`), are read by their name.
  const outer = nearest.at(-1);
  if (outer !== undefined && !HOST_FOLDERS.has(outer)) return undefined;
  for (const part of [name, name.replace(EMBEDDING, ""), ...nearest]) {
    const rule = rules.find((r) => r.names?.test(part));
    if (rule) return rule;
  }
  return undefined;
}
