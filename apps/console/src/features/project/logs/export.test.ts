import { describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { exportFilters, exportRequest, exportSummary } from "./export";

describe("exportSummary", () => {
  it("names the period alone when nothing else filters", () => {
    expect(exportSummary({})).toBe("Every request from the last 24 hours, newest first.");
    expect(exportSummary({ range: "1h" })).toBe("Every request from the last hour, newest first.");
    expect(exportSummary({ range: "7d" })).toBe("Every request from the last 7 days, newest first.");
    expect(exportSummary({ range: "30d" })).toBe("Every request from the last 30 days, newest first.");
    expect(exportSummary({ from: "2026-08-14", to: "2026-08-14" })).toBe("Every request on 14 Aug 2026, newest first.");
    expect(exportSummary({ from: "2026-08-14", to: "2026-10-13" })).toBe("Every request from 14 Aug 2026 to 13 Oct 2026, newest first.");
  });

  it("says the requests match the filters, one or several", () => {
    expect(exportSummary({ source: "playground" })).toBe("Every request from the last 24 hours that matches this filter, newest first.");
    expect(exportSummary({ range: "7d", failed: true, source: "api" })).toBe("Every request from the last 7 days that matches these filters, newest first.");
    expect(exportSummary({ from: "2026-08-14", to: "2026-08-14", status: 429 })).toBe("Every request on 14 Aug 2026 that matches this filter, newest first.");
    expect(exportSummary({ from: "2026-08-14", to: "2026-10-13", stream: false })).toBe(
      "Every request from 14 Aug 2026 to 13 Oct 2026 that matches this filter, newest first.",
    );
  });

  it("is a whole sentence in German and Chinese", async () => {
    apply(await load("de"));
    expect(exportSummary({ range: "1h" })).toBe("Alle Anfragen der letzten Stunde, neueste zuerst.");
    expect(exportSummary({ range: "7d", failed: true })).toBe("Alle Anfragen der letzten 7 Tage, die diesem Filter entsprechen, neueste zuerst.");
    expect(exportSummary({ failed: true, status: 500 })).toBe("Alle Anfragen der letzten 24 Stunden, die diesen Filtern entsprechen, neueste zuerst.");
    apply(await load("zh-CN"));
    expect(exportSummary({ from: "2026-08-14", to: "2026-08-14" })).toMatch(/的所有请求，按时间倒序。$/);
    expect(exportSummary({ range: "30d", key: "key_1" })).toBe("最近 30 天内符合这些筛选条件的所有请求，按时间倒序。");
    apply(await load("en"));
  });
});

describe("exportFilters", () => {
  it("lists every filter, with the names people know", () => {
    expect(
      exportFilters(
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
          source: "api",
        },
        { key: "checkout-web", member: "Dana Ito" },
      ),
    ).toEqual([
      { label: "Key", value: "checkout-web" },
      { label: "Member", value: "Dana Ito" },
      { label: "Model", value: "gpt-5.4-mini" },
      { label: "Provider", value: "OpenAI" },
      { label: "Status", value: "429" },
      { label: "Failed only", value: "Yes" },
      { label: "Error kind", value: "Rate limited" },
      { label: "Streaming", value: "Streamed" },
      { label: "Source", value: "API" },
    ]);
  });

  it("falls back to the ids when it has no names", () => {
    expect(exportFilters({ key: "key_1", member: "sa:sa_deploy", stream: false, source: "playground" })).toEqual([
      { label: "Key", value: "key_1" },
      { label: "Member", value: "sa_deploy" },
      { label: "Streaming", value: "Not streamed" },
      { label: "Source", value: "Playground" },
    ]);
    expect(exportFilters({ range: "1h" })).toEqual([]);
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
