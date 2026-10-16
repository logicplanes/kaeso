export function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

export const NOW = new Date("2026-10-12T14:20:00Z");

export const org = { id: "org_01J9Z4", name: "Northwind Labs" };
export const projects = [
  { id: "proj_01J9Z5A", name: "Checkout assistant", keys: 6, models: 4 },
  { id: "proj_01J9Z5B", name: "Support triage", keys: 3, models: 2 },
  { id: "proj_01J9Z5C", name: "Internal tools", keys: 2, models: 5 },
];

export const members = [
  { id: "usr_ada", name: "Ada Okafor", email: "ada@northwind.example", preset: "Owner" },
  { id: "usr_lin", name: "Lin Park", email: "lin@northwind.example", preset: "Admin" },
  { id: "usr_tomas", name: "Tomás Ruiz", email: "tomas@northwind.example", preset: "Developer" },
  { id: "usr_mira", name: "Mira Sato", email: "mira@northwind.example", preset: "Developer" },
  { id: "usr_jon", name: "Jon Weber", email: "jon@northwind.example", preset: "Viewer" },
];

export type ModelRow = {
  id: string;
  provider: "openai" | "anthropic";
  model: string;
  name: string;
  input: string;
  output: string;
  cacheRead: string;
  context: number;
  capabilities: ("tools" | "reasoning" | "vision" | "pdf" | "structuredOutput")[];
};

export const models: ModelRow[] = [
  {
    id: "model_01",
    provider: "openai",
    model: "gpt-5",
    name: "GPT-5",
    input: "1.25",
    output: "10.00",
    cacheRead: "0.125",
    context: 400000,
    capabilities: ["tools", "reasoning", "vision", "structuredOutput"],
  },
  {
    id: "model_02",
    provider: "openai",
    model: "gpt-5-mini",
    name: "GPT-5 mini",
    input: "0.25",
    output: "2.00",
    cacheRead: "0.025",
    context: 400000,
    capabilities: ["tools", "reasoning", "vision", "structuredOutput"],
  },
  {
    id: "model_03",
    provider: "anthropic",
    model: "claude-sonnet-4-5",
    name: "Claude Sonnet 4.5",
    input: "3.00",
    output: "15.00",
    cacheRead: "0.30",
    context: 200000,
    capabilities: ["tools", "reasoning", "vision", "pdf"],
  },
  {
    id: "model_04",
    provider: "anthropic",
    model: "claude-haiku-4-5",
    name: "Claude Haiku 4.5",
    input: "1.00",
    output: "5.00",
    cacheRead: "0.10",
    context: 200000,
    capabilities: ["tools", "vision", "pdf"],
  },
  {
    id: "model_05",
    provider: "openai",
    model: "gpt-4.1-nano",
    name: "GPT-4.1 nano",
    input: "0.10",
    output: "0.40",
    cacheRead: "0.025",
    context: 1047576,
    capabilities: ["tools", "vision", "structuredOutput"],
  },
];

export type KeyRow = {
  id: string;
  name: string;
  redacted: string;
  state: "active" | "disabled" | "expired" | "archived";
  access: "inference" | "platform";
  project: string;
  owner: string;
  lastUsed: string;
  expires?: string;
};

export const keys: KeyRow[] = [
  {
    id: "key_01JA1",
    name: "checkout-web",
    redacted: "sk-kso-AbC123…wxyz",
    state: "active",
    access: "inference",
    project: "Checkout assistant",
    owner: "Ada Okafor",
    lastUsed: "2026-10-12T14:12:00Z",
    expires: "2027-01-01",
  },
  {
    id: "key_01JA2",
    name: "checkout-worker",
    redacted: "sk-kso-Xm91Lq…4f2a",
    state: "active",
    access: "inference",
    project: "Checkout assistant",
    owner: "Lin Park",
    lastUsed: "2026-10-12T13:58:00Z",
  },
  {
    id: "key_01JA3",
    name: "ci-runner",
    redacted: "sk-kso-Pz0Tt7…91cd",
    state: "active",
    access: "platform",
    project: "Checkout assistant",
    owner: "Tomás Ruiz",
    lastUsed: "2026-10-11T22:04:00Z",
  },
  {
    id: "key_01JA4",
    name: "staging",
    redacted: "sk-kso-Hq55Ra…0b7e",
    state: "disabled",
    access: "inference",
    project: "Checkout assistant",
    owner: "Mira Sato",
    lastUsed: "2026-10-02T09:40:00Z",
  },
  {
    id: "key_01JA5",
    name: "load-test",
    redacted: "sk-kso-Ty8Kc2…aa31",
    state: "expired",
    access: "inference",
    project: "Checkout assistant",
    owner: "Tomás Ruiz",
    lastUsed: "2026-09-18T16:21:00Z",
    expires: "2026-09-30",
  },
  {
    id: "key_01JA6",
    name: "prototype",
    redacted: "sk-kso-Ld3Nn0…5e60",
    state: "archived",
    access: "inference",
    project: "Checkout assistant",
    owner: "Ada Okafor",
    lastUsed: "2026-08-01T11:00:00Z",
  },
];

