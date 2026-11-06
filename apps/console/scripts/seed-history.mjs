import { randomBytes } from "node:crypto";

const DAY = 86_400_000;
const MINUTE = 60_000;
const NIL = "00000000-0000-0000-0000-000000000000";

// Typical latency per model: time to the first token and tokens per second.
const MODELS = {
  "gpt-5.4-nano": { provider: "openai", ttfb: 260, tps: 190 },
  "gpt-5.4-mini": { provider: "openai", ttfb: 380, tps: 120 },
  "gpt-5.6": { provider: "openai", ttfb: 950, tps: 70 },
  "gpt-6-sol": { provider: "openai", ttfb: 1300, tps: 62 },
  "claude-haiku-4-5": { provider: "anthropic", ttfb: 470, tps: 105 },
  "claude-haiku-5-5": { provider: "anthropic", ttfb: 380, tps: 140 },
  "claude-sonnet-5-5": { provider: "anthropic", ttfb: 760, tps: 72 },
  "claude-opus-5-5": { provider: "anthropic", ttfb: 1350, tps: 44 },
  "text-embedding-3-small": { provider: "openai", ttfb: 150, embedding: true },
  "text-embedding-3-large": { provider: "openai", ttfb: 210, embedding: true },
};

const INGRESS = { "chat/completions": "openai_chat", messages: "anthropic_messages", responses: "openai_responses", embeddings: "openai_embeddings" };

const ERRORS = {
  overloaded: { anthropic: [529, "overloaded", "Overloaded"], openai: [503, "overloaded", "The engine is currently overloaded, please try again later."] },
  server: {
    anthropic: [500, "provider_error", "Internal server error"],
    openai: [500, "provider_error", "The server had an error while processing your request. Sorry about that!"],
  },
  tooLong: {
    anthropic: (tokens) => [400, "invalid_request", `prompt is too long: ${tokens} tokens > 200000 maximum`],
    openai: () => [400, "invalid_request", "Your input exceeds the context window of this model. Please adjust your input and try again."],
  },
  embeddingRate: (model, requested, wait) => [
    429,
    "rate_limited",
    `Rate limit reached for ${model} in organization org-nWx9Tq2c4kLmVb7RfYs3Ad1e on tokens per min (TPM): Limit 10000000, Used ${10_000_000 - requested + 1 + Math.floor(wait * 20)}, Requested ${requested}. Please try again in ${wait}ms. Visit https://platform.openai.com/account/rate-limits to learn more.`,
  ],
  projectRate: (limit, seconds) => [
    429,
    "rate_limited",
    `Rate limit reached: the project's request rate of ${limit} requests per minute; retry in ${seconds} seconds.`,
  ],
  disconnected: [499, "client_disconnected", "The app closed the connection before the reply was complete."],
};

function random(seed) {
  let state = seed || 1;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 4294967296;
  };
}

export async function clickhouse(url, query, body) {
  const target = new URL(url);
  const auth = Buffer.from(`${decodeURIComponent(target.username)}:${decodeURIComponent(target.password)}`).toString("base64");
  target.username = "";
  target.password = "";
  target.searchParams.set("query", query);
  const res = await fetch(target, { method: "POST", headers: { authorization: `Basic ${auth}` }, body });
  const text = await res.text();
  if (!res.ok) throw new Error(`ClickHouse: ${text.slice(0, 400)}`);
  return text;
}

