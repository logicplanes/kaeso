import type { ContentBlock, ConversationMessage } from "@logicplanes/kaeso-ui";
import { leftOutOf, type LeftOut } from "./left-out";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";

type Json = unknown;
type Obj = Record<string, Json>;
const isObj = (v: Json): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: Json): string | undefined => (typeof v === "string" ? v : undefined);
const arr = (v: Json): Json[] => (Array.isArray(v) ? v : []);

type Plain = Exclude<ContentBlock, { type: "tool_result" }>;
export type LogBlock =
  | Plain
  | { type: "left_out"; media: LeftOut; name?: string | undefined }
  | { type: "tool_result"; tool_call_id?: string | undefined; content: string | LogBlock[]; is_error?: boolean | undefined };

export interface LogMessage {
  role: ConversationMessage["role"];
  content: string | LogBlock[];
}

const leftOutBlock = (v: Json, name?: string): LogBlock | null => {
  const media = leftOutOf(v);
  return media ? { type: "left_out", media, ...(name ? { name } : {}) } : null;
};

const MEDIA_BLOCKS = new Set<Json>(["image", "document", "audio", "video"]);

interface ToolInfo {
  name: string;
  description?: string;
  schema?: Json;
}

export interface RequestView {
  messages: LogMessage[];
  tools: ToolInfo[];
  params: { label: string; value: string }[];
}

export interface EmbeddingsView {
  inputs: number;
}

class Unmapped extends Error {}
const fail = (): never => {
  throw new Unmapped();
};

const ROLES = new Set(["system", "developer", "user", "assistant", "tool"]);
const role = (v: Json): ConversationMessage["role"] => {
  const r = str(v);
  return r && ROLES.has(r) ? (r as ConversationMessage["role"]) : fail();
};

const parseArgs = (v: Json): Json => {
  if (typeof v !== "string") return v;
  try {
    return JSON.parse(v) as Json;
  } catch {
    return v;
  }
};

const fieldsOf = (v: Obj) => Object.entries(v).filter(([, x]) => x !== null && x !== undefined);
const show = (v: Json) => (typeof v === "string" ? v : JSON.stringify(v));
const nested = (v: Json): string => (isObj(v) && fieldsOf(v).length ? `(${param(v)})` : param(v));

function param(v: Json): string {
  if (Array.isArray(v)) return v.map(nested).join(", ");
  if (isObj(v) && fieldsOf(v).length)
    return fieldsOf(v)
      .map(([k, x]) => `${k}: ${nested(x)}`)
      .join(", ");
  return show(v);
}

const redactedOpaque = (opaque: Json): boolean => arr(opaque).some((o) => isObj(o) && o.kind === "anthropic_redacted_thinking");

const BLOB = "kpp:reasoning:v1:";

function blobBlock(v: Json): Obj | null {
  const s = str(v);
  if (!s?.startsWith(BLOB)) return null;
  try {
    const b64 = s.slice(BLOB.length).replace(/-/g, "+").replace(/_/g, "/");
    const bytes = Uint8Array.from(atob(b64 + "=".repeat((4 - (b64.length % 4)) % 4)), (c) => c.charCodeAt(0));
    const block = JSON.parse(new TextDecoder().decode(bytes)) as Json;
    return isObj(block) ? block : null;
  } catch {
    return null;
  }
}

const paragraphs = (parts: Json[]): string =>
  parts
    .map((p) => (typeof p === "string" ? p : isObj(p) ? (str(p.text) ?? "") : ""))
    .filter((t) => t !== "")
    .join("\n\n");

const reasoning = (text: string, redacted: boolean): LogBlock => ({ type: "reasoning", text, ...(!text && redacted ? { redacted: true } : {}) });

