import type { PlaygroundModel } from "~/api/queries/playground";
import { replyCost } from "./cost";
import type { ResponseState } from "./responses";

export function metricsOf(r: ResponseState | null, model: PlaygroundModel | null) {
  if (!r) return {};
  const u = r.usage ?? null;
  return {
    timeToFirstToken: r.firstTokenAt != null ? Math.max(0, Math.round(r.firstTokenAt - r.startedAt)) : null,
    duration: r.endedAt != null ? Math.max(0, Math.round(r.endedAt - r.startedAt)) : null,
    inputTokens: u?.input_tokens ?? null,
    outputTokens: u?.output_tokens ?? null,
    reasoningTokens: u?.reasoning_tokens || null,
    reasoning: !!model?.reasoning,
    live: r.status === "waiting" || r.status === "streaming",
    startedAt: Date.now() - (performance.now() - r.startedAt),
    cost: u ? replyCost(u, model) : null,
  };
}
