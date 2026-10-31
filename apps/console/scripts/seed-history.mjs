import { randomBytes } from "node:crypto";

const DAY = 86_400_000;
const AGENTS = ["OpenAI/Python 2.9.0", "OpenAI/JS 7.32.0", "Anthropic/Python 0.77.1", "Anthropic/JS 0.133.0", "langchain-core/0.3.79", "checkout-web/3.4.2"];
const FAILURES = [
  ["rate_limited", 429, "Rate limit reached: this key's request rate of 60 requests per minute; retry in 12 seconds.", 14],
  ["overloaded", 529, "Overloaded", 6],
  ["provider_error", 500, "Internal server error", 4],
  ["invalid_request", 400, "messages: text content blocks must be non-empty", 5],
  ["client_disconnected", 499, "The app closed the connection before the reply was complete.", 4],
];

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

function lognormal(next, median, sigma) {
  return median * Math.exp(sigma * Math.sqrt(-2 * Math.log(1 - next())) * Math.cos(2 * Math.PI * next()));
}

function weighted(next, entries) {
  const total = entries.reduce((sum, [, weight]) => sum + weight, 0);
  let roll = next() * total;
  for (const [value, weight] of entries) if ((roll -= weight) < 0) return value;
  return entries.at(-1)[0];
}

function historyRow(next, template, price, at) {
  const failure =
    next() < 0.035
      ? weighted(
          next,
          FAILURES.map((f) => [f, f[3]]),
        )
      : null;
  const embedding = template.endpoint === "embeddings";
  const input = Math.round(lognormal(next, embedding ? 240 : 1400, 0.8));
  const cached = !embedding && input > 1024 && next() < 0.4 ? Math.round(input * (0.4 + next() * 0.5)) : 0;
  const reasoning = !embedding && next() < 0.25 ? Math.round(lognormal(next, 420, 0.7)) : 0;
  const output = embedding ? 0 : Math.round(lognormal(next, 320, 0.7)) + reasoning;
  const usage = failure && failure[0] !== "client_disconnected" ? 0 : failure ? next() : 1;
  const tokens = { input: Math.round((input - cached) * Math.ceil(usage)), cached: Math.ceil(usage) ? cached : 0, output: Math.round(output * usage) };
  const ttfb = Math.round(lognormal(next, /opus|sol|5\.6/.test(template.resolved_model) ? 1200 : 420, 0.5));
  const duration = failure && usage === 0 ? Math.round(lognormal(next, 8, 0.8)) : Math.round(ttfb + tokens.output * (1000 / 90));
  const stream = !embedding && next() < 0.55;
  const cost = (tokens.input * price.input + tokens.cached * price.cached + tokens.output * price.output) / 1e6;
  return {
    ...template,
    request_id: uuid7(at),
    started_at: timestamp(at),
    finished_at: timestamp(at + duration),
    duration_ms: duration,
    ttfb_ms: failure && usage === 0 ? null : ttfb,
    stream,
    status: failure ? failure[1] : 200,
    error_kind: failure ? failure[0] : "",
    error_message: failure ? failure[2] : "",
    input_tokens: tokens.input,
    cache_read_tokens: tokens.cached,
    cache_write_tokens: 0,
    output_tokens: tokens.output,
    reasoning_tokens: Math.round(reasoning * usage),
    cost: cost.toFixed(12),
    user_agent: template.user_agent || AGENTS[Math.floor(next() * AGENTS.length)],
    client_ip: `203.0.113.${1 + Math.floor(next() * 200)}`,
    upstream_request_id: failure && usage === 0 ? "" : `req_${randomBytes(12).toString("hex")}`,
    has_content: false,
    logical_bytes: 0,
    stream_events: stream && usage > 0 ? Math.round(tokens.output / 4) + 4 : 0,
    stream_bytes: stream && usage > 0 ? tokens.output * 18 + 400 : 0,
    stream_first_byte_ms: stream && usage > 0 ? ttfb + 12 : null,
    stream_keepalives: 0,
    usage_estimated: usage > 0 && usage < 1,
    source: "api",
  };
}

const TEMPLATE_COLUMNS = [
  "org_id",
  "project_id",
  "key_id",
  "principal_kind",
  "principal_id",
  "endpoint",
  "ingress_dialect",
  "egress_dialect",
  "provider",
  "requested_model",
  "resolved_model",
  "user_agent",
  "expires_at",
];

const NIL = "00000000-0000-0000-0000-000000000000";

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

export async function writeHistory({ clickhouseUrl, days, perDay, prices, weights, recordUsage }) {
  const templates = (
    await clickhouse(
      clickhouseUrl,
      `SELECT ${TEMPLATE_COLUMNS.join(", ")} FROM request_log WHERE status = 200 AND source = 'api' LIMIT 1 BY project_id, key_id, resolved_model, endpoint FORMAT JSONEachRow`,
    )
  )
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  if (!templates.length) throw new Error("seed: no requests to model the history on");
  const next = random(20261015);
  const today = new Date();
  const startOfToday = Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate());
  const choices = templates.map((t) => [t, weights(t)]);
  let written = 0;
  const until = (Date.now() - startOfToday - 600_000) / 3_600_000;
  for (let d = days; d >= 0; d--) {
    const dayStart = startOfToday - d * DAY;
    const weekday = new Date(dayStart).getUTCDay();
    const weekend = weekday === 0 || weekday === 6;
    const hours = d === 0 ? Math.max(0, until) : 24;
    const count = Math.round(perDay * (0.6 + (0.6 * (days - d)) / days) * (weekend ? 0.4 : 1) * (0.88 + next() * 0.24) * (hours / 24));
    const rows = [];
    for (let i = 0; i < count; i++) {
      let hour;
      do hour = next() * hours;
      while (next() > 0.3 + 0.7 * Math.exp(-((hour - 14) ** 2) / 24));
      const template = weighted(next, choices);
      const at = dayStart + Math.floor(hour * 3_600_000);
      rows.push(JSON.stringify(historyRow(next, { ...template, expires_at: timestamp(at + 90 * DAY).slice(0, 19) }, prices(template.resolved_model), at)));
    }
    if (rows.length) {
      await clickhouse(clickhouseUrl, "INSERT INTO request_log FORMAT JSONEachRow", rows.join("\n"));
      recordUsage(usageSql(usageRows(rows.map((r) => JSON.parse(r)))));
    }
    written += rows.length;
  }
  return written;
}
