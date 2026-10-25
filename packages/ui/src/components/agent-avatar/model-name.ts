import { useMemo } from "react";
import { englishText, useUiText, type UiText } from "../../i18n/context.js";
import { agentModel, type AgentModel } from "./model-glyphs.js";

const PRODUCT_NAMES: Readonly<Record<Exclude<AgentModel, "unknown">, string>> = {
  claude: "Claude",
  gpt: "GPT",
  gemini: "Gemini",
  gemma: "Gemma",
  grok: "Grok",
  llama: "Llama",
  mistral: "Mistral",
  deepseek: "DeepSeek",
  qwen: "Qwen",
  kimi: "Kimi",
  glm: "GLM",
  minimax: "MiniMax",
  command: "Command",
  nova: "Nova",
  phi: "Phi",
  granite: "Granite",
  jamba: "Jamba",
  ernie: "ERNIE",
  hunyuan: "Hunyuan",
  doubao: "Doubao",
  yi: "Yi",
  nemotron: "Nemotron",
  olmo: "OLMo",
  sonar: "Sonar",
  reka: "Reka",
};

/**
 * The display name of a model family from the API, such as `deepseek` → "DeepSeek". Product names are never
 * translated; `unknown` and ids the library doesn't know are "Unknown", translated through `t` (English without it).
 * In a component, prefer `useModelName()`, which translates into the page's language.
 */
export function modelName(model: string, t: UiText = englishText): string {
  const known = agentModel(model);
  return known === "unknown" ? t("agentAvatar.unknownModel", "Unknown") : PRODUCT_NAMES[known];
}

/** `modelName` in the language of the nearest `LocaleProvider`. */
export function useModelName(): (model: string) => string {
  const t = useUiText();
  return useMemo(() => (model: string) => modelName(model, t), [t]);
}
