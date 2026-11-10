import { describe, expect, it } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { apply, load } from "~/i18n";
import { exportFilters, exportRequest, exportSummary, toolExportFilters, toolExportRequest, toolExportSummary } from "./export";

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
        { key: "checkout-web", member: "Dana Ito", provider: "OpenAI" },
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
    expect(exportFilters({ key: "key_1", member: "sa:sa_deploy", provider: "openai", stream: false, source: "playground" })).toEqual([
      { label: "Key", value: "key_1" },
      { label: "Member", value: "sa_deploy" },
      { label: "Provider", value: "openai" },
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

  it("keeps whole days in the install's time zone and asks for content only in JSON Lines", () => {
    setFormatTimeZone("Europe/Berlin");
    const r = exportRequest("org_1", "proj_1", { from: "2026-08-14", to: "2026-10-13" }, "content", now);
    expect(Object.fromEntries(new URL(r.url, "http://x").searchParams)).toEqual({
      from: "2026-08-13T22:00:00.000Z",
      to: "2026-10-13T22:00:00.000Z",
      format: "jsonl",
      include_content: "true",
    });
    expect(r.file.name).toBe("kaeso-logs-proj_1.jsonl");
  });
});

describe("the tool call export", () => {
  it("names the period alone, or says the calls match the filters", async () => {
    apply(await load("en"));
    expect(toolExportSummary({})).toBe("Every tool call from the last 24 hours, newest first.");
    expect(toolExportSummary({ range: "1h" })).toBe("Every tool call from the last hour, newest first.");
    expect(toolExportSummary({ from: "2026-08-14", to: "2026-08-14" })).toBe("Every tool call on 14 Aug 2026, newest first.");
    expect(toolExportSummary({ from: "2026-08-14", to: "2026-10-13" })).toBe("Every tool call from 14 Aug 2026 to 13 Oct 2026, newest first.");
    expect(toolExportSummary({ range: "7d", tool: "x__create_post" })).toBe("Every tool call from the last 7 days that matches this filter, newest first.");
    expect(toolExportSummary({ from: "2026-08-14", to: "2026-08-14", status: "error", error: "timeout" })).toBe(
      "Every tool call on 14 Aug 2026 that matches these filters, newest first.",
    );
  });

  it("lists each filter by its name, or by its id when it has none", () => {
    const filters = { key: "key_web", member: "sa:sa_ci", server: "mcp_x", tool: "x__create_post", status: "refused" as const, error: "tool_off" };
    expect(toolExportFilters(filters).map((f) => f.value)).toEqual(["key_web", "sa_ci", "mcp_x", "x__create_post", "refused", "tool_off"]);
    expect(toolExportFilters(filters, { key: "Web app", member: "CI", server: "X" }, { status: "Refused", error: "Tool is off" }).map((f) => f.value)).toEqual([
      "Web app",
      "CI",
      "X",
      "x__create_post",
      "Refused",
      "Tool is off",
    ]);
    expect(toolExportFilters({})).toEqual([]);
  });

  it("asks for the chosen format with the filters, up to now or to the range's end", () => {
    const now = Date.parse("2026-10-15T12:00:00Z");
    const csv = toolExportRequest("org_1", "proj_1", { server: "mcp_x" }, "csv", now);
    expect(csv.file.name).toBe("kaeso-tool-calls-proj_1.csv");
    const url = new URL(csv.url, "https://kaeso.example.com");
    expect(url.pathname).toBe("/platform/v1/orgs/org_1/projects/proj_1/logs/tool-calls/export");
    expect(Object.fromEntries(url.searchParams)).toMatchObject({ format: "csv", server: "mcp_x", to: "2026-10-15T12:00:00.000Z" });
    expect(url.searchParams.has("include_content")).toBe(false);
    const content = toolExportRequest("org_1", "proj_1", { from: "2026-10-01", to: "2026-10-03" }, "content", now);
    expect(content.file.name).toBe("kaeso-tool-calls-proj_1.jsonl");
    const query = new URL(content.url, "https://kaeso.example.com").searchParams;
    expect(query.get("include_content")).toBe("true");
    expect(query.get("to")).not.toBe("2026-10-15T12:00:00.000Z");
  });
});
