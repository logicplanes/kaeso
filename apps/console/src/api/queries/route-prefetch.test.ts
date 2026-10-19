import { describe, expect, it } from "vitest";
import { queryClient } from "~/query";
import { ORG, PROJECT } from "~/test/fixtures";
import { atFixtureTime } from "~/test/clock";
import { logRow } from "~/test/fixtures/project-logs";
import { auditEvent } from "~/test/fixtures/shell";
import { on, page, requests } from "~/test/network";
import { auditLogQuery } from "./audit";
import { logsInfiniteQuery } from "./logs";
import { prefetchLogs, prefetchProjectAuditLog, prefetchProjectOverview } from "./route-prefetch";

atFixtureTime();

const USAGE = "orgs/:org/projects/:project/usage";
const usageQueries = () => requests("GET", USAGE).map((r) => Object.fromEntries(r.query));

describe("starting a page's data while its route loads", () => {
  it("starts the request log with the page's filters", async () => {
    on("GET", "orgs/:org/projects/:project/logs", page([logRow()]));
    await prefetchLogs(queryClient, ORG, PROJECT, { model: "gpt-5.4-mini" });
    expect(queryClient.getQueryData(logsInfiniteQuery(ORG, PROJECT, { model: "gpt-5.4-mini" }).queryKey)?.pages[0]?.data).toEqual([logRow()]);
  });

  it("starts the project's audit log with the page's filters", async () => {
    on("GET", "orgs/:org/projects/:project/audit-log", page([auditEvent()]));
    await prefetchProjectAuditLog(queryClient, ORG, PROJECT, { action: "key.created" });
    expect(requests("GET", "orgs/:org/projects/:project/audit-log")[0]?.query.get("action")).toBe("key.created");
    expect(queryClient.getQueryData(auditLogQuery({ org: ORG, project: PROJECT }, { action: "key.created" }).queryKey)).toBeDefined();
  });

  it("starts the overview's daily usage for the chosen days", async () => {
    on("GET", USAGE, { data: [] });
    await prefetchProjectOverview(queryClient, ORG, PROJECT, { from: "2026-10-01", to: "2026-10-07" });
    expect(usageQueries()).toEqual([{ from: "2026-10-01", to: "2026-10-07", group_by: "day" }]);
    expect(requests("GET", `${USAGE}/span`)).toHaveLength(0);
  });

  it("reads the first day with usage before starting the whole period", async () => {
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", USAGE, { data: [] });
    await prefetchProjectOverview(queryClient, ORG, PROJECT, { period: "all" });
    expect(usageQueries()).toEqual([{ from: "2026-09-01", to: "2026-10-15", group_by: "day" }]);
  });

  it("leaves the whole period to the page when the first day can't be read", async () => {
    on("GET", `${USAGE}/span`, () => new Response(JSON.stringify({ error: { type: "invalid_request", message: "No." } }), { status: 400 }));
    await prefetchProjectOverview(queryClient, ORG, PROJECT, { period: "all" });
    expect(requests("GET", USAGE)).toHaveLength(0);
  });
});
