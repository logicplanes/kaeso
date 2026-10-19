import type { PlaygroundParameters } from "@logicplanes/kaeso-ui/playground";
import { validatePlaygroundSearch, type PlaygroundSearch } from "~/format/playground-search";

export function parametersOf(s: PlaygroundSearch): PlaygroundParameters {
  return {
    ...(s.system !== undefined ? { instructions: s.system } : {}),
    ...(s.temperature !== undefined ? { temperature: s.temperature } : {}),
    ...(s.top_p !== undefined ? { topP: s.top_p } : {}),
    ...(s.max_tokens !== undefined ? { maxOutputTokens: s.max_tokens } : {}),
    ...(s.effort !== undefined ? { reasoningEffort: s.effort } : {}),
    ...(s.summary !== undefined ? { reasoningSummary: s.summary } : {}),
  };
}

export function searchOf(p: PlaygroundParameters): Omit<PlaygroundSearch, "models"> {
  return validatePlaygroundSearch({
    system: p.instructions,
    temperature: p.temperature,
    top_p: p.topP,
    max_tokens: p.maxOutputTokens,
    effort: p.reasoningEffort,
    summary: p.reasoningSummary,
  });
}
