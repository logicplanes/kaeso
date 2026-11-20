import type { ErrorGroupView, LogEntryView, LogView, PlaygroundModel } from "~/api/generated/types";
import { USER } from "../fixtures";
import { apiError, on, page } from "../network";

export function logRow(overrides: Partial<LogView> = {}): LogView {
  return {
    id: "req_0001",
    client_ip: "203.0.113.7",
    cost: "0.000420",
    cost_known: true,
    cost_basis: "reported",
    duration_ms: 1250,
    egress_dialect: "openai_chat",
    endpoint: "/v1/chat/completions",
    error_kind: null,
    error_message: null,
    has_content: true,
    ingress_dialect: "openai_chat",
    key_id: "key_web",
    model: "gpt-5.4-mini",
    overhead_us: 3000,
    request_upload_us: 40_000,
    principal_id: USER,
    principal_kind: "user",
    provider: "openai",
    requested_model: "gpt-5.4-mini",
    source: "api",
    started_at: "2026-10-15T11:58:30Z",
    status: 200,
    stream: false,
    stream_progress: null,
    ttfb_ms: 420,
    upstream_request_id: "chatcmpl-abc123",
    usage: { input_tokens: 120, output_tokens: 48, cache_read_tokens: 0, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false },
    user_agent: "openai-python/1.40.0",
    line: null,
    fallbacks: [],
    tags: [],
    guardrails: [],
    currency: "USD",
    ...overrides,
    converted_cost: overrides.converted_cost !== undefined ? overrides.converted_cost : (overrides.cost ?? "0.000420"),
  };
}

export function logEntry(overrides: Partial<LogEntryView> = {}): LogEntryView {
  return { ...logRow(), content: null, ...overrides };
}

export const listPage = page;

export function playgroundModel(overrides: Partial<PlaygroundModel> = {}): PlaygroundModel {
  return {
    id: "cm_gpt",
    aliases: [],
    input_modalities: ["text"],
    model: "gpt-5.4-mini",
    provider: "openai",
    model_id: "gpt-5.4-mini",
    display_name: "GPT-5.4 mini",
    reasoning: false,
    efforts: [],
    temperature: true,
    input_price: "0.75",
    output_price: "4.5",
    cache_read_price: "0.075",
    cache_write_price: null,
    price_tiers: [],
    provider_key: true,
    needs_api_key: false,
    max_request_bytes: 33_554_432,
    mode: "text",
    voices: [],
    translations: false,
    durations: false,
    availability: "available",
    ...overrides,
  };
}

export function sse(events: { type: string; [field: string]: unknown }[]): string {
  return events.map((e) => `event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`).join("");
}

export function eventStream(body: string, headers: Record<string, string> = {}): Response {
  return new Response(body, { status: 200, headers: { "content-type": "text/event-stream", ...headers } });
}

export function logsData({ rows = [], entries = [], errors = [] }: { rows?: LogView[]; entries?: LogEntryView[]; errors?: ErrorGroupView[] } = {}): void {
  on("GET", "orgs/:org/projects/:project/logs", listPage(rows));
  on("GET", "orgs/:org/projects/:project/logs/:log", ({ params }: { params: Record<string, string> }) => {
    const entry = entries.find((e) => e.id === params.log);
    return entry ?? apiError(404, "not_found", "No such request.");
  });
  // Registered last, so it answers before `logs/:log`.
  on("GET", "orgs/:org/projects/:project/logs/errors", page(errors));
}

export function errorGroup(overrides: Partial<ErrorGroupView> = {}): ErrorGroupView {
  return {
    error_kind: "rate_limited",
    status: 429,
    provider: "openai",
    count: 12,
    first_at: "2026-10-15T09:00:00Z",
    last_at: "2026-10-15T11:58:00Z",
    example: "Rate limit reached for gpt-5.4-mini; retry after 12 s.",
    request_id: "req_0009",
    ...overrides,
  };
}

export function logContent(request: unknown, reply: unknown): NonNullable<LogEntryView["content"]> {
  return { request, reply } as NonNullable<LogEntryView["content"]>;
}