export const SECRET = "sk-kso-AbC123r7Yq2LmV9xTzWfK4pHdNs8gJcU6bE1oQiwxyz";

export type LogRow = {
  id: string;
  time: string;
  status: number;
  method: "POST";
  endpoint: string;
  model: string;
  provider: "openai" | "anthropic";
  key: string;
  latencyMs: number;
  ttfbMs: number;
  inputTokens: number;
  outputTokens: number;
  cost: string;
  streamed: boolean;
};

export function makeLogs(count = 200): LogRow[] {
  const r = rng(42);
  const endpoints = ["/v1/responses", "/v1/chat/completions", "/v1/messages"];
  const rows: LogRow[] = [];
  let t = NOW.getTime();
  for (let i = 0; i < count; i++) {
    t -= Math.round(4000 + r() * 90000);
    const m = models[Math.floor(r() * 4)]!;
    const ok = r();
    const status = ok > 0.94 ? (ok > 0.985 ? 502 : 429) : ok > 0.92 ? 400 : 200;
    const input = Math.round(400 + r() * 6000);
    const output = status === 200 ? Math.round(80 + r() * 1400) : 0;
    const cost = ((input * Number(m.input) + output * Number(m.output)) / 1_000_000).toFixed(6);
    const latency = Math.round(status === 200 ? 600 + r() * 5200 : 80 + r() * 400);
    rows.push({
      id: `req_01JAR${(100000 + i * 7919).toString(36).toUpperCase()}`,
      time: new Date(t).toISOString(),
      status,
      method: "POST",
      endpoint: m.provider === "anthropic" ? "/v1/messages" : endpoints[Math.floor(r() * 2)]!,
      model: m.model,
      provider: m.provider,
      key: keys[Math.floor(r() * 3)]!.name,
      latencyMs: latency,
      ttfbMs: Math.round(latency * (0.18 + r() * 0.3)),
      inputTokens: input,
      outputTokens: output,
      cost,
      streamed: r() > 0.4,
    });
  }
  return rows;
}

export function usageSeries(days = 30) {
  const r = rng(7);
  return models.slice(0, 4).map((m, mi) => ({
    id: m.model,
    label: m.name,
    points: Array.from({ length: days }, (_, d) => ({
      t: new Date(NOW.getTime() - (days - 1 - d) * 86_400_000),
      v: Math.max(0, Math.round((4 + mi * 2.2 + Math.sin((d + mi) / 3) * 2 + r() * 3) * 100) / 100),
    })),
  }));
}

export const tokenTotals = { input: 18_420_000, cacheRead: 6_310_000, cacheWrite: 1_240_000, output: 4_870_000, reasoning: 1_920_000 };

export const auditEvents = [
  { id: "a1", title: "Ada Okafor created the key checkout-web", time: "14:05", tone: "success" as const },
  { id: "a2", title: "Lin Park changed the monthly budget of Checkout assistant to $500.00", time: "13:41", tone: "info" as const },
  { id: "a3", title: "Tomás Ruiz added the model claude-haiku-4-5", time: "12:17", tone: "neutral" as const },
  { id: "a4", title: "Mira Sato disabled the key staging", time: "09:02", tone: "warning" as const },
  { id: "a5", title: "Jon Weber was removed from Support triage", time: "Yesterday", tone: "danger" as const },
];

export const conversation = [
  { role: "system" as const, content: "You help customers of Northwind finish checkout. Be brief and precise." },
  { role: "user" as const, content: "Can I still change the delivery address on order 4471?" },
  {
    role: "assistant" as const,
    content: [
      { type: "reasoning" as const, text: "The customer asks about an existing order. I should look it up before answering." },
      { type: "tool_call" as const, id: "call_9f2", name: "get_order", arguments: { order_id: "4471", fields: ["status", "shipping_address"] } },
    ],
  },
  {
    role: "tool" as const,
    content: [{ type: "tool_result" as const, tool_call_id: "call_9f2", content: '{"status":"packed","shipping_address":"Hauptstraße 12, 10115 Berlin"}' }],
  },
  { role: "assistant" as const, content: "Yes. Order 4471 is packed but has not shipped, so the address can still change. What is the new address?" },
];
