export const AGENT_MODELS = [
  "claude",
  "gpt",
  "gemini",
  "gemma",
  "grok",
  "llama",
  "mistral",
  "deepseek",
  "qwen",
  "kimi",
  "glm",
  "minimax",
  "command",
  "nova",
  "phi",
  "granite",
  "jamba",
  "ernie",
  "hunyuan",
  "doubao",
  "yi",
  "nemotron",
  "olmo",
  "sonar",
  "reka",
  "unknown",
] as const;

export type AgentModel = (typeof AGENT_MODELS)[number];

/** The model family an API string names, or `unknown` for anything the library doesn't know. */
export function agentModel(model: string): AgentModel {
  return (AGENT_MODELS as readonly string[]).includes(model) ? (model as AgentModel) : "unknown";
}
