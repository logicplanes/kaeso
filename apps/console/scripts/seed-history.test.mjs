import { describe, expect, it } from "vitest";
import { usageRows, usageSql } from "./seed-history.mjs";

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
