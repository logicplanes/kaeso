import { describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { exportRequest, exportSummary } from "./export";

describe("exportSummary", () => {
  it("names the period alone when nothing else filters", () => {
    expect(exportSummary({})).toBe("Every request from the last 24 hours, newest first.");
    expect(exportSummary({ range: "1h" })).toBe("Every request from the last hour, newest first.");
    expect(exportSummary({ from: "2026-08-14", to: "2026-08-14" })).toBe("Every request on 14 Aug 2026, newest first.");
    expect(exportSummary({ from: "2026-08-14", to: "2026-10-13" })).toBe("Every request from 14 Aug 2026 to 13 Oct 2026, newest first.");
  });

  it("says where the requests came from", () => {
    expect(exportSummary({ source: "playground" })).toBe("Every playground request from the last 24 hours, newest first.");
    expect(exportSummary({ source: "api", failed: true })).toBe("Every failed API request from the last 24 hours, newest first.");
  });

  it("says every filter, with the names people know", () => {
    expect(
      exportSummary(
        {
          range: "7d",
          failed: true,
          stream: true,
          key: "key_1",
          member: "user:usr_1",
          model: "gpt-5.4-mini",
          provider: "openai",
          status: 429,
          error: "rate_limited",
        },
        { key: "checkout-web", member: "Dana Ito" },
      ),
    ).toBe(
      "Every failed streamed request from the last 7 days by Dana Ito with key checkout-web, model gpt-5.4-mini, provider OpenAI, status 429 and error kind rate limited, newest first.",
    );
    expect(exportSummary({ range: "30d", stream: false, status: 200 })).toBe(
      "Every request from the last 30 days with status 200, not streamed, newest first.",
    );
  });

  it("falls back to the ids when it has no names", () => {
    expect(exportSummary({ key: "key_1", member: "sa:sa_deploy", model: "gpt-5.4-mini" })).toBe(
      "Every request from the last 24 hours by sa_deploy with key key_1 and model gpt-5.4-mini, newest first.",
    );
  });

  it("keeps the error kind's capital letter outside English", async () => {
    apply(await load("de"));
    expect(exportSummary({ error: "rate_limited" })).toContain("Fehlerart ");
    expect(exportSummary({ error: "rate_limited" })).not.toContain("Fehlerart rate");
  });
});

describe("exportRequest", () => {
  const now = Date.parse("2026-10-13T12:00:00Z");

  it("ends a relative window at the moment of the export", () => {
    const r = exportRequest("org_1", "proj_1", { range: "24h", status: 429 }, "csv", now);
    const url = new URL(r.url, "http://x");
    expect(url.pathname).toBe("/platform/v1/orgs/org_1/projects/proj_1/logs/export");
    expect(Object.fromEntries(url.searchParams)).toEqual({ from: "2026-10-12T12:00:00.000Z", to: "2026-10-13T12:00:00.000Z", status: "429", format: "csv" });
    const check = new URL(r.check, "http://x");
    expect(check.pathname).toBe("/platform/v1/orgs/org_1/projects/proj_1/logs");
    expect(Object.fromEntries(check.searchParams)).toEqual({ from: "2026-10-12T12:00:00.000Z", to: "2026-10-13T12:00:00.000Z", status: "429", limit: "1" });
    expect(r.file).toEqual({ name: "kaeso-logs-proj_1.csv", description: "CSV", mime: "text/csv", extension: "csv" });
  });

  it("keeps whole UTC days and asks for content only in JSON Lines", () => {
    const r = exportRequest("org_1", "proj_1", { from: "2026-08-14", to: "2026-10-13" }, "content", now);
    expect(Object.fromEntries(new URL(r.url, "http://x").searchParams)).toEqual({
      from: "2026-08-14T00:00:00Z",
      to: "2026-10-14T00:00:00.000Z",
      format: "jsonl",
      include_content: "true",
    });
    expect(r.file.name).toBe("kaeso-logs-proj_1.jsonl");
  });
});
