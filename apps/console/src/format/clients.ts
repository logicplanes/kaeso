import type { ClientApp, TimelineBucket, UsageRow } from "~/api/generated/types";
import { sumAmounts, USD } from "~/format/currency";
import { t } from "~/i18n";

/** Every client app the request log names from the user agent, as its `client` filter takes them. */
export const CLIENT_IDS: readonly ClientApp[] = [
  "codex",
  "claude-code",
  "openai-python",
  "openai-js",
  "anthropic-python",
  "anthropic-js",
  "ai-sdk",
  "curl",
  "other",
];

const NAMES: Record<string, string> = {
  codex: "Codex",
  "claude-code": "Claude Code",
  "openai-python": "OpenAI Python SDK",
  "openai-js": "OpenAI Node.js SDK",
  "anthropic-python": "Anthropic Python SDK",
  "anthropic-js": "Anthropic Node.js SDK",
  "ai-sdk": "AI SDK",
  curl: "curl",
};

/** A client app by its product name; requests without a user agent have none. */
export function clientLabel(id: string): string {
  if (!id) return t("format.client.none", "No user agent");
  return NAMES[id] ?? (id === "other" ? t("format.client.other", "Other client") : id);
}

/** The period's usage by client, summed from the hourly timeline grouped by `client`. */
export function clientUsage(buckets: readonly TimelineBucket[]): UsageRow[] {
  const by = new Map<string, TimelineBucket[]>();
  for (const b of buckets) by.set(b.key ?? "", [...(by.get(b.key ?? "") ?? []), b]);
  return [...by].map(([key, list]) => {
    const sum = (f: (b: TimelineBucket) => number) => list.reduce((n, b) => n + f(b), 0);
    const costs = list.map((b) => b.cost);
    const converted = list.map((b) => b.converted_cost);
    return {
      key,
      requests: sum((b) => b.requests),
      errors: sum((b) => b.errors),
      input_tokens: sum((b) => b.input_tokens),
      output_tokens: sum((b) => b.output_tokens),
      cache_read_tokens: sum((b) => b.cache_read_tokens),
      cache_write_tokens: sum((b) => b.cache_write_tokens),
      reasoning_tokens: sum((b) => b.reasoning_tokens),
      cost: costs.some((c) => c != null) ? sumAmounts(costs) : null,
      converted_cost: converted.some((c) => c != null) ? sumAmounts(converted) : null,
      currency: list[0]?.currency ?? USD,
      tool_calls: 0,
      tool_call_errors: 0,
      unpriced: 0,
      media: { images: 0, characters: 0, audio_ms: 0, videos: 0, video_ms: 0 },
    };
  });
}
