import type { ReactNode } from "react";
import { act, renderHook } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LogView } from "~/api/generated/types";
import { LIVE_PAGES, newestLogsQuery } from "~/api/queries/logs";
import { queryClient } from "~/query";
import { ORG, PROJECT } from "~/test/fixtures";
import { logRow } from "~/test/fixtures/project-logs";
import { on, page, requests, type ApiRequest } from "~/test/network";
import { useLiveLogs } from "./live";

const LOGS = "orgs/:org/projects/:project/logs";
const row = (n: number, minute = 59): LogView => logRow({ id: `req_${n}`, started_at: `2026-10-15T11:${minute}:${String(n % 60).padStart(2, "0")}Z` });
const queries = () => requests("GET", LOGS).map((r) => Object.fromEntries(r.query));

describe("asking for the newest requests", () => {
  it("asks only for what started since the newest request shown, past the first page", async () => {
    on("GET", LOGS, ({ query }: ApiRequest) => (query.get("after") ? page([row(1)]) : page([row(3), row(2)], "cur_2")));
    const newest = await queryClient.fetchQuery(newestLogsQuery(ORG, PROJECT, {}, "2026-10-15T11:58:30Z"));
    expect(newest).toEqual({ rows: [row(3), row(2), row(1)], complete: true });
    expect(queries()).toEqual([
      { from: "2026-10-15T11:58:30Z", limit: "100" },
      { from: "2026-10-15T11:58:30Z", limit: "100", after: "cur_2" },
    ]);
  });

  it("stops after a few pages and says the newest requests don't reach the ones shown", async () => {
    on("GET", LOGS, ({ query }: ApiRequest) => page([row(Number(query.get("after") ?? 0))], String(Number(query.get("after") ?? 0) + 1)));
    const newest = await queryClient.fetchQuery(newestLogsQuery(ORG, PROJECT, {}, "2026-10-15T11:58:30Z"));
    expect(newest.complete).toBe(false);
    expect(requests("GET", LOGS)).toHaveLength(LIVE_PAGES);
  });

  it("asks for one page of the range when nothing is shown yet", async () => {
    on("GET", LOGS, page([row(1)], "cur_2"));
    expect(await queryClient.fetchQuery(newestLogsQuery(ORG, PROJECT, { range: "1h" }))).toEqual({ rows: [row(1)], complete: true });
    expect(requests("GET", LOGS)).toHaveLength(1);
  });
});

describe("live updates", () => {
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  const scroller = { current: null };
  const tick = () => act(() => vi.advanceTimersByTimeAsync(5000));
  let hidden = false;

  beforeEach(() => {
    hidden = false;
    vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  const live = (base: LogView[], max?: number) =>
    renderHook(() => useLiveLogs({ org: ORG, project: PROJECT, filters: {}, live: true, base, scroller, ...(max ? { max } : {}) }), { wrapper });

  it("doesn't ask while the tab is in the background, and catches up from the newest request when it's back", async () => {
    const base = [row(1, 58)];
    on("GET", LOGS, page([row(2), ...base]));
    const view = live(base);
    hidden = true;
    await tick();
    await tick();
    expect(requests("GET", LOGS)).toHaveLength(0);

    hidden = false;
    await tick();
    expect(queries()).toEqual([{ from: row(1, 58).started_at, limit: "100" }]);
    expect(view.result.current.rows.map((r) => r.id)).toEqual(["req_2", "req_1"]);
    expect(view.result.current.cut).toBe(false);
  });

  it("keeps the newest requests once there are too many, and ends the list there", async () => {
    const base = [row(1, 58)];
    on("GET", LOGS, page([row(4), row(3), row(2)]));
    const view = live(base, 2);
    await tick();
    expect(view.result.current.rows.map((r) => r.id)).toEqual(["req_4", "req_3"]);
    expect(view.result.current.cut).toBe(true);

    on("GET", LOGS, page([row(5), row(4)]));
    await tick();
    expect(queries().at(-1)).toEqual({ from: row(4).started_at, limit: "100" });
    expect(view.result.current.rows.map((r) => r.id)).toEqual(["req_5", "req_4"]);
  });

  it("drops the requests shown when the newest ones no longer reach them", async () => {
    const base = [row(1, 58)];
    on("GET", LOGS, ({ query }: ApiRequest) => page([row(10 + Number(query.get("after") ?? 0))], String(Number(query.get("after") ?? 0) + 1)));
    const view = live(base);
    await tick();
    expect(view.result.current.rows.map((r) => r.id)).toEqual(["req_10", "req_11", "req_12", "req_13", "req_14"]);
    expect(view.result.current.cut).toBe(true);
  });
});
