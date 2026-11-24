import { SOURCE_NAMES } from "@logicplanes/kaeso-ui";
import type { ExternalSourceView, SourceKindView, SourceStatus } from "~/api/generated/types";
import { t } from "~/i18n";

/** Every kind of source, in the order the add dialog offers them. */
export const SOURCE_KINDS: readonly SourceKindView[] = [
  "claude_code",
  "codex",
  "gemini_cli",
  "antigravity",
  "grok",
  "litellm",
  "openrouter",
  "opentelemetry",
  "http",
];

/** A kind's product name. */
export const kindName = (kind: string): string => SOURCE_NAMES[kind] ?? kind;

/** What a kind reports and how, in one line for the add dialog. */
export function kindHint(kind: SourceKindView): string {
  switch (kind) {
    case "claude_code":
      return t("project.sources.kind.claudeCode", "Its OpenTelemetry events: every request, its tokens and its MCP tool calls.");
    case "codex":
      return t("project.sources.kind.codex", "Its OpenTelemetry log events, set in ~/.codex/config.toml.");
    case "gemini_cli":
      return t("project.sources.kind.geminiCli", "Its OpenTelemetry events over HTTP, set in settings.json.");
    case "antigravity":
      return t("project.sources.kind.antigravity", "A hook script that sends each invocation's model calls.");
    case "grok":
      return t("project.sources.kind.grok", "Its OpenTelemetry log events: every request, its tokens, cost and MCP tool calls.");
    case "litellm":
      return t("project.sources.kind.litellm", "The proxy's generic API logger, with costs as LiteLLM priced them.");
    case "openrouter":
      return t("project.sources.kind.openrouter", "Broadcast to an OpenTelemetry collector: its traces with tokens and cost.");
    case "opentelemetry":
      return t("project.sources.kind.opentelemetry", "Any app or collector that exports GenAI spans or log events.");
    case "http":
      return t("project.sources.kind.http", "Batches of requests and tool calls as JSON, from your own scripts.");
  }
}

/** A source's status in a word. */
export function statusLabel(status: SourceStatus): string {
  switch (status) {
    case "waiting":
      return t("project.sources.status.waiting", "Waiting");
    case "connected":
      return t("project.sources.status.connected", "Connected");
    case "idle":
      return t("project.sources.status.idle", "Idle");
    case "disabled":
      return t("project.sources.status.disabled", "Disabled");
    case "archived":
      return t("project.sources.status.archived", "Archived");
  }
}

export function statusTone(status: SourceStatus): "success" | "warning" | "neutral" | "info" {
  if (status === "connected") return "success";
  if (status === "waiting") return "info";
  if (status === "idle") return "warning";
  return "neutral";
}

/** Why Kaeso refused what a source sent, from the code it keeps as the last error. */
export function refusalText(code: string): string {
  switch (code) {
    case "no_model":
      return t("project.sources.refusal.noModel", "An event named no model.");
    case "no_tool":
      return t("project.sources.refusal.noTool", "A tool call named no tool.");
    case "no_id":
      return t("project.sources.refusal.noId", "An event had no id to tell it from the others.");
    case "too_many_tokens":
      return t("project.sources.refusal.tooManyTokens", "An event counted more than 10 million tokens.");
    case "cost_too_high":
      return t("project.sources.refusal.costTooHigh", "An event cost more than $100.");
    case "bad_status":
      return t("project.sources.refusal.badStatus", "An event's status wasn't an HTTP status.");
    case "too_old":
      return t("project.sources.refusal.tooOld", "An event was more than 7 days old.");
    case "in_future":
      return t("project.sources.refusal.inFuture", "An event was dated in the future; check the client's clock.");
    case "nobody":
      return t("project.sources.refusal.nobody", "Its requests count for nobody who can see this project.");
    case "unreadable":
      return t("project.sources.refusal.unreadable", "A batch wasn't what this kind of source sends.");
    case "inactive":
      return t("project.sources.refusal.inactive", "It sent while disabled, or after the member or service account it counts for left.");
    case "unrecognized":
      return t("project.sources.refusal.unrecognized", "A batch held no records Kaeso reads; check the client and its telemetry settings.");
    default:
      return code;
  }
}

/** The day's figures, summed over the source's recent days. */
export function totals(s: Pick<ExternalSourceView, "activity">): { requests: number; toolCalls: number; refused: number; duplicates: number } {
  return s.activity.reduce(
    (sum, d) => ({
      requests: sum.requests + d.requests,
      toolCalls: sum.toolCalls + d.tool_calls,
      refused: sum.refused + d.refused,
      duplicates: sum.duplicates + d.duplicates,
    }),
    { requests: 0, toolCalls: 0, refused: 0, duplicates: 0 },
  );
}

/** Events a day over the last `days` days, oldest first, with days without any as 0. */
export function eventsByDay(s: Pick<ExternalSourceView, "activity">, today: string, days = 14): { day: string; events: number }[] {
  const by = new Map(s.activity.map((d) => [d.day, d.requests + d.tool_calls]));
  const end = Date.parse(`${today}T00:00:00Z`);
  return Array.from({ length: days }, (_, i) => {
    const day = new Date(end - (days - 1 - i) * 86_400_000).toISOString().slice(0, 10);
    return { day, events: by.get(day) ?? 0 };
  });
}
