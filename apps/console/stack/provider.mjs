import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";

const PORT = Number(process.env.PORT ?? 8080);
const TOKENS_PER_DELTA = 4;
const DEFAULT_TOKENS_PER_SECOND = 80;
const WORDS = [
  "the",
  "order",
  "ships",
  "tomorrow",
  "and",
  "the",
  "refund",
  "is",
  "on",
  "its",
  "way",
  "to",
  "your",
  "account",
  "within",
  "three",
  "business",
  "days",
  "we",
  "checked",
  "the",
  "warehouse",
  "records",
  "for",
  "parcel",
  "4471",
  "which",
  "left",
  "the",
  "dock",
  "this",
  "morning",
  "so",
  "tracking",
  "updates",
  "should",
  "appear",
  "soon",
];

const ERRORS = {
  "server-error": {
    status: { openai: 500, anthropic: 500 },
    openai: { type: "server_error", code: null, message: "The server had an error while processing your request. Sorry about that!" },
    anthropic: { type: "api_error", message: "Internal server error" },
  },
  overloaded: {
    status: { openai: 503, anthropic: 529 },
    openai: { type: "server_error", code: null, message: "The engine is currently overloaded, please try again later." },
    anthropic: { type: "overloaded_error", message: "Overloaded" },
  },
  "rate-limit": {
    status: { openai: 429, anthropic: 429 },
    openai: { type: "requests", code: "rate_limit_exceeded", message: "Rate limit reached for requests. Please try again in 20s." },
    anthropic: { type: "rate_limit_error", message: "Number of requests has exceeded your per-minute rate limit." },
    headers: { "retry-after": "20" },
  },
  "invalid-request": {
    status: { openai: 400, anthropic: 400 },
    openai: { type: "invalid_request_error", code: null, message: "The request is not valid for this model." },
    anthropic: { type: "invalid_request_error", message: "The request is not valid for this model." },
  },
  unauthorized: {
    status: { openai: 401, anthropic: 401 },
    openai: { type: "invalid_request_error", code: "invalid_api_key", message: "Incorrect API key provided." },
    anthropic: { type: "authentication_error", message: "invalid x-api-key" },
  },
};

const MODELS = {
  openai: ["gpt-6-sol", "gpt-6-luna", "gpt-5.6", "gpt-5.4-mini", "gpt-5.4-nano", "text-embedding-3-small", "text-embedding-3-large"],
  anthropic: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-haiku-4-5"],
  xai: ["grok-4.7", "grok-4.3", "grok-4.20-0309-reasoning", "grok-4.20-0309-non-reasoning"],
};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const id = (prefix) => `${prefix}${randomBytes(12).toString("hex")}`;
const now = () => Math.floor(Date.now() / 1000);

function marker(text, name) {
  const match = new RegExp(`\\[mock:${name}(?:=([^\\]]*))?\\]`).exec(text);
  return match ? (match[1] ?? "") : undefined;
}

function numberMarker(text, name) {
  const value = marker(text, name);
  if (value === undefined || value === "") return undefined;
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : undefined;
}

function pieces(tokens, seed) {
  let state = seed | 1;
  const next = () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return state >>> 0;
  };
  return Array.from({ length: Math.ceil(tokens / TOKENS_PER_DELTA) }, () => {
    let piece = "";
    while (piece.length < TOKENS_PER_DELTA * 4) piece += `${WORDS[next() % WORDS.length]} `;
    return piece;
  });
}

function shapeOf(raw) {
  const seed = createHash("sha256").update(raw).digest().readUInt32BE(0);
  const output = Math.max(1, numberMarker(raw, "gen") ?? 24 + (seed % 180));
  const input = Math.max(1, numberMarker(raw, "in") ?? Math.ceil(Buffer.byteLength(raw) / 4));
  return {
    output,
    input,
    cached: Math.min(numberMarker(raw, "cached") ?? 0, input),
    reasoning: numberMarker(raw, "reasoning") ?? 0,
    tps: numberMarker(raw, "tps") || DEFAULT_TOKENS_PER_SECOND,
    pieces: pieces(output, seed),
  };
}

function sampleArguments(tool, raw) {
  const properties = tool?.properties ?? {};
  const number = /\b\d{2,}\b/.exec(raw.replace(/\[mock:[^\]]*\]/g, ""))?.[0];
  return Object.fromEntries(
    Object.entries(properties).map(([name, schema]) => {
      if (schema?.type === "number" || schema?.type === "integer") return [name, Number(number ?? 1)];
      if (schema?.type === "boolean") return [name, true];
      if (/city|location|place/i.test(name)) return [name, "Paris"];
      return [name, /id|number/i.test(name) && number ? number : "example"];
    }),
  );
}