const paramLabels = (): Record<string, string> => ({
  max_tokens: t("logs.param.maxTokens", "Max tokens"),
  max_completion_tokens: t("logs.param.maxCompletionTokens", "Max completion tokens"),
  max_output_tokens: t("logs.param.maxOutputTokens", "Max output tokens"),
  temperature: t("logs.param.temperature", "Temperature"),
  top_p: t("logs.param.topP", "Top p"),
  top_k: t("logs.param.topK", "Top k"),
  stop: t("logs.param.stop", "Stop"),
  stop_sequences: t("logs.param.stopSequences", "Stop sequences"),
  stream: t("logs.param.stream", "Streamed"),
  thinking: t("logs.param.thinking", "Thinking"),
  reasoning: t("logs.param.reasoning", "Reasoning"),
  reasoning_effort: t("logs.param.reasoningEffort", "Reasoning effort"),
  output_config: t("logs.param.output", "Output"),
  tool_choice: t("logs.param.toolChoice", "Tool choice"),
  response_format: t("logs.param.responseFormat", "Response format"),
  text: t("logs.param.textFormat", "Text format"),
  seed: t("logs.param.seed", "Seed"),
  n: t("logs.param.choices", "Choices"),
  parallel_tool_calls: t("logs.param.parallelToolCalls", "Parallel tool calls"),
  service_tier: t("logs.param.serviceTier", "Service tier"),
  store: t("logs.param.store", "Store"),
  metadata: t("logs.inspector.metadata", "Metadata"),
  modalities: t("logs.param.modalities", "Output modalities"),
  audio: t("logs.param.audio", "Audio output"),
});

function params(body: Obj): RequestView["params"] {
  return Object.entries(paramLabels())
    .filter(([k]) => body[k] !== undefined && body[k] !== null)
    .map(([k, label]) => ({ label, value: param(body[k]) }));
}

function anthropicSource(source: Json): { src?: string; media_type?: string } {
  if (!isObj(source)) return {};
  const media = str(source.media_type);
  if (source.type === "base64" && str(source.data))
    return { src: `data:${media ?? "application/octet-stream"};base64,${String(source.data)}`, ...(media ? { media_type: media } : {}) };
  if (source.type === "url" && str(source.url)) return { src: String(source.url), ...(media ? { media_type: media } : {}) };
  return media ? { media_type: media } : {};
}

function anthropicBlocks(content: Json): LogBlock[] {
  if (typeof content === "string") return [{ type: "text", text: content }];
  return arr(content).map((b): LogBlock => {
    if (!isObj(b)) return fail();
    const left = MEDIA_BLOCKS.has(b.type) && isObj(b.source) ? leftOutBlock(b.source.data, str(b.title)) : null;
    if (left) return left;
    switch (b.type) {
      case "text":
        return { type: "text", text: str(b.text) ?? fail() };
      case "image": {
        const s = anthropicSource(b.source);
        return { type: "image", ...s };
      }
      case "document": {
        const s = anthropicSource(b.source);
        return { type: "document", ...(str(b.title) ? { name: String(b.title) } : {}), ...(s.media_type ? { media_type: s.media_type } : {}) };
      }
      case "video":
        return { type: "video", ...anthropicSource(b.source) };
      case "tool_use":
        return { type: "tool_call", ...(str(b.id) ? { id: String(b.id) } : {}), name: str(b.name) ?? fail(), arguments: b.input };
      case "tool_result":
        return {
          type: "tool_result",
          ...(str(b.tool_use_id) ? { tool_call_id: String(b.tool_use_id) } : {}),
          content: typeof b.content === "string" ? b.content : anthropicBlocks(b.content ?? []),
          ...(b.is_error === true ? { is_error: true } : {}),
        };
      case "thinking":
        return reasoning(str(b.thinking) ?? "", redactedOpaque(blobBlock(b.signature)?.opaque));
      case "redacted_thinking":
        return { type: "reasoning", text: "", redacted: true };
      default:
        return fail();
    }
  });
}

function anthropicRequest(body: Obj): RequestView {
  const messages: LogMessage[] = [];
  if (body.system !== undefined) messages.push({ role: "system", content: anthropicBlocks(body.system) });
  for (const m of arr(body.messages)) {
    if (!isObj(m)) fail();
    const msg = m as Obj;
    messages.push({ role: role(msg.role), content: anthropicBlocks(msg.content) });
  }
  const tools = arr(body.tools).map((t): ToolInfo => {
    if (!isObj(t)) return fail();
    return {
      name: str(t.name) ?? str(t.type) ?? fail(),
      ...(str(t.description) ? { description: String(t.description) } : {}),
      ...(t.input_schema ? { schema: t.input_schema } : {}),
    };
  });
  return { messages, tools, params: params(body) };
}

