import type { PlaygroundModel } from "~/api/queries/playground";
import { replyCost } from "./cost";
import type { Mode, ResponseState } from "./responses";

export function metricsOf(r: ResponseState, model: PlaygroundModel | null, mode: Mode = "text") {
  const u = r.usage ?? null;
  const live = r.status === "waiting" || r.status === "streaming";
  const duration = r.endedAt != null ? Math.max(0, Math.round(r.endedAt - r.startedAt)) : null;
  if (mode !== "text")
    return {
      timeToFirstToken: null,
      duration,
      inputTokens: u?.input_tokens ?? null,
      outputTokens: u?.output_tokens ?? null,
      live: false,
      cost: r.cost ?? null,
      costKnown: r.costKnown ?? true,
    };
  return {
    timeToFirstToken: r.firstTokenAt != null ? Math.max(0, Math.round(r.firstTokenAt - r.startedAt)) : null,
    duration,
    inputTokens: u?.input_tokens ?? null,
    outputTokens: u?.output_tokens ?? null,
    reasoningTokens: u?.reasoning_tokens || null,
    live,
    startedAt: Date.now() - (performance.now() - r.startedAt),
    cost: u ? replyCost(u, model) : null,
  };
}
