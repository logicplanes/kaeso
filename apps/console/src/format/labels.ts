import { locale, t } from "~/i18n";
import { formatNumber } from "@logicplanes/kaeso-ui/format";

const titled = (s: string) => s.charAt(0).toUpperCase() + s.slice(1).replace(/_/g, " ");

export const joinList = (items: readonly string[]) => items.join(t("format.listSeparator", ", "));

const ERROR_KINDS: Record<string, () => string> = {
  invalid_request: () => t("format.errorKind.invalidRequest", "Invalid request"),
  authentication: () => t("format.errorKind.authentication", "Authentication"),
  permission_denied: () => t("format.errorKind.permissionDenied", "Permission denied"),
  not_found: () => t("format.errorKind.notFound", "Not found"),
  request_too_large: () => t("format.errorKind.requestTooLarge", "Request too large"),
  rate_limited: () => t("format.errorKind.rateLimited", "Rate limited"),
  overloaded: () => t("format.errorKind.overloaded", "Provider overloaded"),
  context_too_long: () => t("format.errorKind.contextTooLong", "Context too long"),
  content_filtered: () => t("format.errorKind.contentFiltered", "Content filtered"),
  unsupported: () => t("format.errorKind.unsupported", "Unsupported by the target format"),
  timeout: () => t("format.errorKind.timeout", "Timeout"),
  billing: () => t("format.errorKind.billing", "Billing"),
  provider_error: () => t("format.errorKind.providerError", "Provider error"),
  stream_interrupted: () => t("format.errorKind.streamInterrupted", "Stream interrupted"),
  client_disconnected: () => t("format.errorKind.clientDisconnected", "Client disconnected"),
  client_stalled: () => t("format.errorKind.clientStalled", "Client stalled"),
  shutdown: () => t("format.errorKind.shutdown", "Gateway shutdown"),
};
export const ERROR_KIND_IDS = Object.keys(ERROR_KINDS);
export const errorKindLabel = (kind: string) => ERROR_KINDS[kind]?.() ?? titled(kind);

export const DIALECTS: Record<string, string> = {
  anthropic_messages: "Anthropic Messages",
  openai_chat: "OpenAI Chat Completions",
  openai_responses: "OpenAI Responses",
  openai_embeddings: "OpenAI Embeddings",
};
export const dialectLabel = (d: string) => DIALECTS[d] ?? d;

const ACCESS: Record<string, () => string> = {
  inference: () => t("format.access.inference", "Inference"),
  platform: () => t("format.access.platform", "Platform"),
  both: () => t("format.access.both", "Inference + platform"),
};
export const accessLabel = (a: string) => ACCESS[a]?.() ?? titled(a);

const KEY_STATES: Record<string, () => string> = {
  active: () => t("common.active", "Active"),
  disabled: () => t("format.keyState.disabled", "Disabled"),
  expired: () => t("common.expired", "Expired"),
  archived: () => t("common.archived", "Archived"),
};
export const keyStateLabel = (s: string) => KEY_STATES[s]?.() ?? titled(s);

const LIMIT_KINDS: Record<string, { label: () => string; unit: () => string }> = {
  request_rate: { label: () => t("format.limitKind.requestRate", "Request rate"), unit: () => t("format.limitUnit.requestsPerMinute", "requests / min") },
  token_rate: { label: () => t("format.limitKind.tokenRate", "Token rate"), unit: () => t("format.limitUnit.tokensPerMinute", "tokens / min") },
  token_budget: { label: () => t("format.limitKind.tokenBudget", "Token budget"), unit: () => t("format.limitUnit.tokens", "tokens") },
  spend_budget: { label: () => t("format.limitKind.spendBudget", "Spend budget"), unit: () => "USD" },
};
export const limitKindLabel = (k: string) => LIMIT_KINDS[k]?.label() ?? titled(k);
export const limitUnit = (k: string) => LIMIT_KINDS[k]?.unit() ?? "";

export function limitDescription(l: { kind: string; period: string; amount: string }): string {
  const amount = formatNumber(Number(l.amount), { maximumFractionDigits: 6, useGrouping: locale() !== "en" });
  const period = l.period;
  switch (l.kind) {
    case "request_rate":
      return t("format.limit.requestRate", "request rate of {amount} requests per minute", { amount });
    case "token_rate":
      return t("format.limit.tokenRate", "token rate of {amount} tokens per minute", { amount });
    case "token_budget":
      return t("format.limit.tokenBudget", "{period, select, day {daily} month {monthly} other {total}} token budget of {amount} tokens", { period, amount });
    default:
      return t("format.limit.spendBudget", "{period, select, day {daily} month {monthly} other {total}} spend budget of {amount} USD", { period, amount });
  }
}

const PERIODS: Record<string, () => string> = {
  minute: () => t("format.limitPeriod.minute", "Per minute"),
  day: () => t("format.limitPeriod.day", "Per day"),
  month: () => t("format.limitPeriod.month", "Per month"),
  total: () => t("format.limitPeriod.total", "In total"),
};
export const periodLabel = (p: string) => PERIODS[p]?.() ?? titled(p);

const SESSION_METHODS: Record<string, () => string> = {
  password: () => t("format.sessionMethod.password", "Password"),
  magic_link: () => t("format.sessionMethod.magicLink", "Sign-in link"),
  oidc: () => "SSO",
  setup: () => t("format.sessionMethod.setup", "Setup"),
  signup: () => t("format.sessionMethod.signup", "Sign-up"),
};
export const sessionMethodLabel = (m: string) => SESSION_METHODS[m]?.() ?? titled(m);

const USER_STATES: Record<string, () => string> = {
  active: () => t("common.active", "Active"),
  deactivated: () => t("format.userState.deactivated", "Deactivated"),
  deleted: () => t("format.userState.deleted", "Deleted"),
};
export const userStateLabel = (s: string) => USER_STATES[s]?.() ?? titled(s);

const INVITE_STATES: Record<string, () => string> = {
  pending: () => t("format.inviteState.pending", "Pending"),
  accepted: () => t("format.inviteState.accepted", "Accepted"),
  revoked: () => t("common.revoked", "Revoked"),
  expired: () => t("common.expired", "Expired"),
};
export const inviteStateLabel = (s: string) => INVITE_STATES[s]?.() ?? titled(s);
