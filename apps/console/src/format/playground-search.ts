export const EFFORTS = ["none", "minimal", "low", "medium", "high", "xhigh", "max"] as const;
type Effort = (typeof EFFORTS)[number];
const SUMMARIES = ["auto", "concise", "detailed", "off"] as const;
type Summary = (typeof SUMMARIES)[number];

export const MAX_COLUMNS = 4;
const MAX_SYSTEM = 8000;

export interface PlaygroundSearch {
  models?: string;
  system?: string;
  temperature?: number;
  top_p?: number;
  max_tokens?: number;
  effort?: Effort;
  summary?: Exclude<Summary, "auto">;
}

const MODEL = /^[a-z0-9][a-z0-9._-]*\/[A-Za-z0-9][\w.:@/+-]*$/;

export function parseModels(value: string | undefined): string[] {
  if (!value) return [];
  return value
    .split(",")
    .slice(0, MAX_COLUMNS)
    .map((m) => m.trim())
    .map((m) => (m.length <= 200 && MODEL.test(m) ? m : ""));
}

const number = (v: unknown, min: number, max: number, integer = false): number | undefined => {
  const n = typeof v === "number" ? v : typeof v === "string" && v.trim() ? Number(v) : NaN;
  if (!Number.isFinite(n) || n < min || n > max || (integer && !Number.isInteger(n))) return undefined;
  return n;
};

export function validatePlaygroundSearch(s: Record<string, unknown>): PlaygroundSearch {
  const models = typeof s.models === "string" ? parseModels(s.models) : [];
  const system = typeof s.system === "string" ? s.system : typeof s.system === "number" ? String(s.system) : "";
  const temperature = number(s.temperature, 0, 2);
  const topP = number(s.top_p, 0, 1);
  const maxTokens = number(s.max_tokens, 1, 1_000_000, true);
  const effort = EFFORTS.find((e) => e === s.effort);
  const summary = SUMMARIES.find((x) => x === s.summary && x !== "auto") as PlaygroundSearch["summary"];
  return {
    ...(models.some(Boolean) ? { models: models.join(",") } : {}),
    ...(system.trim() && system.length <= MAX_SYSTEM ? { system } : {}),
    ...(temperature !== undefined ? { temperature } : {}),
    ...(topP !== undefined ? { top_p: topP } : {}),
    ...(maxTokens !== undefined ? { max_tokens: maxTokens } : {}),
    ...(effort ? { effort } : {}),
    ...(summary ? { summary } : {}),
  };
}
