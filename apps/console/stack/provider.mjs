import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";
import { scriptedReply } from "./conversations.mjs";

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

// `[mock:fail=<model> <error>]` fails only the requests for that model, so an alias's route can
// fall back from it to the next step; a prompt may carry several.
function failureFor(text, model) {
  for (const m of text.matchAll(/\[mock:fail=([^\]\s]+) ([^\]]+)\]/g)) if (m[1] === model) return m[2];
  return undefined;
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

// The seed's prompts get their written answers, streamed fast so seeding stays quick. Tokens are
// counted from the text the way real tokenizers come out on average, about 3.6 characters each.
function scriptedShape(raw, body, reply) {
  const tokens = (text) => Math.ceil([...text].length / 3.6);
  const texts = (value) =>
    typeof value === "string" ? [value] : Array.isArray(value) ? value.flatMap((part) => texts(part?.text ?? part?.content ?? "")) : [];
  const system = texts(body.system).reduce((sum, text) => sum + tokens(text), 0);
  const messages = (body.messages ?? []).reduce((sum, m) => sum + 4 + texts(m.content).reduce((n, text) => n + tokens(text), 0), 0);
  const input = Math.max(1, system + messages + (typeof body.input === "string" ? tokens(body.input) : 0) + 3);
  return {
    output: Math.max(1, tokens(reply)),
    input,
    cached: raw.includes('"cache_control"') ? Math.min(system, input - 1) : 0,
    reasoning: 0,
    tps: 4000,
    pieces: reply.match(new RegExp(`[\\s\\S]{1,${TOKENS_PER_DELTA * 4}}`, "g")) ?? [reply],
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
  if (path === "/v1beta/decisions") return "gemini_decisions";
  if (path.endsWith("/v1/systemone")) return "system_one";
  if (path.endsWith("/v1/decisions")) return "decisions";
  if (path.startsWith("/v1/messages") || (path.startsWith("/v1/models") && headers["x-api-key"])) return "anthropic_messages";
  if (path.startsWith("/v1/responses")) return "openai_responses";
  if (path.startsWith("/v1/embeddings")) return "openai_embeddings";
  return "openai_chat";
}

function authorized(dialect, headers) {
  if (dialect === "anthropic_messages") return Boolean(headers["x-api-key"]);
  if (dialect === "gemini_decisions") return Boolean(headers["x-goog-api-key"]);
  return /^Bearer \S+/.test(headers.authorization ?? "");
}

// Each question gets an answer in the format it was asked in: yes 0.82, the first option, a score near the top level.
const YES = 0.82;

function spread(n) {
  return Array.from({ length: n }, (_, i) => (i === 0 ? 0.7 : 0.3 / Math.max(1, n - 1)));
}

function scored(n) {
  const p = Array.from({ length: n }, (_, i) => (n === 1 ? 1 : i === n - 1 ? 0.6 : i === n - 2 ? 0.4 : 0));
  return { p, score: p.reduce((sum, x, i) => sum + i * x, 0) };
}

const byIndex = (values) => Object.fromEntries(values.map((v, i) => [String(i), v]));

function decisions(res, dialect, body, shape) {
  if (dialect === "decisions" && body.state !== undefined) dialect = "system_one";
  if (dialect === "decisions") {
    const answers = (body.questions ?? []).map((q) => {
      const named = { name: q.name ?? null };
      if (q.type === "choice") {
        const p = spread(q.choices.length);
        return {
          type: "choice",
          ...named,
          choice: q.choices[0].value,
          probabilities: q.choices.map((c, i) => ({ value: c.value, probability: p[i] })),
          confidence: 0.6,
        };
      }
      if (q.type === "score") {
        const { p, score } = scored(q.levels.length);
        return { type: "score", ...named, score, probabilities: q.levels.map((l, i) => ({ value: i, label: l.label, probability: p[i] })), confidence: 0.5 };
      }
      return { type: "predicate", ...named, probability: YES };
    });
    const usage = {
      input_tokens: shape.input,
      input_tokens_details: { cached_tokens: 0, cache_write_tokens: 0 },
      output_tokens: 0,
      output_tokens_details: { reasoning_tokens: 0 },
      total_tokens: shape.input,
    };
    return sendJson(res, 200, { model: body.model, answers, usage });
  }
  const gemini = dialect === "gemini_decisions";
  const answers = Object.fromEntries(
    Object.entries(body.questions ?? {}).map(([key, q]) => {
      if (q.type === "choice") {
        const keys = Object.keys(q.criteria ?? {});
        const p = spread(keys.length);
        return [key, { type: "choice", choice: keys[0], probabilities: Object.fromEntries(keys.map((k, i) => [k, p[i]])), confidence: gemini ? 0.7 : 0.6 }];
      }
      if (q.type === "score") {
        const levels = q.criteria ?? [];
        const { p, score } = scored(levels.length);
        return [
          key,
          {
            type: "score",
            score,
            legend: byIndex(levels.map((l) => (typeof l === "string" ? l : JSON.stringify(l)))),
            probabilities: byIndex(p),
            confidence: gemini ? 0.6 : 0.5,
          },
        ];
      }
      if (q.type === "box") return [key, { type: "box", box: { top: 0.1, bottom: 0.9, left: 0.2, right: 0.8 }, confidence: null }];
      return [key, gemini ? { type: "noul", noul: YES, confidence: YES } : { type: "noul", noul: YES }];
    }),
  );
  if (gemini) {
    const usage = {
      total_input_tokens: shape.input,
      input_tokens_by_modality: [{ modality: "text", tokens: shape.input }],
      total_output_tokens: 0,
      total_tokens: shape.input,
    };
    return sendJson(res, 200, { answers, usage });
  }
  return sendJson(res, 200, { model: body.model, answers, usage: { input_tokens: shape.input, output_tokens: Object.keys(answers).length } });
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
  return Buffer.concat(chunks).toString("latin1");
}

// A 1×1 PNG, and an MPEG audio stream of silent frames: what OpenAI's media APIs answer with, in
// the shapes their API reference documents.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
const MP3_FRAME = Buffer.concat([Buffer.from([0xff, 0xfb, 0x90, 0x00]), Buffer.alloc(413)]);
const mp3 = (seconds) => Buffer.concat(Array.from({ length: Math.max(1, Math.round((seconds * 44100) / 1152)) }, () => MP3_FRAME));

function formField(raw, name) {
  return new RegExp(`name="${name}"\\r\\n\\r\\n([^\\r]*)`).exec(raw)?.[1];
}

function images(res, raw, body) {
  const multipart = !raw.trimStart().startsWith("{");
  const n = Number((multipart ? formField(raw, "n") : body.n) ?? 1);
  const prompt = (multipart ? formField(raw, "prompt") : body.prompt) ?? "";
  const tokens = Math.max(1, Math.ceil(prompt.length / 4));
  sendJson(res, 200, {
    created: now(),
    data: Array.from({ length: n }, () => ({ b64_json: PNG })),
    usage: {
      input_tokens: tokens,
      input_tokens_details: { text_tokens: tokens, image_tokens: multipart ? 194 : 0 },
      output_tokens: 272 * n,
      total_tokens: tokens + 272 * n,
    },
  });
}

function speech(res, body) {
  const characters = String(body.input ?? "").length;
  const audio = mp3(Math.max(0.5, characters / 15));
  if (body.stream_format !== "sse") {
    res.writeHead(200, { "content-type": "audio/mpeg", "x-request-id": id("req_") });
    return res.end(audio);
  }
  const usage = { input_tokens: Math.ceil(characters / 4), output_tokens: Math.ceil(audio.length / 64), total_tokens: 0 };
  usage.total_tokens = usage.input_tokens + usage.output_tokens;
  return sendStream(res, [dataLine({ type: "speech.audio.delta", audio: audio.toString("base64") }), dataLine({ type: "speech.audio.done", usage })]);
}

// xAI's video API, as its API reference documents it: a job started at `/v1/videos/generations`,
// asked about at `/v1/videos/{request_id}` until it is done, its file a link on the provider.
const MP4 = Buffer.from(
  "AAAAIGZ0eXBpc29tAAACAGlzb21pc28yYXZjMW1wNDEAAAMybW9vdgAAAGxtdmhkAAAAAAAAAAAAAAAAAAAD6AAAA+gAAQAAAQAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAlx0cmFrAAAAXHRraGQAAAADAAAAAAAAAAAAAAABAAAAAAAAA+gAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAACAAAAAgAAAAAAAkZWR0cwAAABxlbHN0AAAAAAAAAAEAAAPoAAAAAAABAAAAAAHUbWRpYQAAACBtZGhkAAAAAAAAAAAAAAAAAABAAAAAQABVxAAAAAAALWhkbHIAAAAAAAAAAHZpZGUAAAAAAAAAAAAAAABWaWRlb0hhbmRsZXIAAAABf21pbmYAAAAUdm1oZAAAAAEAAAAAAAAAAAAAACRkaW5mAAAAHGRyZWYAAAAAAAAAAQAAAAx1cmwgAAAAAQAAAT9zdGJsAAAAv3N0c2QAAAAAAAAAAQAAAK9hdmMxAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAACAAIABIAAAASAAAAAAAAAABFUxhdmM2Mi4yOC4xMDEgbGlieDI2NAAAAAAAAAAAAAAAGP//AAAANWF2Y0MBZAAK/+EAGGdkAAqs2UlsBEAAAAMAQAAAAwEDxIllgAEABmjr4OSyLP34+AAAAAAQcGFzcAAAAAEAAAABAAAAFGJ0cnQAAAAAAAAWyAAAAAAAAAAYc3R0cwAAAAAAAAABAAAAAgAAIAAAAAAUc3RzcwAAAAAAAAABAAAAAQAAABxzdHNjAAAAAAAAAAEAAAABAAAAAgAAAAEAAAAcc3RzegAAAAAAAAAAAAAAAgAAAs0AAAAMAAAAFHN0Y28AAAAAAAAAAQAAA2IAAABidWR0YQAAAFptZXRhAAAAAAAAACFoZGxyAAAAAAAAAABtZGlyYXBwbAAAAAAAAAAAAAAAAC1pbHN0AAAAJal0b28AAAAdZGF0YQAAAAEAAAAATGF2ZjYyLjEyLjEwMQAAAAhmcmVlAAAC4W1kYXQAAAKtBgX//6ncRem95tlIt5Ys2CDZI+7veDI2NCAtIGNvcmUgMTY1IHIzMjIyIGIzNTYwNWEgLSBILjI2NC9NUEVHLTQgQVZDIGNvZGVjIC0gQ29weWxlZnQgMjAwMy0yMDI1IC0gaHR0cDovL3d3dy52aWRlb2xhbi5vcmcveDI2NC5odG1sIC0gb3B0aW9uczogY2FiYWM9MSByZWY9MyBkZWJsb2NrPTE6MDowIGFuYWx5c2U9MHgzOjB4MTEzIG1lPWhleCBzdWJtZT03IHBzeT0xIHBzeV9yZD0xLjAwOjAuMDAgbWl4ZWRfcmVmPTEgbWVfcmFuZ2U9MTYgY2hyb21hX21lPTEgdHJlbGxpcz0xIDh4OGRjdD0xIGNxbT0wIGRlYWR6b25lPTIxLDExIGZhc3RfcHNraXA9MSBjaHJvbWFfcXBfb2Zmc2V0PS0yIHRocmVhZHM9MSBsb29rYWhlYWRfdGhyZWFkcz0xIHNsaWNlZF90aHJlYWRzPTAgbnI9MCBkZWNpbWF0ZT0xIGludGVybGFjZWQ9MCBibHVyYXlfY29tcGF0PTAgY29uc3RyYWluZWRfaW50cmE9MCBiZnJhbWVzPTMgYl9weXJhbWlkPTIgYl9hZGFwdD0xIGJfYmlhcz0wIGRpcmVjdD0xIHdlaWdodGI9MSBvcGVuX2dvcD0wIHdlaWdodHA9MiBrZXlpbnQ9MjUwIGtleWludF9taW49MiBzY2VuZWN1dD00MCBpbnRyYV9yZWZyZXNoPTAgcmNfbG9va2FoZWFkPTQwIHJjPWNyZiBtYnRyZWU9MSBjcmY9NDAuMCBxY29tcD0wLjYwIHFwbWluPTAgcXBtYXg9NjkgcXBzdGVwPTQgaXBfcmF0aW89MS40MCBhcT0xOjEuMDAAgAAAABhliIQAFP/+hB/ApS8BHYUodOdxQHOCP8EAAAAIQZohbEEvFdA=",
  "base64",
);
const videoPolls = new Map();

function videoJob(res, jobId, host) {
  const polls = (videoPolls.get(jobId) ?? 0) + 1;
  videoPolls.set(jobId, polls);
  if (polls < 3) return sendJson(res, 200, { status: "pending", model: "grok-imagine-video-1.5", progress: polls * 35 });
  return sendJson(res, 200, {
    status: "done",
    model: "grok-imagine-video-1.5",
    progress: 100,
    video: { url: `http://${host}/files/${jobId}.mp4`, duration: 1, respect_moderation: true },
    usage: { cost_in_usd_ticks: 500000000, input_tokens: 12, output_tokens: 0, total_tokens: 12 },
  });
}

function transcription(res, raw) {
  const model = formField(raw, "model") ?? "";
  const format = formField(raw, "response_format") ?? "json";
  const text = "The parcel left the dock this morning, so tracking updates should appear soon.";
  if (format === "text") {
    res.writeHead(200, { "content-type": "text/plain", "x-request-id": id("req_") });
    return res.end(`${text}\n`);
  }
  const seconds = 4.2;
  const usage = model.startsWith("whisper")
    ? { type: "duration", seconds: Math.ceil(seconds) }
    : { type: "tokens", input_tokens: 70, input_token_details: { text_tokens: 0, audio_tokens: 70 }, output_tokens: 18, total_tokens: 88 };
  return format === "verbose_json"
    ? sendJson(res, 200, { task: "transcribe", language: "english", duration: seconds, text, segments: [], usage })
    : sendJson(res, 200, { text, usage });
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
  if (path === "/v1/audio/transcriptions" || path === "/v1/audio/translations") return transcription(res, raw);
  if (req.method === "POST" && path === "/v1/videos/generations") return sendJson(res, 200, { request_id: id("vid-") });
  if (req.method === "GET" && path.startsWith("/v1/videos/")) return videoJob(res, path.slice("/v1/videos/".length), req.headers.host);
  if (req.method === "GET" && path.startsWith("/files/") && path.endsWith(".mp4")) {
    res.writeHead(200, { "content-type": "video/mp4", "content-length": MP4.length });
    return res.end(MP4);
  }
  let body = {};
  if (raw.trimStart().startsWith("{") || raw === "") {
    try {
      body = JSON.parse(raw || "{}");
    } catch {
      return sendError(res, dialect, "invalid-request");
    }
  } else if (!path.startsWith("/v1/images/")) {
    return sendError(res, dialect, "invalid-request");
  }
  if (path === "/v1/images/generations" || path === "/v1/images/edits") return images(res, raw, body);
  if (path === "/v1/audio/speech") return speech(res, body);
  const failing = typeof body.model === "string" ? failureFor(raw, body.model) : undefined;
  if (failing !== undefined) return sendError(res, dialect, failing);
  const scripted = ["openai_embeddings", "decisions", "system_one", "gemini_decisions"].includes(dialect) ? undefined : scriptedReply(body);
  const shape = scripted === undefined ? shapeOf(raw) : scriptedShape(raw, body, scripted);
  if (path === "/v1/messages/count_tokens") return sendJson(res, 200, { input_tokens: shape.input });
  if (path === "/v1/responses/input_tokens") return sendJson(res, 200, { object: "response.input_tokens", input_tokens: shape.input });
  if (dialect === "decisions" || dialect === "system_one" || dialect === "gemini_decisions") return decisions(res, dialect, body, shape);
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
