import type { PlaygroundRequest } from "~/api/playground";
import type { PlaygroundModel } from "~/api/queries/playground";
import type { PlaygroundParameters } from "@logicplanes/kaeso-ui/playground";
import { historyItems, type Turn } from "./responses";

export const modelRef = (m: Pick<PlaygroundModel, "provider" | "model_id">) => `${m.provider}/${m.model_id}`;

export function requestFor(m: PlaygroundModel, turns: readonly Turn[], prompt: string, p: PlaygroundParameters): PlaygroundRequest {
  const effort = p.reasoningEffort && (m.efforts.length === 0 || m.efforts.includes(p.reasoningEffort)) ? p.reasoningEffort : undefined;
  const summary = p.reasoningSummary === "off" ? undefined : (p.reasoningSummary ?? "auto");
  const reasoning = m.reasoning && (effort || summary) ? { ...(effort ? { effort } : {}), ...(summary ? { summary } : {}) } : null;
  return {
    model: m.model,
    input: [...historyItems(turns), { role: "user", content: prompt }],
    ...(p.instructions?.trim() ? { instructions: p.instructions } : {}),
    ...(m.temperature && p.temperature !== undefined ? { temperature: p.temperature } : {}),
    ...(m.temperature && p.topP !== undefined ? { top_p: p.topP } : {}),
    ...(p.maxOutputTokens !== undefined ? { max_output_tokens: p.maxOutputTokens } : {}),
    ...(reasoning ? { reasoning } : {}),
    stream: true,
  };
}