function chatParts(content: Json): LogBlock[] {
  if (typeof content === "string") return [{ type: "text", text: content }];
  if (content === null || content === undefined) return [];
  return arr(content).map((p): LogBlock => {
    if (!isObj(p)) return fail();
    switch (p.type) {
      case "text":
        return { type: "text", text: str(p.text) ?? fail() };
      case "image_url": {
        const left = isObj(p.image_url) ? leftOutBlock(p.image_url.url) : null;
        if (left) return left;
        const url = isObj(p.image_url) ? str(p.image_url.url) : undefined;
        return { type: "image", ...(url ? { src: url } : {}) };
      }
      case "file": {
        const name = isObj(p.file) ? str(p.file.filename) : undefined;
        const left = isObj(p.file) ? leftOutBlock(p.file.file_data, name) : null;
        if (left) return left;
        return { type: "document", ...(name ? { name } : {}) };
      }
      case "input_audio": {
        const left = isObj(p.input_audio) ? leftOutBlock(p.input_audio.data) : null;
        if (left) return left;
        const format = isObj(p.input_audio) ? str(p.input_audio.format) : undefined;
        const media = format ? (CHAT_AUDIO[format] ?? format) : undefined;
        return { type: "audio", ...(media ? { media_type: media } : {}) };
      }
      case "video_url": {
        const left = isObj(p.video_url) ? leftOutBlock(p.video_url.url) : null;
        if (left) return left;
        const url = isObj(p.video_url) ? str(p.video_url.url) : undefined;
        return { type: "video", ...(url ? { src: url } : {}) };
      }
      case "refusal":
        return { type: "refusal", text: str(p.refusal) ?? "" };
      default:
        return fail();
    }
  });
}

const CHAT_AUDIO: Record<string, string> = { wav: "audio/wav", mp3: "audio/mpeg" };

function chatRequest(body: Obj): RequestView {
  const messages = arr(body.messages).map((m): LogMessage => {
    if (!isObj(m)) return fail();
    const r = role(m.role);
    if (r === "tool") {
      return {
        role: "tool",
        content: [
          {
            type: "tool_result",
            ...(str(m.tool_call_id) ? { tool_call_id: String(m.tool_call_id) } : {}),
            content: typeof m.content === "string" ? m.content : chatParts(m.content),
          },
        ],
      };
    }
    const blocks = chatParts(m.content);
    if (str(m.refusal)) blocks.push({ type: "refusal", text: String(m.refusal) });
    for (const c of arr(m.tool_calls)) {
      if (!isObj(c) || !isObj(c.function)) fail();
      const call = c as Obj;
      const fn = call.function as Obj;
      blocks.push({ type: "tool_call", ...(str(call.id) ? { id: String(call.id) } : {}), name: str(fn.name) ?? fail(), arguments: parseArgs(fn.arguments) });
    }
    return { role: r, content: blocks };
  });
  const tools = arr(body.tools).map((t): ToolInfo => {
    if (!isObj(t)) return fail();
    const fn = isObj(t.function) ? t.function : undefined;
    return {
      name: str(fn?.name) ?? str(t.type) ?? fail(),
      ...(str(fn?.description) ? { description: String(fn?.description) } : {}),
      ...(fn?.parameters ? { schema: fn.parameters } : {}),
    };
  });
  return { messages, tools, params: params(body) };
}

function responsesParts(content: Json): LogBlock[] {
  if (typeof content === "string") return [{ type: "text", text: content }];
  return arr(content).map((p): LogBlock => {
    if (!isObj(p)) return fail();
    switch (p.type) {
      case "input_text":
      case "output_text":
        return { type: "text", text: str(p.text) ?? fail() };
      case "input_image":
        return leftOutBlock(p.image_url) ?? { type: "image", ...(str(p.image_url) ? { src: String(p.image_url) } : {}) };
      case "input_file":
        return leftOutBlock(p.file_data, str(p.filename)) ?? { type: "document", ...(str(p.filename) ? { name: String(p.filename) } : {}) };
      case "refusal":
        return { type: "refusal", text: str(p.refusal) ?? "" };
      default:
        return fail();
    }
  });
}