function uuid7(ms) {
  const bytes = randomBytes(16);
  bytes.writeUIntBE(ms, 0, 6);
  bytes[6] = (bytes[6] & 0x0f) | 0x70;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

const timestamp = (ms) => new Date(ms).toISOString().replace("T", " ").replace("Z", "");
const BASE62 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const upstreamId = (next, provider) =>
  provider === "anthropic" ? `req_011C${Array.from({ length: 20 }, () => BASE62[Math.floor(next() * 62)]).join("")}` : `req_${randomBytes(16).toString("hex")}`;

function lognormal(next, median, sigma) {
  return median * Math.exp(sigma * Math.sqrt(-2 * Math.log(1 - next())) * Math.cos(2 * Math.PI * next()));
}

function weighted(next, entries) {
  const total = entries.reduce((sum, [, weight]) => sum + weight, 0);
  let roll = next() * total;
  for (const [value, weight] of entries) if ((roll -= weight) < 0) return value;
  return entries.at(-1)[0];
}

const pick = (next, list) => list[Math.floor(next() * list.length)];
const clamp = (value, low, high) => Math.min(high, Math.max(low, value));

// Requests per hour of the day (UTC) for each kind of caller, for a shop whose customers are in Europe.
const CURVES = {
  customers: [0.22, 0.15, 0.11, 0.09, 0.1, 0.16, 0.3, 0.5, 0.68, 0.78, 0.86, 0.95, 0.98, 0.9, 0.86, 0.88, 0.95, 1.08, 1.25, 1.32, 1.18, 0.92, 0.62, 0.38],
  office: [0, 0, 0, 0, 0, 0.02, 0.12, 0.55, 0.95, 1.1, 1.05, 0.8, 0.62, 0.95, 1.05, 0.92, 0.6, 0.28, 0.1, 0.06, 0.08, 0.05, 0.01, 0],
  inbox: [0.3, 0.2, 0.15, 0.12, 0.15, 0.3, 0.6, 0.95, 1.1, 1.05, 1.0, 0.95, 0.9, 0.95, 0.95, 0.9, 0.85, 0.8, 0.85, 0.85, 0.75, 0.6, 0.5, 0.4],
  night: [0.6, 1.4, 1.6, 1.2, 0.5, 0.2, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.15, 0.2, 0.3],
};

// A time in the day, drawn from the curve; null when the curve allows none before `until`.
function moment(next, dayStart, curve, until) {
  const weights = CURVES[curve];
  const open = weights.filter((_, hour) => dayStart + hour * 3_600_000 < until);
  const top = Math.max(0, ...open);
  if (top === 0) return null;
  for (;;) {
    const at = dayStart + Math.floor(next() * DAY);
    if (at >= until) continue;
    if (next() * top < weights[new Date(at).getUTCHours()]) return at;
  }
}

function cost(price, r) {
  const value = (r.input_tokens * price.input + r.cache_read_tokens * price.cached + r.cache_write_tokens * price.write + r.output_tokens * price.output) / 1e6;
  return value.toFixed(12);
}

// A request log row as the gateway writes it, from what the caller sent and how it went.
function logRow(next, ctx, req) {
  const model = MODELS[req.model];
  const caller = ctx.keys[req.key];
  const stream = Boolean(req.stream);
  const embedding = Boolean(model.embedding);
  const failed = req.error && req.error[0] !== 499;
  const output = failed ? 0 : req.error ? Math.round(req.output * (0.2 + next() * 0.5)) : req.output;
  const reasoning = failed ? 0 : Math.min(req.reasoning ?? 0, output);
  const hit = req.responseCache === "hit";
  let duration;
  let ttfb;
  if (hit) {
    duration = 1 + Math.floor(next() * 4);
    ttfb = 1;
  } else if (failed) {
    duration = Math.round(lognormal(next, req.refused ? 3 : 520, 0.6));
    ttfb = null;
  } else if (embedding) {
    duration = Math.round(lognormal(next, model.ttfb, 0.35) + req.input / 120);
    ttfb = Math.max(1, duration - 2 - Math.floor(next() * 6));
  } else {
    const prefill = (req.input + (req.cacheWrite ?? 0)) / 9 + (req.cacheRead ?? 0) / 60;
    const first = lognormal(next, model.ttfb, 0.3) + prefill + (reasoning / model.tps) * 1000;
    const tps = model.tps * lognormal(next, 1, 0.18);
    duration = Math.round(first + ((output - reasoning) / tps) * 1000 + (req.fallbackMs ?? 0));
    ttfb = stream ? Math.round(first + (req.fallbackMs ?? 0)) : Math.max(1, duration - 1 - Math.floor(next() * 12));
  }
  const egress = embedding ? "openai_embeddings" : model.provider === "openai" ? "openai_responses" : "anthropic_messages";
  const row = {
    request_id: uuid7(req.at),
    ...caller,
    started_at: timestamp(req.at),
    finished_at: timestamp(req.at + duration),
    duration_ms: duration,
    ttfb_ms: ttfb,
    endpoint: req.endpoint,
    ingress_dialect: INGRESS[req.endpoint],
    egress_dialect: egress,
    provider: model.provider,
    requested_model: req.alias ?? req.model,
    resolved_model: req.model,
    stream,
    status: req.error ? req.error[0] : 200,
    error_kind: req.error ? req.error[1] : "",
    error_message: req.error ? req.error[2] : "",
    input_tokens: failed ? 0 : req.input,
    output_tokens: output,
    cache_read_tokens: failed ? 0 : (req.cacheRead ?? 0),
    cache_write_tokens: failed ? 0 : (req.cacheWrite ?? 0),
    reasoning_tokens: reasoning,
    cost: "0",
    user_agent: req.ua,
    client_ip: req.ip,
    upstream_request_id: hit || req.refused ? "" : upstreamId(next, model.provider),
    has_content: false,
    logical_bytes: 0,
    expires_at: timestamp(req.at + 90 * DAY).slice(0, 19),
    stream_events: stream && !failed ? Math.round(output / (model.provider === "anthropic" ? 3.2 : 3.6)) + 6 : 0,
    stream_bytes: stream && !failed ? Math.round(output * 54 + 900) : 0,
    stream_first_byte_ms: stream && !failed ? ttfb + 2 + Math.floor(next() * 6) : null,
    stream_keepalives: stream && duration > 30_000 ? Math.floor(duration / 15_000) : 0,
    usage_estimated: req.error?.[0] === 499,
    source: "api",
    overhead_ms: hit ? 0 : Math.round(lognormal(next, 1.6, 0.5)),
    stream_processing_ms: stream && !failed ? Math.floor(next() * 3) : null,
    cache_breakpoints: model.provider === "anthropic" ? (req.breakpoints ?? 0) : null,
    request_upload_ms: req.input > 20_000 ? Math.round(req.input / 25_000 + next() * 4) : 0,
    response_cache: req.responseCache ?? "",
    alias: req.alias ?? "",
    fallbacks: req.fallbacks ?? "",
  };
  row.cost = hit ? "0.000000000000" : cost(ctx.prices(req.model), row);
  return row;
}

// The checkout assistant's aliases as the seed defines them, and where each one sends a caller.
function checkoutRoute(next, ctx, at, outage) {
  const alias = weighted(next, [
    ["fast", 40],
    ["support", 35],
    ["sturdy", 15],
    ["per-user", 10],
  ]);
  const down = outage && next() < 0.55;
  if (alias === "fast") return down ? { alias, model: "claude-haiku-5-5", error: ERRORS.overloaded.anthropic } : { alias, model: "claude-haiku-5-5" };
  if (alias === "sturdy" || (alias === "support" && next() < 0.12)) {
    if (!down) return { alias, model: "claude-sonnet-5-5" };
    return {
      alias,
      model: "gpt-6-sol",
      fallbacks: ctx.fallback(alias === "sturdy" ? "sturdy" : "support-then"),
      fallbackMs: Math.round(lognormal(next, 700, 0.4)),
    };
  }
  if (alias === "support") {
    const model = ctx.supportBranch;
    if (down && model === "claude-haiku-5-5")
      return { alias, model: "gpt-5.4-mini", fallbacks: ctx.fallback("support-else"), fallbackMs: Math.round(lognormal(next, 500, 0.4)) };
    return { alias, model };
  }
  const model = next() < 0.5 ? "claude-haiku-5-5" : "gpt-6-sol";
  return down && model === "claude-haiku-5-5" ? { alias, model, error: ERRORS.overloaded.anthropic } : { alias, model };
}

function baselineError(next, provider, stream) {
  const roll = next();
  if (provider === "anthropic" && roll < 0.0015) return ERRORS.overloaded.anthropic;
  if (roll < 0.002) return ERRORS.server[provider];
  if (provider === "openai" && roll < 0.0023) return ERRORS.overloaded.openai;
  if (stream && roll > 0.994) return ERRORS.disconnected;
  return null;
}

// Errors every provider has now and then, and Anthropic's overload for callers without a fallback.
function withBaseline(next, ctx, req) {
  if (req.error || req.responseCache === "hit") return req;
  if (!req.alias && MODELS[req.model].provider === "anthropic" && ctx.inOutage(req.at) && next() < 0.55) return { ...req, error: ERRORS.overloaded.anthropic };
  const error = baselineError(next, MODELS[req.model].provider, req.stream);
  return error ? { ...req, error } : req;
}

const CHECKOUT_IPS = ["203.0.113.10", "203.0.113.11", "203.0.113.12", "203.0.113.13"];
const OFFICE = "198.51.100.7";

// Every caller of the seeded organization: when it sends requests and what they look like.
function workloads(ctx) {
  // The assistant's system prompt and tools (cached when warm), then the conversation so far.
  const chat = (next, model, conversation, output) => {
    const anthropic = MODELS[model].provider === "anthropic";
    const prefix = anthropic ? 1850 : 1792;
    const warm = next();
    const cacheRead = anthropic ? (warm < 0.82 ? prefix : 0) : warm < 0.72 ? prefix : 0;
    const cacheWrite = anthropic && warm >= 0.82 && warm < 0.9 ? prefix : 0;
    const reasoning = model === "gpt-6-sol" ? Math.round(lognormal(next, 120, 0.6)) : 0;
    return { input: prefix + conversation - cacheRead - cacheWrite, cacheRead, cacheWrite, output: output + reasoning, reasoning };
  };
  return [
    {
      key: "checkout-web",
      perDay: 1300,
      weekend: 1.2,
      curve: "customers",
      make(next, at, day) {
        const route = checkoutRoute(next, ctx, at, ctx.inOutage(at));
        const tool = next() < 0.24;
        const shape = chat(
          next,
          route.model,
          Math.round(lognormal(next, 820, 0.65)),
          tool ? Math.round(lognormal(next, 38, 0.3)) : Math.round(lognormal(next, 170, 0.55)),
        );
        return {
          ...route,
          ...shape,
          endpoint: "chat/completions",
          stream: next() < 0.86,
          breakpoints: 1,
          ua: "OpenAI/JS 7.43.0",
          ip: pick(next, CHECKOUT_IPS),
          day,
        };
      },
    },
    {
      key: "checkout-worker",
      perDay: 640,
      weekend: 1,
      curve: "night",
      make(next) {
        if (next() < 0.74)
          return {
            model: "text-embedding-3-small",
            endpoint: "embeddings",
            input: Math.round(lognormal(next, 420, 0.8)),
            output: 0,
            ua: "OpenAI/Python 2.9.0",
            ip: pick(next, ["203.0.113.21", "203.0.113.22"]),
          };
        const prompt = Math.round(lognormal(next, 1150, 0.4));
        const cached = prompt > 1200 && next() < 0.8 ? 1024 : 0;
        return {
          model: "gpt-5.4-mini",
          endpoint: "responses",
          input: prompt - cached,
          cacheRead: cached,
          output: Math.round(lognormal(next, 140, 0.4)),
          ua: "OpenAI/Python 2.9.0",
          ip: pick(next, ["203.0.113.21", "203.0.113.22"]),
        };
      },
    },
    {
      key: "checkout-web",
      perDay: 0,
      flashSale: true,
    },
    {
      key: "ranking-service",
      perDay: 860,
      weekend: 1.15,
      curve: "customers",
      make(next) {
        if (next() < 0.7)
          return {
            model: "text-embedding-3-large",
            endpoint: "embeddings",
            input: Math.round(lognormal(next, 14, 0.5)),
            output: 0,
            ua: "OpenAI/Python 2.9.0",
            ip: pick(next, ["203.0.113.40", "203.0.113.41", "203.0.113.42", "203.0.113.43"]),
          };
        return {
          model: "gpt-5.4-nano",
          endpoint: "chat/completions",
          input: Math.round(lognormal(next, 230, 0.25)),
          output: Math.round(lognormal(next, 36, 0.4)),
          ua: "OpenAI/Python 2.9.0",
          ip: pick(next, ["203.0.113.40", "203.0.113.41", "203.0.113.42", "203.0.113.43"]),
        };
      },
    },
    {
      key: "slack-helper",
      perDay: 22,
      weekend: 0.12,
      curve: "office",
      content: "slack",
      make(next, at, day, ctx2) {
        const example = ctx2.exemplar("slack", next, day);
        return {
          ...example.shape,
          model: "claude-haiku-5-5",
          endpoint: "messages",
          stream: true,
          breakpoints: 0,
          ua: "ai-sdk/anthropic/2.0.41",
          ip: "203.0.113.50",
          content: example,
        };
      },
    },
  ];
}

function sessions(next, dayStart, until, count, length, gapMs) {
  const out = [];
  for (let s = 0; s < count; s++) {
    let at = moment(next, dayStart, "office", until);
    if (at === null) return out;
    const n = Math.max(3, Math.round(lognormal(next, length, 0.4)));
    for (let i = 0; i < n && at < until; i++) {
      out.push(at);
      at += Math.round(lognormal(next, gapMs, 0.7));
    }
  }
  return out;
}

// The Research agent project: Ada's notebook, Lin in Claude Code and Mateo in Codex, on weekdays.
function research(next, ctx, dayStart, until, d, weekday) {
  const requests = [];
  if (d > 45) return requests;
  const weekend = weekday === 0 || weekday === 6;
  const home = (at) => new Date(at).getUTCHours() >= 17 || weekend;
  if (!weekend || next() < 0.15) {
    for (const at of sessions(next, dayStart, until, weekend ? 1 : next() < 0.5 ? 2 : 3, 32, 45_000)) {
      const small = next() < 0.28;
      const model = small ? "claude-haiku-5-5" : "claude-opus-5-5";
      const context = small ? Math.round(lognormal(next, 6000, 0.5)) : Math.round(clamp(lognormal(next, 52_000, 0.55), 9000, 196_000));
      const write = next() < 0.09 ? Math.round(context * 0.12) : 0;
      const read = Math.round((context - write) * (small ? 0.6 : 0.93));
      const tooLong = !small && next() < 0.004;
      requests.push({
        key: "lin-claude-code",
        at,
        model,
        endpoint: "messages",
        stream: true,
        input: context - read - write,
        cacheRead: read,
        cacheWrite: write,
        output: Math.round(lognormal(next, small ? 60 : 420, 0.8)),
        breakpoints: small ? 1 : 4,
        ua: "claude-cli/2.1.31 (external, cli)",
        ip: home(at) ? "2001:db8:4a1:20::1f" : OFFICE,
        ...(tooLong ? { error: ERRORS.tooLong.anthropic(200_000 + Math.floor(next() * 14_000)) } : {}),
      });
    }
  }
  if (!weekend) {
    const sol = d <= 30 ? 0.82 : 0;
    for (const at of sessions(next, dayStart, until, next() < 0.6 ? 2 : 1, 26, 60_000)) {
      const model = next() < sol ? "gpt-6-sol" : "gpt-5.6";
      const context = Math.round(clamp(lognormal(next, 38_000, 0.5), 6000, 260_000));
      const cached = Math.floor((context * 0.86) / 128) * 128;
      const reasoning = Math.round(lognormal(next, model === "gpt-6-sol" ? 900 : 600, 0.8));
      requests.push({
        key: "mateo-codex",
        at,
        model,
        endpoint: "responses",
        stream: true,
        input: context - cached,
        cacheRead: cached,
        output: Math.round(lognormal(next, 380, 0.7)) + reasoning,
        reasoning,
        ua: "codex_cli_rs/0.59.0 (Mac OS 26.1.0; arm64) Ghostty/1.2.3",
        ip: home(at) ? "192.0.2.81" : OFFICE,
      });
    }
    const notebook = Math.round(lognormal(next, 11, 0.4));
    for (let i = 0; i < notebook; i++) {
      const at = moment(next, dayStart, "office", until);
      if (at === null) break;
      const context = Math.round(clamp(lognormal(next, 21_000, 0.6), 3000, 150_000));
      const read = next() < 0.7 ? Math.round(context * 0.8) : 0;
      requests.push({
        key: "research-notebook",
        at,
        model: "claude-opus-5-5",
        endpoint: "messages",
        stream: true,
        input: context - read,
        cacheRead: read,
        output: Math.round(lognormal(next, 1300, 0.6)),
        breakpoints: 2,
        ua: "Anthropic/Python 0.77.1",
        ip: home(at) ? "192.0.2.55" : OFFICE,
      });
    }
  }
  return requests;
}

// Support triage: each incoming email is classified, most get a drafted answer, and a retried
// webhook now and then sends the same email again and is answered from the response cache.
function triage(next, ctx, dayStart, until, d, weekday) {
  const requests = [];
  const monday = weekday === 1 ? 1.35 : 1;
  const count = Math.round((weekday === 0 || weekday === 6 ? 55 : 165) * monday * ctx.growth(d) * (0.9 + next() * 0.2));
  const migrated = clamp((40 - d) / 7, 0, 1);
  for (let i = 0; i < count; i++) {
    const at = moment(next, dayStart, "inbox", until);
    if (at === null) break;
    const t = ctx.ticket(next, d);
    const model = next() < migrated ? "claude-haiku-5-5" : "claude-haiku-4-5";
    const base = {
      key: "triage-bot",
      endpoint: "messages",
      stream: false,
      breakpoints: 1,
      ua: "Anthropic/Python 0.77.1",
      ip: "203.0.113.30",
      responseCache: "miss",
    };
    const classify = { ...base, at, model, ...t.classify.shape, content: t.classify };
    requests.push(classify);
    if (next() < 0.09) {
      const again = at + Math.round(lognormal(next, 8, 0.8) * MINUTE);
      if (again < until) requests.push({ ...classify, at: again, responseCache: "hit", hitOf: classify });
    }
    if (t.draft) {
      const later = at + 2000 + Math.floor(next() * 5000);
      if (later < until) requests.push({ ...base, at: later, model: "claude-sonnet-5-5", ...t.draft.shape, content: t.draft });
    }
  }
  return requests;
}

// CI runs the checkout assistant's smoke tests on every deploy, on weekdays.
function ci(next, ctx, dayStart, until, weekday) {
  const requests = [];
  if (weekday === 0 || weekday === 6) return requests;
  const deploys = 2 + Math.floor(next() * 5);
  for (let i = 0; i < deploys; i++) {
    let at = moment(next, dayStart, "office", until);
    if (at === null) break;
    for (const alias of ["fast", "sturdy", "support", "per-user", "fast", "sturdy", "support", "fast", "support", "sturdy", "fast", "per-user"]) {
      if (at >= until) break;
      const model =
        alias === "fast" ? "claude-haiku-5-5" : alias === "per-user" ? "claude-haiku-5-5" : alias === "support" ? ctx.supportBranch : "claude-sonnet-5-5";
      requests.push({
        key: "ci-smoke-tests",
        at,
        alias,
        model,
        endpoint: "chat/completions",
        stream: false,
        input: Math.round(lognormal(next, 160, 0.2)),
        output: Math.round(lognormal(next, 18, 0.3)),
        breakpoints: 1,
        ua: "OpenAI/Python 2.9.0",
        ip: pick(next, ["198.51.100.40", "198.51.100.41"]),
      });
      at += 400 + Math.floor(next() * 2500);
    }
  }
  return requests;
}

// Search ranking re-embeds the catalogue every Sunday night.
function reindex(next, dayStart, until, rateLimited) {
  const requests = [];
  const start = dayStart + 90 * MINUTE;
  for (let i = 0; i < 1150; i++) {
    const at = start + Math.floor(next() * 70 * MINUTE);
    if (at >= until) continue;
    const input = Math.round(clamp(lognormal(next, 6200, 0.35), 900, 8191 * 4));
    const limited = rateLimited && at - start > 20 * MINUTE && at - start < 45 * MINUTE && next() < 0.18;
    requests.push({
      key: "ranking-service",
      at,
      model: "text-embedding-3-large",
      endpoint: "embeddings",
      input,
      output: 0,
      ua: "OpenAI/Python 2.9.0",
      ip: "203.0.113.44",
      ...(limited ? { error: ERRORS.embeddingRate("text-embedding-3-large", input, 40 + Math.floor(next() * 400)) } : {}),
    });
  }
  return requests;
}

// The autumn sale: the newsletter goes out and checkout traffic runs into the project's request-rate limit.
function flashSale(next, ctx, dayStart, until) {
  const requests = [];
  const start = dayStart + 17 * 3_600_000;
  for (let i = 0; i < 2300; i++) {
    const offset = Math.floor(Math.min(lognormal(next, 22, 0.9), 150) * MINUTE + next() * MINUTE);
    const at = start + offset;
    if (at >= until) continue;
    const req = workloads(ctx)[0].make(next, at);
    requests.push({ ...req, key: "checkout-web", at });
  }
  requests.sort((a, b) => a.at - b.at);
  const perMinute = new Map();
  for (const req of requests) {
    const minute = Math.floor(req.at / MINUTE);
    const n = (perMinute.get(minute) ?? 0) + 1;
    perMinute.set(minute, n);
    if (n > ctx.requestRate) Object.assign(req, { error: ERRORS.projectRate(ctx.requestRate, 60 - Math.floor((req.at % MINUTE) / 1000)), refused: true });
  }
  return requests;
}

// How many days ago the first `weekday` (0 is Sunday) between `from` and `to` days ago was.
function weekdayNear(startOfToday, weekday, from, to) {
  for (let d = from; d <= to; d++) if (new Date(startOfToday - d * DAY).getUTCDay() === weekday) return d;
  return from;
}

// Writes `days` days of history up to ten minutes ago: request log rows, their content where the
// project keeps content, and the usage rollup the gateway keeps in PostgreSQL.
export async function writeHistory({
  clickhouseUrl,
  days,
  now = Date.now(),
  until = now - 10 * MINUTE,
  keys,
  prices,
  fallbacks,
  supportBranch,
  requestRate,
  exemplars,
  contentDays,
  recordUsage,
}) {
  const next = random(20261015);
  const startOfToday = Math.floor(now / DAY) * DAY;
  const dayOf = (ms) => Math.floor((startOfToday - Math.floor(ms / DAY) * DAY) / DAY);
  const outageDay = weekdayNear(startOfToday, 2, 8, 12);
  const saleDay = weekdayNear(startOfToday, 4, 11, 17);
  const reindexIncident = weekdayNear(startOfToday, 0, 20, 27);
  let ticketIndex = 0;
  const ctx = {
    keys,
    prices,
    supportBranch,
    requestRate,
    growth: (d) => 0.72 + 0.28 * (1 - d / days),
    fallback: (name) => fallbacks[name](Math.round(lognormal(next, 640, 0.4))),
    inOutage: (at) => dayOf(at) === outageDay && at % DAY >= (12 * 60 + 5) * MINUTE && at % DAY < (14 * 60 + 20) * MINUTE,
    ticket(nextFn, d) {
      const kept = d < contentDays;
      const list = exemplars.tickets;
      const t = kept && ticketIndex < list.length ? list[ticketIndex++] : list[Math.floor(nextFn() * list.length)];
      return { classify: { ...t.classify, kept }, draft: t.draft ? { ...t.draft, kept } : null };
    },
    exemplar(kind, nextFn, d) {
      const e = pick(nextFn, exemplars[kind]);
      return { ...e, kept: d < contentDays };
    },
  };
  const all = workloads(ctx);
  let written = 0;
  let contents = 0;
  for (let d = days; d >= 0; d--) {
    const dayStart = startOfToday - d * DAY;
    const weekday = new Date(dayStart).getUTCDay();
    const weekend = weekday === 0 || weekday === 6;
    const end = Math.min(dayStart + DAY, until);
    if (end <= dayStart) continue;
    const share = (end - dayStart) / DAY;
    const requests = [];
    for (const w of all) {
      if (w.flashSale) {
        if (d === saleDay) requests.push(...flashSale(next, ctx, dayStart, end));
        continue;
      }
      const count = Math.round(w.perDay * ctx.growth(d) * (weekend ? w.weekend : 1) * (0.9 + next() * 0.2) * share);
      for (let i = 0; i < count; i++) {
        const at = moment(next, dayStart, w.curve, end);
        if (at !== null) requests.push({ key: w.key, at, ...w.make(next, at, d, ctx) });
      }
    }
    requests.push(...triage(next, ctx, dayStart, end, d, weekday));
    requests.push(...research(next, ctx, dayStart, end, d, weekday));
    requests.push(...ci(next, ctx, dayStart, end, weekday));
    if (weekday === 0) requests.push(...reindex(next, dayStart, end, d === reindexIncident));
    requests.sort((a, b) => a.at - b.at);

    const rows = [];
    const content = [];
    const answered = new Map();
    for (const raw of requests) {
      const original = raw.hitOf ? answered.get(raw.hitOf) : null;
      if (raw.hitOf && !original) continue;
      const req = withBaseline(next, ctx, raw);
      const row = logRow(next, ctx, req);
      if (original) {
        row.input_tokens = original.row.input_tokens;
        row.cache_read_tokens = original.row.cache_read_tokens;
        row.output_tokens = original.row.output_tokens;
      }
      const reply =
        row.status !== 200
          ? ""
          : original
            ? original.reply
            : (req.content?.reply.replaceAll(req.content.replyId, `msg_${randomBytes(12).toString("hex")}`) ?? "");
      if (row.status === 200) answered.set(raw, { row, reply });
      if (req.content?.kept) {
        row.has_content = true;
        row.logical_bytes = Buffer.byteLength(req.content.request_body) + Buffer.byteLength(reply);
        content.push({
          request_id: row.request_id,
          org_id: row.org_id,
          project_id: row.project_id,
          started_at: row.started_at,
          request_body: req.content.request_body,
          reply,
          expires_at: timestamp(req.at + contentDays * DAY).slice(0, 19),
        });
      }
      rows.push(row);
    }
    if (rows.length) {
      await clickhouse(clickhouseUrl, "INSERT INTO request_log FORMAT JSONEachRow", rows.map((r) => JSON.stringify(r)).join("\n"));
      if (content.length) await clickhouse(clickhouseUrl, "INSERT INTO request_content FORMAT JSONEachRow", content.map((r) => JSON.stringify(r)).join("\n"));
      recordUsage(usageSql(usageRows(rows)));
    }
    written += rows.length;
    contents += content.length;
  }
  return { written, contents, outageDay, saleDay, reindexIncident };
}

export function usageRows(rows) {
  const groups = new Map();
  for (const r of rows) {
    const day = r.started_at.slice(0, 10);
    const member =
      r.principal_id === NIL
        ? { user_id: null, service_account_id: null }
        : r.principal_kind === "user"
          ? { user_id: r.principal_id, service_account_id: null }
          : { user_id: null, service_account_id: r.principal_id };
    const row = {
      day,
      org_id: r.org_id,
      project_id: r.project_id,
      ...member,
      api_key_id: r.key_id === NIL ? null : r.key_id,
      model_id: r.resolved_model,
      provider: r.provider,
      ingress_dialect: r.ingress_dialect,
      egress_dialect: r.egress_dialect,
    };
    const key = JSON.stringify(row);
    const sum = groups.get(key) ?? {
      ...row,
      requests: 0,
      errors: 0,
      input_tokens: 0,
      output_tokens: 0,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      reasoning_tokens: 0,
      cost: 0,
    };
    sum.requests += 1;
    sum.errors += r.status >= 400 ? 1 : 0;
    sum.input_tokens += r.input_tokens;
    sum.output_tokens += r.output_tokens;
    sum.cache_read_tokens += r.cache_read_tokens;
    sum.cache_write_tokens += r.cache_write_tokens;
    sum.reasoning_tokens += r.reasoning_tokens;
    sum.cost += Number(r.cost);
    groups.set(key, sum);
  }
  return [...groups.values()];
}

export function usageSql(rows) {
  const json = JSON.stringify(rows.map((r) => ({ ...r, cost: r.cost.toFixed(9) }))).replaceAll("'", "''");
  return `INSERT INTO usage_daily AS u (day, org_id, project_id, user_id, service_account_id, api_key_id, model_id, provider, ingress_dialect, egress_dialect,
  requests, errors, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, cost)
SELECT day, org_id, project_id, user_id, service_account_id, api_key_id, model_id, provider, ingress_dialect, egress_dialect,
  requests, errors, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, cost
FROM jsonb_to_recordset('${json}'::jsonb) AS r(day date, org_id uuid, project_id uuid, user_id uuid, service_account_id uuid, api_key_id uuid,
  model_id text, provider text, ingress_dialect text, egress_dialect text, requests bigint, errors bigint, input_tokens bigint, output_tokens bigint,
  cache_read_tokens bigint, cache_write_tokens bigint, reasoning_tokens bigint, cost numeric)
ON CONFLICT (project_id, day, api_key_id, user_id, service_account_id, provider, model_id, ingress_dialect, egress_dialect) DO UPDATE SET
  requests = u.requests + excluded.requests, errors = u.errors + excluded.errors, input_tokens = u.input_tokens + excluded.input_tokens,
  output_tokens = u.output_tokens + excluded.output_tokens, cache_read_tokens = u.cache_read_tokens + excluded.cache_read_tokens,
  cache_write_tokens = u.cache_write_tokens + excluded.cache_write_tokens, reasoning_tokens = u.reasoning_tokens + excluded.reasoning_tokens,
  cost = u.cost + excluded.cost;
`;
}