function toolCall(dialect, body, raw) {
  const tools = Array.isArray(body.tools) ? body.tools : [];
  if (!tools.length) return null;
  if (dialect === "openai_chat") {
    if (body.messages?.at(-1)?.role === "tool") return null;
    const fn = tools[0].function ?? tools[0];
    return { name: fn.name, arguments: JSON.stringify(sampleArguments(fn.parameters, raw)) };
  }
  if (dialect === "openai_responses") {
    const input = Array.isArray(body.input) ? body.input : [];
    if (input.at(-1)?.type === "function_call_output") return null;
    return { name: tools[0].name, arguments: JSON.stringify(sampleArguments(tools[0].parameters, raw)) };
  }
  const last = body.messages?.at(-1);
  if (Array.isArray(last?.content) && last.content.some((block) => block.type === "tool_result")) return null;
  return { name: tools[0].name, arguments: JSON.stringify(sampleArguments(tools[0].input_schema, raw)) };
}

function dialectOf(path, headers) {
  if (path.startsWith("/v1/messages") || (path.startsWith("/v1/models") && headers["x-api-key"])) return "anthropic_messages";
  if (path.startsWith("/v1/responses")) return "openai_responses";
  if (path.startsWith("/v1/embeddings")) return "openai_embeddings";
  return "openai_chat";
}

function authorized(dialect, headers) {
  if (dialect === "anthropic_messages") return Boolean(headers["x-api-key"]);
  return /^Bearer \S+/.test(headers.authorization ?? "");
}

function sendJson(res, status, body, headers = {}) {
  res.writeHead(status, { "content-type": "application/json", "x-request-id": id("req_"), ...headers });
  res.end(JSON.stringify(body));
}

function sendError(res, dialect, name) {
  const error = ERRORS[name] ?? ERRORS["server-error"];
  if (dialect === "anthropic_messages") {
    sendJson(res, error.status.anthropic, { type: "error", error: error.anthropic, request_id: id("req_") }, error.headers);
  } else {
    sendJson(
      res,
      error.status.openai,
      { error: { message: error.openai.message, type: error.openai.type, param: null, code: error.openai.code } },
      error.headers,
    );
  }
}

async function sendStream(res, chunks) {
  res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache", "x-request-id": id("req_") });
  let closed = false;
  res.on("close", () => {
    closed = true;
  });
  for (const { wait, text } of chunks) {
    if (wait > 0) await sleep(wait);
    if (closed) return;
    res.write(text);
  }
  res.end();
}

const event = (type, data) => ({ wait: 0, text: `event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n` });
const dataLine = (data) => ({ wait: 0, text: `data: ${typeof data === "string" ? data : JSON.stringify(data)}\n\n` });

function paced(shape, make) {
  const every = (TOKENS_PER_DELTA / shape.tps) * 1000;
  return shape.pieces.map((piece, i) => ({ ...make(piece), wait: i === 0 ? (shape.reasoning / shape.tps) * 1000 : every }));
}

function chatUsage(shape) {
  return {
    prompt_tokens: shape.input,
    completion_tokens: shape.output + shape.reasoning,
    total_tokens: shape.input + shape.output + shape.reasoning,
    prompt_tokens_details: { cached_tokens: shape.cached, audio_tokens: 0 },
    completion_tokens_details: { reasoning_tokens: shape.reasoning, audio_tokens: 0, accepted_prediction_tokens: 0, rejected_prediction_tokens: 0 },
  };
}

function responsesUsage(shape) {
  return {
    input_tokens: shape.input,
    input_tokens_details: { cached_tokens: shape.cached },
    output_tokens: shape.output + shape.reasoning,
    output_tokens_details: { reasoning_tokens: shape.reasoning },
    total_tokens: shape.input + shape.output + shape.reasoning,
  };
}

function anthropicUsage(shape, output) {
  return { input_tokens: shape.input - shape.cached, cache_creation_input_tokens: 0, cache_read_input_tokens: shape.cached, output_tokens: output };
}