function responsesRequest(body: Obj): RequestView {
  const messages: LogMessage[] = [];
  if (str(body.instructions)) messages.push({ role: "system", content: String(body.instructions) });
  if (typeof body.input === "string") messages.push({ role: "user", content: body.input });
  else
    for (const item of arr(body.input)) {
      if (!isObj(item)) fail();
      const it = item as Obj;
      const type = str(it.type) ?? "message";
      if (type === "message") messages.push({ role: role(it.role), content: responsesParts(it.content) });
      else if (type === "function_call")
        messages.push({
          role: "assistant",
          content: [
            { type: "tool_call", ...(str(it.call_id) ? { id: String(it.call_id) } : {}), name: str(it.name) ?? fail(), arguments: parseArgs(it.arguments) },
          ],
        });
      else if (type === "function_call_output")
        messages.push({
          role: "tool",
          content: [{ type: "tool_result", ...(str(it.call_id) ? { tool_call_id: String(it.call_id) } : {}), content: show(it.output) }],
        });
      else if (type === "reasoning")
        messages.push({
          role: "assistant",
          content: [reasoning(paragraphs([...arr(it.summary), ...arr(it.content)]), redactedOpaque(blobBlock(it.encrypted_content)?.opaque))],
        });
      else fail();
    }
  const tools = arr(body.tools).map((t): ToolInfo => {
    if (!isObj(t)) return fail();
    return {
      name: str(t.name) ?? str(t.type) ?? fail(),
      ...(str(t.description) ? { description: String(t.description) } : {}),
      ...(t.parameters ? { schema: t.parameters } : {}),
    };
  });
  return { messages, tools, params: params(body) };
}

function geminiParts(parts: Json): LogBlock[] {
  return arr(parts).flatMap((p): LogBlock[] => {
    if (!isObj(p)) return fail();
    if (str(p.text) !== undefined) return [p.thought === true ? reasoning(String(p.text), false) : { type: "text", text: String(p.text) }];
    if (isObj(p.functionCall))
      return [
        {
          type: "tool_call",
          ...(str(p.functionCall.id) ? { id: String(p.functionCall.id) } : {}),
          name: str(p.functionCall.name) ?? fail(),
          arguments: p.functionCall.args ?? {},
        },
      ];
    if (isObj(p.functionResponse))
      return [
        {
          type: "tool_result",
          ...(str(p.functionResponse.id) ? { tool_call_id: String(p.functionResponse.id) } : {}),
          content: show(p.functionResponse.response ?? {}),
        },
      ];
    if (isObj(p.inlineData)) {
      const left = leftOutBlock(p.inlineData.data);
      if (left) return [left];
      const media = str(p.inlineData.mimeType);
      if (media?.startsWith("image/") && str(p.inlineData.data))
        return [{ type: "image", src: `data:${media};base64,${String(p.inlineData.data)}`, media_type: media }];
      if (media?.toLowerCase().startsWith("audio/")) return [{ type: "audio", media_type: media }];
      if (media?.toLowerCase().startsWith("video/")) return [{ type: "video", media_type: media }];
      return [{ type: "document", ...(media ? { media_type: media } : {}) }];
    }
    if (isObj(p.fileData)) {
      const media = str(p.fileData.mimeType);
      const uri = str(p.fileData.fileUri);
      if (media?.startsWith("image/")) return [{ type: "image", ...(uri ? { src: uri } : {}), media_type: media }];
      if (media?.toLowerCase().startsWith("audio/")) return [{ type: "audio", media_type: media }];
      if (media?.toLowerCase().startsWith("video/") || (!media && uri && YOUTUBE.test(uri)))
        return [{ type: "video", ...(uri ? { src: uri } : {}), ...(media ? { media_type: media } : {}) }];
      return [{ type: "document", ...(uri ? { name: uri } : {}), ...(media ? { media_type: media } : {}) }];
    }
    if (str(p.thoughtSignature) !== undefined && Object.keys(p).length === 1) return [];
    return fail();
  });
}

const YOUTUBE = /^https?:\/\/(www\.|m\.)?(youtube\.com|youtu\.be)\//i;

