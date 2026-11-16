import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import type { TimelineBucket } from "~/api/generated/types";
import { CLIENT_IDS, clientLabel, clientUsage } from "./clients";

const bucket = (key: string | null, requests: number, cost: string | null): TimelineBucket => ({
  start: "2026-10-14T10:00:00Z",
  key,
  requests,
  errors: 1,
  input_tokens: 10,
  output_tokens: 5,
  cache_read_tokens: 0,
  cache_write_tokens: 0,
  reasoning_tokens: 0,
  cost,
  duration_p50_ms: 100,
  duration_p95_ms: 200,
  duration_max_ms: 300,
});

describe("client apps", () => {
  it("names every client the gateway's filter takes", () => {
    const doc = JSON.parse(readFileSync(join(__dirname, "../../../../openapi.json"), "utf8")) as {
      components: { schemas: { ClientApp: { enum: string[] } } };
    };
    expect([...CLIENT_IDS]).toEqual(doc.components.schemas.ClientApp.enum);
    expect(CLIENT_IDS.map(clientLabel)).toEqual([
      "Codex",
      "Claude Code",
      "OpenAI Python SDK",
      "OpenAI Node.js SDK",
      "Anthropic Python SDK",
      "Anthropic Node.js SDK",
      "AI SDK",
      "curl",
      "Other client",
    ]);
    expect([clientLabel(""), clientLabel("vim")]).toEqual(["No user agent", "vim"]);
  });

  it("sums the hourly timeline into a row per client", () => {
    const rows = clientUsage([bucket("codex", 2, "0.5"), bucket("codex", 3, null), bucket(null, 1, null), bucket("", 4, null)]);
    expect(rows.map((r) => [r.key, r.requests, r.errors, r.input_tokens, r.cost])).toEqual([
      ["codex", 5, 2, 20, "0.5"],
      ["", 5, 2, 20, null],
    ]);
  });
});
