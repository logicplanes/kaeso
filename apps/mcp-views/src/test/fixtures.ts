import type { CallToolResult } from "@modelcontextprotocol/client";
import type { Stats } from "../data";

export function stats(patch: Partial<Stats> = {}): Stats {
  return {
    metric: "cost",
    range: "7d",
    group_by: "model",
    scope: { org: { id: "org_1", name: "Acme" }, project: null },
    from: "2026-11-04T00:00:00Z",
    to: "2026-11-10T13:00:00Z",
    step: "day",
    totals: { requests: 1234, tokens: 2_500_000, cost: "12.5", errors: 12, latency_p95_ms: 1800, latency_p50_ms: 420 },
    series: ["04", "05", "06", "07", "08", "09", "10"].map((d, i) => ({ start: `2026-11-${d}`, value: String(i + 1) })),
    groups: [
      { key: "openai/gpt-5", label: "openai/gpt-5", value: "8", requests: 900 },
      { key: "anthropic/claude-sonnet-4-5", label: "anthropic/claude-sonnet-4-5", value: "4.5", requests: 334 },
    ],
    console_url: "https://kaeso.example.com/orgs/org_1/analytics",
    ...patch,
  };
}

export const ok = (s: Stats): CallToolResult => ({
  content: [{ type: "text", text: "Cost of Acme in the last 7 days: $12.50." }],
  structuredContent: s as unknown as Record<string, unknown>,
});

export const refused: CallToolResult = {
  isError: true,
  content: [{ type: "text", text: "insufficient_rights: You need View org usage." }],
  structuredContent: {
    error: {
      type: "insufficient_rights",
      right: "usage.org.view",
      message: "This agent doesn't have `usage.org.view`; its user can grant it in the console.",
      approve_url: "https://kaeso.example.com/settings/account/agents/agt_1?add=usage.org.view",
    },
  },
};