function chat(res, body, shape, call) {
  const base = { id: id("chatcmpl-"), created: now(), model: body.model, system_fingerprint: "fp_mock" };
  const text = shape.pieces.join("").trimEnd();
  const finish = call ? "tool_calls" : "stop";
  if (!body.stream) {
    const message = call
      ? { role: "assistant", content: null, refusal: null, tool_calls: [{ id: id("call_"), type: "function", function: call }] }
      : { role: "assistant", content: text, refusal: null };
    return sendJson(res, 200, {
      ...base,
      object: "chat.completion",
      choices: [{ index: 0, message, logprobs: null, finish_reason: finish }],
      usage: chatUsage(shape),
      service_tier: "default",
    });
  }
  const chunk = (delta, finishReason = null) => ({
    ...base,
    object: "chat.completion.chunk",
    choices: [{ index: 0, delta, logprobs: null, finish_reason: finishReason }],
  });
  const content = call
    ? [
        dataLine(
          chunk({
            role: "assistant",
            content: null,
            tool_calls: [{ index: 0, id: id("call_"), type: "function", function: { name: call.name, arguments: "" } }],
          }),
        ),
        dataLine(chunk({ tool_calls: [{ index: 0, function: { arguments: call.arguments } }] })),
      ]
    : [dataLine(chunk({ role: "assistant", content: "", refusal: null })), ...paced(shape, (piece) => dataLine(chunk({ content: piece })))];
  const usage = body.stream_options?.include_usage ? [dataLine({ ...base, object: "chat.completion.chunk", choices: [], usage: chatUsage(shape) })] : [];
  return sendStream(res, [...content, dataLine(chunk({}, finish)), ...usage, dataLine("[DONE]")]);
}

function responses(res, body, shape, call) {
  const responseId = id("resp_");
  const itemId = call ? id("fc_") : id("msg_");
  const text = shape.pieces.join("").trimEnd();
  const item = call
    ? { type: "function_call", id: itemId, call_id: id("call_"), name: call.name, arguments: call.arguments, status: "completed" }
    : { type: "message", id: itemId, status: "completed", role: "assistant", content: [{ type: "output_text", text, annotations: [] }] };
  const response = (status, output, usage) => ({
    id: responseId,
    object: "response",
    created_at: now(),
    status,
    model: body.model,
    output,
    error: null,
    incomplete_details: null,
    parallel_tool_calls: true,
    tool_choice: body.tool_choice ?? "auto",
    tools: body.tools ?? [],
    usage,
  });
  if (!body.stream) return sendJson(res, 200, response("completed", [item], responsesUsage(shape)));
  let sequence = 0;
  const next = (type, data) => event(type, { sequence_number: sequence++, ...data });
  const opening = [
    next("response.created", { response: response("in_progress", [], null) }),
    next("response.in_progress", { response: response("in_progress", [], null) }),
  ];
  const middle = call
    ? [
        next("response.output_item.added", { output_index: 0, item: { ...item, arguments: "", status: "in_progress" } }),
        next("response.function_call_arguments.delta", { item_id: itemId, output_index: 0, delta: call.arguments }),
        next("response.function_call_arguments.done", { item_id: itemId, output_index: 0, arguments: call.arguments }),
      ]
    : [
        next("response.output_item.added", { output_index: 0, item: { ...item, status: "in_progress", content: [] } }),
        next("response.content_part.added", { item_id: itemId, output_index: 0, content_index: 0, part: { type: "output_text", text: "", annotations: [] } }),
        ...paced(shape, (piece) => next("response.output_text.delta", { item_id: itemId, output_index: 0, content_index: 0, delta: piece })),
        next("response.output_text.done", { item_id: itemId, output_index: 0, content_index: 0, text }),
        next("response.content_part.done", { item_id: itemId, output_index: 0, content_index: 0, part: { type: "output_text", text, annotations: [] } }),
      ];
  const closing = [
    next("response.output_item.done", { output_index: 0, item }),
    next("response.completed", { response: response("completed", [item], responsesUsage(shape)) }),
  ];
  return sendStream(res, [...opening, ...middle, ...closing]);
}