const GEMINI_ROLES: Record<string, ConversationMessage["role"]> = { user: "user", model: "assistant" };

const geminiParamLabels = (): Record<string, string> => ({
  maxOutputTokens: t("logs.param.maxOutputTokens", "Max output tokens"),
  temperature: t("logs.param.temperature", "Temperature"),
  topP: t("logs.param.topP", "Top p"),
  topK: t("logs.param.topK", "Top k"),
  stopSequences: t("logs.param.stopSequences", "Stop sequences"),
  thinkingConfig: t("logs.param.thinking", "Thinking"),
  seed: t("logs.param.seed", "Seed"),
  candidateCount: t("logs.param.choices", "Choices"),
  responseMimeType: t("logs.param.responseFormat", "Response format"),
  responseModalities: t("logs.param.modalities", "Output modalities"),
  speechConfig: t("logs.param.audio", "Audio output"),
});

function geminiRequest(body: Obj): RequestView {
  const messages: LogMessage[] = [];
  if (isObj(body.systemInstruction)) messages.push({ role: "system", content: geminiParts(body.systemInstruction.parts) });
  for (const c of arr(body.contents)) {
    if (!isObj(c)) fail();
    const content = c as Obj;
    const r = GEMINI_ROLES[str(content.role) ?? "user"] ?? fail();
    const blocks = geminiParts(content.parts);
    const results = blocks.length > 0 && blocks.every((b) => b.type === "tool_result");
    messages.push({ role: results ? "tool" : r, content: blocks });
  }
  const tools = arr(body.tools).flatMap((t): ToolInfo[] => {
    if (!isObj(t)) return fail();
    const declared = arr(t.functionDeclarations);
    if (declared.length === 0) return [{ name: Object.keys(t)[0] ?? fail() }];
    return declared.map((d): ToolInfo => {
      if (!isObj(d)) return fail();
      const schema = d.parametersJsonSchema ?? d.parameters;
      return {
        name: str(d.name) ?? fail(),
        ...(str(d.description) ? { description: String(d.description) } : {}),
        ...(schema ? { schema } : {}),
      };
    });
  });
  const config = isObj(body.generationConfig) ? body.generationConfig : {};
  const shown = Object.entries(geminiParamLabels())
    .filter(([k]) => config[k] !== undefined && config[k] !== null)
    .map(([k, label]) => ({ label, value: param(config[k]) }));
  if (isObj(body.toolConfig)) shown.push({ label: t("logs.param.toolChoice", "Tool choice"), value: param(body.toolConfig) });
  return { messages, tools, params: shown };
}

export function requestView(dialect: string, body: Json): RequestView | null {
  if (!isObj(body)) return null;
  try {
    if (dialect === "anthropic_messages") return anthropicRequest(body);
    if (dialect === "openai_chat") return chatRequest(body);
    if (dialect === "openai_responses") return responsesRequest(body);
    if (dialect === "gemini") return geminiRequest(body);
    return null;
  } catch (e) {
    if (e instanceof Unmapped) return null;
    throw e;
  }
}

export function embeddingsView(dialect: string, body: Json): EmbeddingsView | null {
  if (dialect === "gemini_embeddings" && isObj(body)) return { inputs: Array.isArray(body.requests) ? body.requests.length : 1 };
  if (dialect !== "openai_embeddings" || !isObj(body)) return null;
  const input = body.input;
  if (typeof input === "string") return { inputs: 1 };
  if (Array.isArray(input)) return { inputs: input.length > 0 && typeof input[0] === "number" ? 1 : input.length };
  return null;
}

export interface ReplyView {
  blocks: LogBlock[];
  unknown: Json[];
  stop: string;
  model: string;
  id: string;
}

const stops = (): Record<string, string> => ({
  end_turn: t("logs.stop.endTurn", "End of turn"),
  max_tokens: t("logs.stop.maxTokens", "Max tokens reached"),
  tool_use: t("logs.stop.toolUse", "Tool use"),
  content_filter: t("logs.stop.contentFilter", "Content filter"),
  refusal: t("logs.stop.refusal", "Refusal"),
  pause_turn: t("logs.stop.pauseTurn", "Paused turn"),
});

