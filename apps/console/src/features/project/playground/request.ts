import type { PlaygroundRequest } from "~/api/playground";
import type { PlaygroundModel } from "~/api/queries/playground";
import type { PlaygroundParameters } from "@logicplanes/kaeso-ui/playground";
import { historyItems, type ToolOutput, type Turn } from "./responses";

export const modelRef = (m: Pick<PlaygroundModel, "provider" | "model_id">) => `${m.provider}/${m.model_id}`;

export type FunctionTool = { type: "function"; name: string; description: string; parameters: Record<string, unknown> };

export function requestFor(
  m: PlaygroundModel,
  turns: readonly Turn[],
  next: string | readonly ToolOutput[],
  p: PlaygroundParameters,
  tools: readonly FunctionTool[] = [],
): PlaygroundRequest {
  const effort = p.reasoningEffort && (m.efforts.length === 0 || m.efforts.includes(p.reasoningEffort)) ? p.reasoningEffort : undefined;
  const summary = p.reasoningSummary === "off" ? undefined : (p.reasoningSummary ?? "auto");
  const reasoning = m.reasoning && (effort || summary) ? { ...(effort ? { effort } : {}), ...(summary ? { summary } : {}) } : null;
  return {
    model: m.model,
    input: [
      ...historyItems(turns),
      ...(typeof next === "string"
        ? [{ role: "user" as const, content: next }]
        : next.map((o) => ({ type: "function_call_output", call_id: o.call_id, output: o.output }))),
    ],
    ...(tools.length ? { tools: tools.map((x) => x as unknown as Record<string, never>) } : {}),
    ...(p.instructions?.trim() ? { instructions: p.instructions } : {}),
    ...(m.temperature && p.temperature !== undefined ? { temperature: p.temperature } : {}),
    ...(m.temperature && p.topP !== undefined ? { top_p: p.topP } : {}),
    ...(p.maxOutputTokens !== undefined ? { max_output_tokens: p.maxOutputTokens } : {}),
    ...(reasoning ? { reasoning } : {}),
    stream: true,
  };
}
