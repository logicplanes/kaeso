import { beforeAll, describe, expect, it, vi } from "vitest";
import { usageRows, usageSql, writeHistory } from "./seed-history.mjs";

const NIL = "00000000-0000-0000-0000-000000000000";
const base = {
  org_id: "0190f1c2-0000-7000-8000-000000000001",
  project_id: "0190f1c2-0000-7000-8000-000000000002",
  key_id: "0190f1c2-0000-7000-8000-000000000003",
  principal_kind: "user",
  principal_id: "0190f1c2-0000-7000-8000-000000000004",
  provider: "openai",
  resolved_model: "gpt-5.4-mini",
  ingress_dialect: "openai_chat",
  egress_dialect: "openai_responses",
  status: 200,
  input_tokens: 10,
  output_tokens: 5,
  cache_read_tokens: 2,
  cache_write_tokens: 0,
  reasoning_tokens: 1,
  cost: "0.000100000000",
};

describe("the seed's usage history", () => {
  it("adds up a day's requests per key, member, model and route", () => {
    const rows = usageRows([
      { ...base, started_at: "2026-10-01 09:00:00.000" },
      { ...base, started_at: "2026-10-01 18:30:00.000", status: 502, cost: "0.000050000000" },
      { ...base, started_at: "2026-10-02 08:00:00.000" },
      { ...base, started_at: "2026-10-01 10:00:00.000", ingress_dialect: "anthropic_messages" },
    ]);
    expect(rows).toHaveLength(3);
    expect(rows[0]).toMatchObject({
      day: "2026-10-01",
      user_id: base.principal_id,
      service_account_id: null,
      api_key_id: base.key_id,
      model_id: "gpt-5.4-mini",
      requests: 2,
      errors: 1,
      input_tokens: 20,
      reasoning_tokens: 2,
    });
    expect(rows[0].cost).toBeCloseTo(0.00015, 9);
  });

  it("files service accounts and requests without a key as the gateway does", () => {
    const [row] = usageRows([{ ...base, started_at: "2026-10-01 09:00:00.000", principal_kind: "service_account", key_id: NIL }]);
    expect(row).toMatchObject({ user_id: null, service_account_id: base.principal_id, api_key_id: null });
  });

  it("adds to the rows the gateway already wrote", () => {
    const sql = usageSql(usageRows([{ ...base, started_at: "2026-10-01 09:00:00.000", resolved_model: "it's" }]));
    expect(sql).toContain("ON CONFLICT (project_id, day, api_key_id, user_id, service_account_id, provider, model_id, ingress_dialect, egress_dialect)");
    expect(sql).toContain("requests = u.requests + excluded.requests");
    expect(sql).toContain("it''s");
    expect(sql).toContain('"cost":"0.000100000"');
  });
});