export function stopLabel(stop: Json): string {
  if (typeof stop === "string") return stops()[stop] ?? stop;
  if (isObj(stop) && str(stop.stop_sequence) !== undefined)
    return t("logs.stop.sequence", 'Stop sequence "{sequence}"', { sequence: String(stop.stop_sequence) });
  if (isObj(stop) && str(stop.other) !== undefined) return String(stop.other);
  return t("common.unknown", "Unknown");
}

function kppSource(source: Json): { src?: string; media_type?: string } {
  if (!isObj(source)) return {};
  if (str(source.url)) return { src: String(source.url) };
  if (isObj(source.base64)) {
    const media = str(source.base64.media_type);
    return { src: `data:${media ?? "application/octet-stream"};base64,${str(source.base64.data) ?? ""}`, ...(media ? { media_type: media } : {}) };
  }
  if (isObj(source.text)) return str(source.text.media_type) ? { media_type: String(source.text.media_type) } : {};
  return {};
}

const opaqueBytes = (opaque: Json[]) => opaque.reduce<number>((n, o) => n + (isObj(o) ? (str(o.data)?.length ?? 0) : 0), 0);

function kppBlock(b: Json, unknown: Json[]): LogBlock | null {
  if (!isObj(b)) return fail();
  const [tag, v] = Object.entries(b)[0] ?? fail();
  const x = isObj(v) ? v : {};
  const left = MEDIA_BLOCKS.has(tag) && isObj(x.source) && isObj(x.source.base64) ? leftOutBlock(x.source.base64.data, str(x.title)) : null;
  if (left) return left;
  switch (tag) {
    case "text":
      return { type: "text", text: str(x.text) ?? fail() };
    case "tool_call":
      return { type: "tool_call", ...(str(x.id) ? { id: String(x.id) } : {}), name: str(x.name) ?? fail(), arguments: x.arguments };
    case "tool_result":
      return {
        type: "tool_result",
        ...(str(x.call_id) ? { tool_call_id: String(x.call_id) } : {}),
        content: arr(x.content).flatMap((c) => kppBlock(c, unknown) ?? []),
        ...(x.is_error === true ? { is_error: true } : {}),
      };
    case "reasoning": {
      const text = paragraphs([...arr(x.summary), ...arr(x.text)]);
      const bytes = opaqueBytes(arr(x.opaque));
      const note =
        text && bytes ? `\n\n${t("logs.reasoning.signedBytes", "{count} bytes of provider-signed data, not readable.", { count: formatNumber(bytes) })}` : "";
      return reasoning(text + note, redactedOpaque(x.opaque));
    }
    case "refusal":
      return { type: "refusal", text: str(x.text) ?? "" };
    case "image":
      return { type: "image", ...kppSource(x.source) };
    case "document": {
      const media = kppSource(x.source).media_type;
      return { type: "document", ...(str(x.title) ? { name: String(x.title) } : {}), ...(media ? { media_type: media } : {}) };
    }
    case "audio": {
      const media = kppSource(x.source).media_type;
      return { type: "audio", ...(media ? { media_type: media } : {}), ...(str(x.transcript) ? { transcript: String(x.transcript) } : {}) };
    }
    case "video": {
      const s = kppSource(x.source);
      return { type: "video", ...(s.src && !s.src.startsWith("data:") ? { src: s.src } : {}), ...(s.media_type ? { media_type: s.media_type } : {}) };
    }
    case "unknown":
      unknown.push(v);
      return null;
    default:
      return fail();
  }
}

export const isKnownReplyForm = (reply: Json): boolean => !isObj(reply) || !("format" in reply);

export function replyView(reply: Json): ReplyView | null {
  if (!isObj(reply) || !isKnownReplyForm(reply) || !Array.isArray(reply.blocks)) return null;
  try {
    const unknown: Json[] = [];
    const blocks = reply.blocks.flatMap((b) => kppBlock(b, unknown) ?? []);
    return { blocks, unknown, stop: stopLabel(reply.stop), model: str(reply.model) ?? "", id: str(reply.id) ?? "" };
  } catch (e) {
    if (e instanceof Unmapped) return null;
    throw e;
  }
}
