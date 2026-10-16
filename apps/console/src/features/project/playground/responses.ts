import { t } from "~/i18n";

type ResponseStatus = "waiting" | "streaming" | "completed" | "incomplete" | "failed" | "stopped";

export interface ResponseUsage {
  input_tokens: number;
  output_tokens: number;
  reasoning_tokens: number;
  cached_tokens: number;
  cache_write_tokens: number;
  cost?: string;
}

type OutputItem = { type: string; [field: string]: unknown };

export interface ResponseState {
  status: ResponseStatus;
  text: string;
  reasoning: string;
  reasoned: boolean;
  output: OutputItem[];
  usage: ResponseUsage | null;
  error: string | null;
  incompleteReason: string | null;
  startedAt: number;
  firstTokenAt: number | null;
  firstTextAt: number | null;
  endedAt: number | null;
  parts: Record<string, string>;
  thoughts: Record<string, string>;
}

export const newResponse = (startedAt: number): ResponseState => ({
  status: "waiting",
  text: "",
  reasoning: "",
  reasoned: false,
  output: [],
  usage: null,
  error: null,
  incompleteReason: null,
  startedAt,
  firstTokenAt: null,
  firstTextAt: null,
  endedAt: null,
  parts: {},
  thoughts: {},
});

export const isLive = (r: ResponseState) => r.status === "waiting" || r.status === "streaming";

type Json = Record<string, unknown>;
const obj = (v: unknown): Json | null => (v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : null);
const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : 0);
const str = (v: unknown) => (typeof v === "string" ? v : "");

function usageOf(response: Json | null): ResponseUsage | null {
  const u = obj(response?.usage);
  if (!u) return null;
  const cost = u.cost;
  return {
    input_tokens: num(u.input_tokens),
    output_tokens: num(u.output_tokens),
    reasoning_tokens: num(obj(u.output_tokens_details)?.reasoning_tokens),
    cached_tokens: num(obj(u.input_tokens_details)?.cached_tokens),
    cache_write_tokens: num(obj(u.input_tokens_details)?.cache_write_tokens),
    ...(typeof cost === "string" || typeof cost === "number" ? { cost: String(cost) } : {}),
  };
}

const joined = (parts: Record<string, string>) =>
  Object.values(parts)
    .filter((p) => p !== "")
    .join("\n\n");

function errorMessage(data: Json): string {
  const nested = obj(data.error);
  const message = str(nested?.message) || str(data.message);
  return message || t("playground.modelError", "The model stopped with an error. Try again.");
}

export function applyEvent(state: ResponseState, event: string, data: Json, now: number): ResponseState {
  if (!isLive(state)) return state;
  const type = str(data.type) || event;
  const first = state.firstTokenAt ?? now;
  switch (type) {
    case "response.created":
    case "response.in_progress":
      return state.status === "waiting" ? { ...state, status: "streaming" } : state;
    case "response.output_item.added": {
      const item = obj(data.item);
      return item?.type === "reasoning" ? { ...state, status: "streaming", reasoned: true } : { ...state, status: "streaming" };
    }
    case "response.output_text.delta":
    case "response.refusal.delta": {
      const delta = str(data.delta);
      if (!delta) return state;
      const key = `${str(data.item_id) || num(data.output_index)}:${num(data.content_index)}`;
      const parts = { ...state.parts, [key]: (state.parts[key] ?? "") + delta };
      return { ...state, status: "streaming", parts, text: joined(parts), firstTokenAt: first, firstTextAt: state.firstTextAt ?? now };
    }
    case "response.reasoning_summary_part.added":
    case "response.reasoning_summary_text.delta":
    case "response.reasoning_text.delta": {
      const delta = str(data.delta);
      const index = type === "response.reasoning_text.delta" ? `t${num(data.content_index)}` : `s${num(data.summary_index)}`;
      const key = `${str(data.item_id) || num(data.output_index)}:${index}`;
      const thoughts = { ...state.thoughts, [key]: (state.thoughts[key] ?? "") + delta };
      return { ...state, status: "streaming", reasoned: true, thoughts, reasoning: joined(thoughts), ...(delta ? { firstTokenAt: first } : {}) };
    }
    case "response.output_item.done": {
      const item = obj(data.item);
      if (!item || typeof item.type !== "string") return state;
      const output = [...state.output];
      output[num(data.output_index)] = item as OutputItem;
      return { ...state, output, ...(item.type === "reasoning" ? { reasoned: true } : {}) };
    }
    case "response.completed":
    case "response.incomplete": {
      const response = obj(data.response);
      const output = Array.isArray(response?.output) ? (response.output as OutputItem[]) : state.output.filter(Boolean);
      const reason = str(obj(response?.incomplete_details)?.reason) || "unknown";
      return {
        ...state,
        status: type === "response.completed" ? "completed" : "incomplete",
        output,
        reasoned: state.reasoned || output.some((o) => o.type === "reasoning"),
        usage: usageOf(response) ?? state.usage,
        incompleteReason: type === "response.incomplete" ? reason : null,
        endedAt: now,
      };
    }
    case "response.failed": {
      const response = obj(data.response);
      return { ...state, status: "failed", error: errorMessage(obj(response?.error) ?? {}), usage: usageOf(response) ?? state.usage, endedAt: now };
    }
    case "error":
      return { ...state, status: "failed", error: errorMessage(data), endedAt: now };
    default:
      return state;
  }
}

export function endResponse(state: ResponseState, status: "stopped" | "failed", now: number, error?: string): ResponseState {
  if (!isLive(state)) return state;
  return { ...state, status, endedAt: now, error: error ?? null, output: state.output.filter(Boolean) };
}

export type InputItem = { role: "user" | "assistant"; content: string } | OutputItem;

export interface Turn {
  id: string;
  prompt: string;
  response: ResponseState;
}

function messageText(item: OutputItem): string {
  const content = Array.isArray(item.content) ? (item.content as Json[]) : [];
  return content
    .map((c) => (c.type === "output_text" ? str(c.text) : c.type === "refusal" ? str(c.refusal) : ""))
    .filter(Boolean)
    .join("\n\n");
}

export function historyItems(turns: readonly Turn[]): InputItem[] {
  const items: InputItem[] = [];
  for (const { prompt, response: r } of turns) {
    if (isLive(r)) continue;
    const finished = r.status === "completed" || r.status === "incomplete";
    if (!finished && !r.text) continue;
    items.push({ role: "user", content: prompt });
    if (finished && r.output.length) {
      for (const item of r.output) {
        if (item.type === "reasoning" && typeof item.encrypted_content === "string") items.push(item);
        else if (item.type === "message") {
          const text = messageText(item);
          if (text) items.push({ role: "assistant", content: text });
        }
      }
    } else if (r.text) items.push({ role: "assistant", content: r.text });
  }
  return items;
}