describe("the seed's request history", () => {
  const DAY = 86_400_000;
  const now = Date.UTC(2026, 10, 6, 12);
  const names = [
    "checkout-web",
    "checkout-worker",
    "ci-smoke-tests",
    "triage-bot",
    "ranking-service",
    "research-notebook",
    "slack-helper",
    "lin-claude-code",
    "mateo-codex",
  ];
  const keys = Object.fromEntries(
    names.map((name, i) => [
      name,
      {
        org_id: base.org_id,
        project_id: `0190f1c2-0000-7000-8000-0000000001${i}0`,
        key_id: `0190f1c2-0000-7000-8000-0000000002${i}0`,
        principal_kind: "user",
        principal_id: base.principal_id,
      },
    ]),
  );
  const exemplar = (n) => ({
    request_body: `{"n":${n}}`,
    reply: `{"id":"msg_${n}","blocks":[]}`,
    replyId: `msg_${n}`,
    shape: { input: 120 + n, cacheRead: 350, output: 70 },
  });
  const step = (model) => (ms) =>
    JSON.stringify([
      { catalogue_model_id: base.key_id, path: "route", model, status: 529, error_kind: "overloaded", error_message: "Overloaded", duration_ms: ms },
    ]);

  async function history(days) {
    const inserts = { request_log: [], request_content: [] };
    const usage = [];
    const fetch = vi.fn(async (url, init) => {
      const table = /INSERT INTO (\w+)/.exec(new URL(url).searchParams.get("query"))?.[1];
      if (table) inserts[table].push(...init.body.split("\n").map((line) => JSON.parse(line)));
      return new Response("");
    });
    vi.stubGlobal("fetch", fetch);
    const result = await writeHistory({
      clickhouseUrl: "http://kaeso:kaeso@clickhouse/?database=kaeso",
      days,
      now,
      until: Math.floor(now / DAY) * DAY,
      keys,
      prices: () => ({ input: 1, cached: 0.1, write: 1.25, output: 5 }),
      fallbacks: {
        sturdy: step("anthropic/claude-sonnet-5-5"),
        "support-then": step("anthropic/claude-sonnet-5-5"),
        "support-else": step("anthropic/claude-haiku-5-5"),
      },
      supportBranch: "claude-haiku-5-5",
      requestRate: 60,
      exemplars: {
        tickets: Array.from({ length: 40 }, (_, i) => ({ classify: exemplar(i), draft: i % 3 ? exemplar(100 + i) : null })),
        slack: [exemplar(500)],
      },
      contentDays: 30,
      recordUsage: (sql) => usage.push(sql),
    });
    vi.unstubAllGlobals();
    return { ...result, ...inserts, usage };
  }

  let h;
  beforeAll(async () => {
    h = await history(30);
  }, 60_000);
  const day = (row) => Math.floor(now / DAY) - Math.floor(Date.parse(`${row.started_at}Z`) / DAY);

  it("ends with yesterday and keeps every caller busy", () => {
    expect(h.written).toBe(h.request_log.length);
    expect(Math.max(...h.request_log.map((r) => Date.parse(`${r.started_at}Z`)))).toBeLessThan(Math.floor(now / DAY) * DAY);
    expect(new Set(h.request_log.map((r) => r.key_id))).toEqual(new Set(Object.values(keys).map((k) => k.key_id)));
    expect(h.usage).toHaveLength(30);
  });

  it("keeps content only where the project logs it and only for the retention", () => {
    const ids = new Set(h.request_content.map((c) => c.request_id));
    expect(h.contents).toBe(h.request_content.length);
    for (const row of h.request_log) expect(row.has_content).toBe(ids.has(row.request_id));
    for (const row of h.request_log.filter((r) => r.has_content)) expect(day(row)).toBeLessThan(30);
    expect(new Set(h.request_log.filter((r) => r.has_content).map((r) => r.user_agent))).toEqual(
      new Set(["Anthropic/Python 0.77.1", "ai-sdk/anthropic/2.0.41"]),
    );
  });

  it("answers repeated support emails from the response cache, free and with the first answer", () => {
    const hits = h.request_log.filter((r) => r.response_cache === "hit");
    expect(hits.length).toBeGreaterThan(50);
    const content = new Map(h.request_content.map((c) => [c.request_id, c]));
    for (const hit of hits) {
      expect(hit).toMatchObject({ cost: "0.000000000000", upstream_request_id: "", status: 200 });
      expect(hit.duration_ms).toBeLessThan(5);
      if (hit.has_content) expect(content.get(hit.request_id).reply).not.toBe("");
    }
  });

  it("falls back to OpenAI while Anthropic is overloaded and refuses past the checkout's request rate", () => {
    const fellBack = h.request_log.filter((r) => r.fallbacks);
    expect(fellBack.length).toBeGreaterThan(5);
    expect(new Set(fellBack.map(day))).toEqual(new Set([h.outageDay]));
    for (const row of fellBack) expect(row.provider).toBe("openai");
    const refused = h.request_log.filter((r) => r.error_message.startsWith("Rate limit reached: the project's request rate of 60"));
    expect(refused.length).toBeGreaterThan(0);
    expect(new Set(refused.map(day))).toEqual(new Set([h.saleDay]));
    for (const row of refused) expect(row).toMatchObject({ upstream_request_id: "", input_tokens: 0, cost: "0.000000000000" });
  });

  it("adds up to the same usage the request log shows", () => {
    const rows = usageRows(h.request_log);
    expect(rows.reduce((n, r) => n + r.requests, 0)).toBe(h.request_log.length);
    expect(rows.reduce((n, r) => n + r.errors, 0)).toBe(h.request_log.filter((r) => r.status >= 400).length);
  });
});