function messages(res, body, shape, call) {
  const messageId = id("msg_");
  const text = shape.pieces.join("").trimEnd();
  const stop = call ? "tool_use" : "end_turn";
  const block = call ? { type: "tool_use", id: id("toolu_"), name: call.name, input: JSON.parse(call.arguments) } : { type: "text", text };
  const output = shape.output + shape.reasoning;
  if (!body.stream) {
    return sendJson(res, 200, {
      id: messageId,
      type: "message",
      role: "assistant",
      model: body.model,
      content: [block],
      stop_reason: stop,
      stop_sequence: null,
      usage: anthropicUsage(shape, output),
    });
  }
  const start = event("message_start", {
    message: {
      id: messageId,
      type: "message",
      role: "assistant",
      model: body.model,
      content: [],
      stop_reason: null,
      stop_sequence: null,
      usage: anthropicUsage(shape, 1),
    },
  });
  const content = call
    ? [
        event("content_block_start", { index: 0, content_block: { ...block, input: {} } }),
        event("content_block_delta", { index: 0, delta: { type: "input_json_delta", partial_json: call.arguments } }),
      ]
    : [
        event("content_block_start", { index: 0, content_block: { type: "text", text: "" } }),
        event("ping", {}),
        ...paced(shape, (piece) => event("content_block_delta", { index: 0, delta: { type: "text_delta", text: piece } })),
      ];
  return sendStream(res, [
    start,
    ...content,
    event("content_block_stop", { index: 0 }),
    event("message_delta", { delta: { stop_reason: stop, stop_sequence: null }, usage: { output_tokens: output } }),
    event("message_stop", {}),
  ]);
}

function embeddings(res, body) {
  const inputs = Array.isArray(body.input) ? body.input : [body.input];
  const vector = (text) => {
    const digest = createHash("sha256").update(String(text)).digest();
    return Float32Array.from({ length: 64 }, (_, i) => (digest[i % digest.length] - 128) / 128);
  };
  const tokens = inputs.reduce((sum, text) => sum + Math.max(1, Math.ceil(String(text).length / 4)), 0);
  sendJson(res, 200, {
    object: "list",
    data: inputs.map((text, index) => {
      const v = vector(text);
      return { object: "embedding", index, embedding: body.encoding_format === "base64" ? Buffer.from(v.buffer).toString("base64") : Array.from(v) };
    }),
    model: body.model,
    usage: { prompt_tokens: tokens, total_tokens: tokens },
  });
}

function models(res, dialect, path, headers) {
  const anthropic = dialect === "anthropic_messages";
  const xai = !anthropic && /^Bearer xai-/.test(headers.authorization ?? "");
  const list = anthropic ? MODELS.anthropic : xai ? MODELS.xai : MODELS.openai;
  const describe = (model) =>
    anthropic
      ? { type: "model", id: model, display_name: model, created_at: "2026-01-01T00:00:00Z" }
      : { id: model, object: "model", created: 1767225600, owned_by: xai ? "xai" : "openai" };
  const one = path.slice("/v1/models/".length);
  if (one) return list.includes(one) ? sendJson(res, 200, describe(one)) : sendError(res, dialect, "invalid-request");
  const data = list.map(describe);
  return sendJson(res, 200, anthropic ? { data, has_more: false, first_id: data[0].id, last_id: data.at(-1).id } : { object: "list", data });
}

async function readBody(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

async function handle(req, res) {
  const path = new URL(req.url ?? "/", "http://mock").pathname;
  if (path === "/healthz") return sendJson(res, 200, { status: "ok" });
  const dialect = dialectOf(path, req.headers);
  const raw = await readBody(req);
  if (!authorized(dialect, req.headers)) return sendError(res, dialect, "unauthorized");
  const delay = numberMarker(raw, "delay");
  if (delay) await sleep(delay);
  const failure = marker(raw, "error");
  if (failure !== undefined) return sendError(res, dialect, failure);
  if (req.method === "GET" && path.startsWith("/v1/models")) return models(res, dialect, path, req.headers);
  let body;
  try {
    body = JSON.parse(raw || "{}");
  } catch {
    return sendError(res, dialect, "invalid-request");
  }
  const shape = shapeOf(raw);
  if (path === "/v1/messages/count_tokens") return sendJson(res, 200, { input_tokens: shape.input });
  if (path === "/v1/responses/input_tokens") return sendJson(res, 200, { object: "response.input_tokens", input_tokens: shape.input });
  if (dialect === "openai_embeddings") return embeddings(res, body);
  const call = toolCall(dialect, body, raw);
  if (dialect === "anthropic_messages") return messages(res, body, shape, call);
  if (dialect === "openai_responses") return responses(res, body, shape, call);
  return chat(res, body, shape, call);
}

createServer((req, res) => {
  handle(req, res).catch((error) => {
    console.error(error);
    if (!res.headersSent) sendJson(res, 500, { error: { message: "The mock provider failed.", type: "server_error", param: null, code: null } });
    else res.end();
  });
}).listen(PORT, () => console.log(`mock provider on :${PORT}`));
