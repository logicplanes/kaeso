import type { LogEntryView } from "~/api/generated/types";
import { MAX_SYSTEM, validatePlaygroundSearch, type PlaygroundSearch } from "~/format/playground-search";
import { requestView, type LogMessage } from "./render";

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const obj = (v: unknown): Obj => (isObj(v) ? v : {});

/** How a logged request opens in the Playground: its model and settings, and the prompt to put in the composer. */
export interface PlaygroundStart {
  search: PlaygroundSearch;
  /** The last user message's text; empty when it had none, such as only an image. */
  prompt: string;
  /** The system prompt was longer than the Playground takes, so it was left out. */
  systemLeftOut: boolean;
}

const textOf = (m: LogMessage): string =>
  typeof m.content === "string" ? m.content : m.content.flatMap((b) => (b.type === "text" ? [b.text] : [])).join("\n\n");

// The parameters as the app sent them, by format; the Playground's own checks drop what it can't take.
function parameters(dialect: string, body: Obj): Record<string, unknown> {
  if (dialect === "anthropic_messages")
    return { temperature: body.temperature, top_p: body.top_p, max_tokens: body.max_tokens, effort: obj(body.output_config).effort };
  if (dialect === "openai_chat")
    return { temperature: body.temperature, top_p: body.top_p, max_tokens: body.max_completion_tokens ?? body.max_tokens, effort: body.reasoning_effort };
  if (dialect === "openai_responses") {
    const reasoning = obj(body.reasoning);
    return { temperature: body.temperature, top_p: body.top_p, max_tokens: body.max_output_tokens, effort: reasoning.effort, summary: reasoning.summary };
  }
  const config = obj(body.generationConfig);
  return { temperature: config.temperature, top_p: config.topP, max_tokens: config.maxOutputTokens, effort: obj(config.thinkingConfig).thinkingLevel };
}

/** The Playground start of a logged chat request in a format the inspector reads, or null for any other. */
export function playgroundStart(e: Pick<LogEntryView, "ingress_dialect" | "provider" | "model" | "content">): PlaygroundStart | null {
  const body: unknown = e.content?.request;
  if (!e.model || !e.provider || !isObj(body)) return null;
  const view = requestView(e.ingress_dialect, body);
  if (!view) return null;
  const system = view.messages
    .filter((m) => m.role === "system" || m.role === "developer")
    .map(textOf)
    .filter((s) => s.trim())
    .join("\n\n");
  const last = view.messages.findLast((m) => m.role === "user");
  const systemLeftOut = system.length > MAX_SYSTEM;
  return {
    search: validatePlaygroundSearch({ ...parameters(e.ingress_dialect, body), models: `${e.provider}/${e.model}`, ...(systemLeftOut ? {} : { system }) }),
    prompt: last ? textOf(last).trim() : "",
    systemLeftOut,
  };
}
