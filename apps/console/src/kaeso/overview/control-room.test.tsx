import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { TimelineBucket, UsageRow } from "~/api/generated/types";
import { atFixtureTime } from "~/test/clock";
import { ORG, OTHER_PROJECT, PROJECT, project, signedIn, whoAmI, withRights, without } from "~/test/fixtures";
import { member, usageRow as orgRow } from "~/test/fixtures/org";
import { apiKey, catalogueModel, limit, listed, projectData, projectPath, serviceAccount, usageRow } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const USAGE = "orgs/:org/projects/:project/usage";
const ORG_USAGE = "orgs/:org/usage";
const PERIOD = "from=2026-10-01&to=2026-10-14";

type Rows = Partial<Record<string, UsageRow[]>>;

function bucket(start: string, overrides: Partial<TimelineBucket> = {}): TimelineBucket {
  return {
    start,
    requests: 40,
    errors: 4,
    input_tokens: 1000,
    output_tokens: 600,
    cache_read_tokens: 500,
    cache_write_tokens: 50,
    reasoning_tokens: 100,
    cost: "2",
    duration_p50_ms: 900,
    duration_p95_ms: 2400,
    duration_max_ms: 3100,
    ttfb_p50_ms: 300,
    ttfb_p95_ms: 800,
    ...overrides,
  };
}

function usage(path: string, now: Rows, before: Rows = {}, from = "2026-10-01"): void {
  on("GET", path, ({ query }: { query: URLSearchParams }) => {
    const rows = query.get("from") === from ? now : before;
    return { data: rows[query.get("group_by") ?? "day"] ?? [] };
  });
}

const cell = (title: string | RegExp) => screen.getByRole("heading", { name: title }).closest("section") as HTMLElement;
const tab = (name: RegExp) => screen.getByRole("tab", { name });

const THIS_PERIOD: Rows = {
  day: [
    usageRow({ key: "2026-10-13", requests: 40, errors: 4, cache_read_tokens: 500, cache_write_tokens: 50, reasoning_tokens: 100, cost: "2", tool_calls: 6 }),
  ],
  model: [
    usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 30, errors: 3, cost: "1.5", tool_calls: 4, cache_read_tokens: 400 }),
    usageRow({ key: "openai/gpt-5.4-mini", requests: 8, errors: 1, cost: "0.5", tool_calls: 2 }),
    usageRow({ key: "anthropic/", requests: 2, errors: 0, cost: null }),
  ],
  key: [usageRow({ key: "key_web", requests: 25 }), usageRow({ key: "playground", requests: 15 })],
};
const BEFORE: Rows = {
  day: [usageRow({ key: "2026-09-20", requests: 20, errors: 1, cost: "1", tool_calls: 2 })],
  model: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 15, errors: 1, cost: "0.5" })],
};

describe("the analytics control room", () => {
  it("leads with the totals and their change, charts the chosen one by the day and ranks what drives it", async () => {
    signedIn();
    projectData({ models: [catalogueModel()], keys: [apiKey()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD, BEFORE);
    on("GET", `${USAGE}/timeline`, { data: [bucket("2026-10-13T09:00:00Z")] });
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByRole("heading", { name: "Requests per day, UTC" })).toBeInTheDocument();
    await waitFor(() => expect(tab(/^Requests/)).toHaveTextContent(/^Requests40↑100%vs\. prior 14 days$/));
    expect(tab(/^Error rate/)).toHaveTextContent(/10%↑100% worsevs\. prior 14 days/);
    expect(requests("GET", `${USAGE}/timeline`).map((r) => r.query.get("step"))).toEqual(expect.arrayContaining(["hour"]));
    const chart = cell("Requests per day, UTC");
    expect(chart).toHaveTextContent("3 per day on average; highest 40, 13 Oct");
    expect(
      within(chart)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual(["This period", "vs. prior 14 days"]);
    expect(chart).not.toHaveTextContent("Failed requests");

    const keys = within(cell("Keys by requests")).getByRole("list");
    await waitFor(() =>
      expect(
        within(keys)
          .getAllByRole("button")
          .map((b) => b.textContent),
      ).toEqual([expect.stringContaining("checkout-web"), expect.stringContaining("Playground")]),
    );
    const models = within(cell("Models by requests")).getAllByRole("button");
    expect(models[0]).toHaveTextContent(/Claude Sonnet 4\.5.*75%↑100%30$/);
    expect(models[1]).toHaveTextContent(/gpt-5\.4-mini.*20%New8$/);
    expect(models[2]).toHaveTextContent("No model");
    expect(within(cell("Models by requests")).getByRole("link", { name: "Show all" })).toHaveAttribute(
      "href",
      projectPath("analytics/models?from=2026-10-01&to=2026-10-14"),
    );

    await user.click(tab(/^Tokens/));
    expect(await screen.findByRole("heading", { name: "Tokens per day, UTC" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Models by tokens" })).toBeInTheDocument();

    await user.click(tab(/^Cost/));
    expect(await screen.findByRole("heading", { name: "Models by cost" })).toBeInTheDocument();
    expect(cell("Models by cost")).toHaveTextContent(/Claude Sonnet 4\.5.*75%.*\$1\.50/);
    expect(tab(/^Cost/)).not.toHaveTextContent(/better|worse/);

    await user.click(tab(/^Error rate/));
    expect(await screen.findByRole("heading", { name: "Error rate per day, UTC" })).toBeInTheDocument();
    expect(cell("Error rate per day, UTC")).toHaveTextContent("Highest: 10%, 13 Oct");
    expect(cell("Models by failed requests")).toHaveTextContent(/Claude Sonnet 4\.5.*10%.*↑200% worse.*3/);

    await user.click(tab(/^Tool calls/));
    expect(await screen.findByRole("heading", { name: "Models by tool calls" })).toBeInTheDocument();

    await user.click(within(cell("Models by tool calls")).getByRole("button", { name: /gpt-5\.4-mini/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("logs")));
    expect(router.state.location.searchStr).toBe("?model=gpt-5.4-mini&from=2026-10-01&to=2026-10-14");
  });

  it("counts the requests the spend leaves out because no price covers them", async () => {
    signedIn();
    projectData({ models: [catalogueModel()], keys: [apiKey()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, { ...THIS_PERIOD, day: [usageRow({ key: "2026-10-13", requests: 40, cost: "2", unpriced: 3 })] }, BEFORE);
    on("GET", `${USAGE}/timeline`, () => ({ data: [bucket("2026-10-13T09:00:00Z")] }));
    await renderApp(projectPath(`analytics?${PERIOD}`));

    await waitFor(() => expect(tab(/^Cost/)).toHaveTextContent(/\$2\.00.*3 requests unknown/));
    expect(cell("Cost by model")).toHaveTextContent("3 requests without a price aren't included.");
  });

  it("opens the period's failed requests of a model from the ranks, and of a key its requests", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD);
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}&metric=tokens`));
    expect(await screen.findByRole("heading", { name: "Tokens per day, UTC" })).toBeInTheDocument();
    expect(within(cell("Tokens per day, UTC")).queryByText("vs. prior 14 days")).not.toBeInTheDocument();
    const keys = within(cell("Keys by tokens")).getByRole("list");
    await user.click(await within(keys).findByRole("button", { name: /checkout-web/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?key=key_web&from=2026-10-01&to=2026-10-14"));

    await router.navigate({ href: projectPath(`analytics?${PERIOD}&metric=errors`) });
    await user.click(await within(await screen.findByRole("list", { name: "Models by failed requests" })).findByRole("button", { name: /gpt-5\.4-mini/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?model=gpt-5.4-mini&failed=true&from=2026-10-01&to=2026-10-14"));
  });

  it("says what failed and when answers were slowest, and opens the request logs for each", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD, BEFORE);
    on("GET", `${USAGE}/timeline`, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "error_kind"
        ? {
            data: [
              bucket("2026-10-13T09:00:00Z", { key: "rate_limited", requests: 3 }),
              bucket("2026-10-13T10:00:00Z", { key: "rate_limited", requests: 2 }),
              bucket("2026-10-13T10:00:00Z", { key: "", requests: 1 }),
            ],
          }
        : {
            data: [
              bucket("2026-10-13T09:00:00Z", { requests: 30, duration_p95_ms: 2400 }),
              bucket("2026-10-12T22:00:00Z", { requests: 12, duration_p95_ms: 8100 }),
              bucket("2026-10-11T03:00:00Z", { requests: 2, duration_p95_ms: 60_000 }),
            ],
          },
    );
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByRole("heading", { name: "Reliability" })).toBeInTheDocument();
    const failed = cell("Failed requests");
    await waitFor(() => expect(failed).toHaveTextContent(/4↑300% worse10% of requests/));
    expect(within(failed).getByRole("link", { name: "Failed requests" })).toHaveAttribute(
      "href",
      projectPath("logs?failed=true&from=2026-10-01&to=2026-10-14"),
    );
    expect(
      requests("GET", `${USAGE}/timeline`)
        .filter((r) => r.query.get("group_by") === "error_kind")
        .map((r) => r.query.get("failed")),
    ).toContain("true");

    const kinds = within(cell("Failures by kind")).getAllByRole("button");
    await waitFor(() => expect(kinds.map((b) => b.textContent)).toEqual(["Rate limited83%5", "Other failures17%1"]));
    const slowest = within(cell("Slowest hours, 95th percentile")).getAllByRole("button");
    expect(slowest.map((b) => b.textContent)).toEqual([expect.stringMatching(/Oct 12.*12 requests8\.1 s/), expect.stringMatching(/Oct 13.*30 requests2\.4 s/)]);

    await user.click(kinds[0] as HTMLElement);
    await waitFor(() => expect(router.state.location.searchStr).toBe("?failed=true&error=rate_limited&from=2026-10-01&to=2026-10-14"));
    await router.navigate({ href: projectPath(`analytics?${PERIOD}`) });
    await user.click(await within(await screen.findByRole("list", { name: "Failures by kind" })).findByRole("button", { name: /Other failures/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?failed=true&from=2026-10-01&to=2026-10-14"));
    await router.navigate({ href: projectPath(`analytics?${PERIOD}`) });
    await user.click(await within(await screen.findByRole("list", { name: "Slowest hours, 95th percentile" })).findByRole("button", { name: /Oct 12/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?from=2026-10-12&to=2026-10-12"));
  });

  it("keeps the slowest hour of a quiet period, and says how the cache and the tokens of a request did", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD, { day: [usageRow({ key: "2026-09-20", requests: 20, input_tokens: 1000, cache_read_tokens: 250 })] });
    on("GET", `${USAGE}/timeline`, ({ query }: { query: URLSearchParams }) => ({
      data: query.get("group_by") ? [] : [bucket("2026-10-11T03:00:00Z", { requests: 2, duration_p95_ms: 60_000 })],
    }));
    await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByRole("heading", { name: "Efficiency" })).toBeInTheDocument();
    await waitFor(() => expect(within(cell("Slowest hours, 95th percentile")).getByRole("button")).toHaveTextContent(/Oct 11.*2 requests1 min/));
    expect(cell("Failures by kind")).toHaveTextContent("No failed requests.");
    await waitFor(() => expect(cell("Cache hit")).toHaveTextContent(/%↑\d+% better.*500 input tokens from cache/));
    expect(cell("Tokens per request")).toHaveTextContent(/ in, .* out/);
  });

  describe("on the current day", () => {
    atFixtureTime();

    it("charts today by the hour", async () => {
      signedIn();
      projectData();
      on("GET", "orgs/:org/projects/:project/logs", listed([]));
      usage(USAGE, { day: [usageRow({ key: "2026-10-15", requests: 40 })] }, {}, "2026-10-15");
      on("GET", `${USAGE}/timeline`, { data: [bucket("2026-10-15T10:00:00Z", { cost: null }), bucket("2026-10-15T11:00:00Z")] });
      await renderApp(projectPath("analytics?period=today"));

      expect(await screen.findByRole("heading", { name: "Requests per hour, UTC" })).toBeInTheDocument();
      await waitFor(() => expect(tab(/^Requests/)).toHaveTextContent(/40.*So far today/));
      expect(new Set(requests("GET", `${USAGE}/timeline`).map((r) => r.query.get("step")))).toEqual(new Set(["hour"]));
    });

    it("sets today against the 13 days before when request logs are off, and ranks the failures by model", async () => {
      signedIn({ session: whoAmI({ request_logs: false }) });
      projectData();
      usage(
        USAGE,
        { day: [usageRow({ key: "2026-10-15", requests: 40 })], model: [usageRow({ key: "openai/gpt-5.4-mini", requests: 8, errors: 2 })] },
        { day: [usageRow({ key: "2026-10-10", requests: 12 }), usageRow({ key: "2026-10-15", requests: 40 })] },
        "2026-10-15",
      );
      const { user } = await renderApp(projectPath("analytics?period=today"));

      expect(await screen.findByRole("heading", { name: "Requests per day, the last 14 days, UTC" })).toBeInTheDocument();
      expect(requests("GET", `${USAGE}/timeline`)).toHaveLength(0);
      await waitFor(() => expect(cell("Failed requests by model")).toHaveTextContent(/gpt-5\.4-mini.*25%.*2/));
      expect(within(cell("Failed requests by model")).queryByRole("button")).not.toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "Slowest hours, 95th percentile" })).not.toBeInTheDocument();
      expect(screen.queryByRole("link", { name: "Failed requests" })).not.toBeInTheDocument();
      await user.click(tab(/^Tokens/));
      expect(await screen.findByRole("heading", { name: "Tokens per day, the last 14 days, UTC" })).toBeInTheDocument();
    });

    it("falls back to days when the timeline can't load", async () => {
      signedIn();
      projectData();
      on("GET", "orgs/:org/projects/:project/logs", listed([]));
      usage(USAGE, { day: [usageRow({ key: "2026-10-14", requests: 40 })] }, {}, "2026-10-09");
      on("GET", `${USAGE}/timeline`, () => apiError(500, "api_error", "ClickHouse is down."));
      await renderApp(projectPath("analytics?period=7d"));
      expect(await screen.findByRole("heading", { name: "Requests per day, UTC" })).toBeInTheDocument();
      expect(await screen.findByRole("heading", { name: "Failed requests by model" })).toBeInTheDocument();
    });

    it("charts weeks beyond a quarter and still forecasts the month, with the budget and the cost by model", async () => {
      signedIn();
      projectData({ limits: [limit()], models: [catalogueModel()] });
      on("GET", "orgs/:org/projects/:project/logs", listed([]));
      const priced = ["a", "b", "c", "d", "e"].map((m, i) => usageRow({ key: `openai/model-${m}`, requests: 10, cost: String(5 - i) }));
      on("GET", USAGE, ({ query }: { query: URLSearchParams }) => {
        const by = query.get("group_by");
        if (by === "model") return { data: priced };
        if (by === "week") return { data: [usageRow({ key: "2026-10-12", cost: "8" })] };
        return { data: [usageRow({ key: "2026-10-01", cost: "3" }), usageRow({ key: "2026-10-10", cost: "7" }), usageRow({ key: "2026-10-15", cost: "1" })] };
      });
      await renderApp(projectPath("analytics?period=6m"));

      expect(await screen.findByRole("heading", { name: "Requests per week, UTC" })).toBeInTheDocument();
      expect(requests("GET", `${USAGE}/timeline`)).toHaveLength(0);
      const spend = (await screen.findByRole("heading", { name: "Forecast for October" })).closest("section") as HTMLElement;
      await waitFor(() => expect(spend).toHaveTextContent("$11.00 so far, at the last 7 days' pace"));
      expect(spend).toHaveTextContent("$12.50 of $50.00");
      expect(within(spend).getByRole("link", { name: "Limits" })).toHaveAttribute("href", projectPath("limits"));
      expect(screen.getByRole("img", { name: /^Cost by model:/ })).toHaveAccessibleName(/model-a.*model-b.*model-c.*model-d.*Other \$1\.00/);
      expect(within(cell("Models by requests")).getByRole("link", { name: "Show all" })).toHaveAttribute("href", projectPath("analytics/models?period=6m"));
    });
  });

  it("says when a period cost nothing, and counts the rows behind Show all", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, {
      day: [usageRow({ key: "2026-10-13", requests: 6, cost: "0" })],
      model: ["a", "b", "c", "d", "e", "f"].map((m) => usageRow({ key: `openai/model-${m}`, requests: 1, cost: "0" })),
    });
    on("GET", `${USAGE}/timeline`, { data: [] });
    await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByText("No cost in this period")).toBeInTheDocument();
    expect(await within(cell("Models by requests")).findByRole("link", { name: "Show all 6" })).toBeInTheDocument();
    expect(within(cell("Models by requests")).getAllByRole("listitem")).toHaveLength(5);
    expect(cell("Slowest hours, 95th percentile")).toHaveTextContent("Nothing in this period");
  });

  it("keeps a noun's capital in a German heading", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD);
    on("GET", `${USAGE}/timeline`, { data: [] });
    await renderApp(projectPath(`analytics?${PERIOD}`), { locale: "de" });
    expect(await screen.findByRole("heading", { name: "Modelle nach Anfragen" })).toBeInTheDocument();
  });

  it("ranks an organization's projects and opens one from either list", async () => {
    signedIn();
    usage(ORG_USAGE, {
      project: [orgRow(PROJECT, { requests: 30, errors: 3 }), orgRow(OTHER_PROJECT, { requests: 10, errors: 1 }), orgRow("proj_gone", { requests: 1 })],
      model: [orgRow("anthropic/claude-sonnet-4-5", { requests: 30 })],
    });
    on("GET", `${ORG_USAGE}/timeline`, () => apiError(500, "api_error", "ClickHouse is down."));
    const { user, router } = await renderApp(`/orgs/${ORG}/analytics?${PERIOD}&metric=errors`);

    expect(await screen.findByRole("heading", { name: "Projects by failed requests" })).toBeInTheDocument();
    await user.click(tab(/^Requests/));
    const failed = await waitFor(() => cell("Failed requests by project"));
    await waitFor(() => expect(within(failed).getAllByRole("button")[0]).toHaveTextContent(/Checkout assistant.*10%/));
    expect(within(cell("Projects by requests")).getAllByRole("button")[2]).toHaveTextContent("proj_gone");
    expect(within(cell("Projects by requests")).getByRole("link", { name: "Show all" })).toHaveAttribute(
      "href",
      `/orgs/${ORG}/analytics/projects?from=2026-10-01&to=2026-10-14`,
    );
    expect(within(cell("Models by requests")).queryByRole("button")).not.toBeInTheDocument();
    await user.click(within(failed).getByRole("button", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));

    await router.navigate({ href: `/orgs/${ORG}/analytics?${PERIOD}` });
    await user.click(await within(cell("Projects by requests")).findByRole("button", { name: /Checkout assistant/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}`));
  });

  it("lists an organization's failures by kind without links, since request logs belong to projects", async () => {
    signedIn();
    usage(ORG_USAGE, { day: [orgRow("2026-10-13", { requests: 30, errors: 3 })], project: [orgRow(PROJECT, { requests: 30, errors: 3 })] });
    on("GET", `${ORG_USAGE}/timeline`, ({ query }: { query: URLSearchParams }) => ({
      data: [bucket("2026-10-13T09:00:00Z", query.get("group_by") ? { key: "provider_error", requests: 3 } : { requests: 30 })],
    }));
    await renderApp(`/orgs/${ORG}/analytics?${PERIOD}`);

    await waitFor(() => expect(cell("Failures by kind")).toHaveTextContent(/Provider error100%3/));
    expect(within(cell("Failures by kind")).queryByRole("button")).not.toBeInTheDocument();
    expect(within(cell("Slowest hours, 95th percentile")).queryByRole("button")).not.toBeInTheDocument();
    expect(cell("Slowest hours, 95th percentile")).toHaveTextContent(/Oct 13.*30 requests/);
  });
});

describe("the analytics views", () => {
  it("breaks an organization down by project and keeps the period between views", async () => {
    signedIn();
    usage(
      ORG_USAGE,
      { project: [orgRow(PROJECT, { requests: 30, errors: 3, cost: "2" }), orgRow(OTHER_PROJECT, { requests: 10, cost: "1" })] },
      { project: [orgRow(PROJECT, { requests: 30 })] },
    );
    const { user, router } = await renderApp(`/orgs/${ORG}/analytics/projects?${PERIOD}`);

    const table = await screen.findByRole("grid", { name: "Projects in the period" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(3));
    expect(screen.getByRole("link", { name: "Analytics", current: "page" })).toHaveAttribute("href", `/orgs/${ORG}/analytics`);
    expect(within(table).getAllByRole("row")[2]).toHaveTextContent(/Support triage.*10.*New/);
    expect(cell("Highest cost")).toHaveTextContent(/Checkout assistant.*\$2\.00, 67% of cost/);
    expect(cell("Highest error rate")).toHaveTextContent(/Checkout assistant.*10% of 30 failed/);
    expect(screen.getByRole("list", { name: "Grew the most" })).toHaveTextContent(/Support triage.*New.*\+10/);

    await user.click(screen.getByRole("tab", { name: "Members" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/analytics/members`));
    expect(router.state.location.search).toEqual({ from: "2026-10-01", to: "2026-10-14" });
    await user.click(screen.getByRole("tab", { name: "Overview" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/analytics`));

    await router.navigate({ href: `/orgs/${ORG}/analytics/projects?${PERIOD}` });
    await user.click((await screen.findAllByRole("row", { name: /Support triage/ }))[0] as HTMLElement);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("names an organization's members, service accounts and former members", async () => {
    signedIn();
    on("GET", "orgs/:org/members", listed([member(), member({ user: { id: "usr_grace", name: "Grace Hopper" } })]));
    usage(ORG_USAGE, {
      member: [
        orgRow("user:usr_ada", { requests: 30 }),
        orgRow("user:usr_grace", { requests: 20 }),
        orgRow("service_account:sa_ci", { requests: 5 }),
        orgRow("user:usr_gone", { requests: 1 }),
      ],
    });
    await renderApp(`/orgs/${ORG}/analytics/members?${PERIOD}`);
    const table = await screen.findByRole("table", { name: "Members in the period" });
    await waitFor(() =>
      expect(
        within(table)
          .getAllByRole("row")
          .map((r) => r.textContent),
      ).toEqual([
        expect.any(String),
        expect.stringContaining("Ada Okafor (you)"),
        expect.stringContaining("Grace Hopper"),
        expect.stringContaining("Service account"),
        expect.stringContaining("Former member"),
      ]),
    );
  });

  it("says when an organization's view doesn't exist", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}/analytics/routes`);
    expect(await screen.findByText(/doesn't exist|not found/i)).toBeInTheDocument();
  });

  it("tells people without the right that they can't see an organization's usage", async () => {
    withRights(["org.view"]);
    await renderApp(`/orgs/${ORG}/analytics/models`);
    expect(await screen.findByText("You can't see this organization's usage")).toBeInTheDocument();
    expect(screen.queryByRole("tablist", { name: "Analytics views" })).not.toBeInTheDocument();
  });

  it("shows models without a price as unknown, not $0, with what their media requests made", async () => {
    signedIn();
    projectData();
    const none = { images: 0, characters: 0, audio_ms: 0, videos: 0, video_ms: 0 };
    usage(
      USAGE,
      {
        model: [
          usageRow({ key: "openai/gpt-5.4-mini", requests: 20, cost: "3" }),
          usageRow({ key: "openai/sora-2", requests: 6, unpriced: 6, cost: "0", media: { ...none, videos: 6, video_ms: 48000 } }),
          usageRow({ key: "openai/gpt-image-2", requests: 4, unpriced: 1, cost: "1", media: { ...none, images: 5 } }),
        ],
      },
      {},
    );
    await renderApp(projectPath(`analytics/models?${PERIOD}`));
    const table = await screen.findByRole("grid", { name: "Models in the period" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(4));
    const sora = within(table)
      .getAllByRole("row")
      .find((r) => r.textContent?.includes("sora-2")) as HTMLElement;
    expect(sora).toHaveTextContent(/6 videos, 48/);
    expect(within(sora).getAllByText("Unknown")).toHaveLength(2);
    expect(within(sora).queryByText("$0.00")).not.toBeInTheDocument();
    const image = within(table)
      .getAllByRole("row")
      .find((r) => r.textContent?.includes("gpt-image-2")) as HTMLElement;
    expect(image).toHaveTextContent(/5 images/);
    expect(within(image).getByText("+1 unknown")).toBeInTheDocument();
    expect(cell("Highest cost")).toHaveTextContent(/\$3\.00, 75% of the known cost/);
    expect(cell("Share of cost")).toHaveTextContent("7 requests without a price aren't included.");
  });

  it("calls the highest cost unknown when no model has a price", async () => {
    signedIn();
    projectData();
    usage(USAGE, { model: [usageRow({ key: "openai/sora-2", requests: 6, unpriced: 6, cost: "0" })] }, {});
    await renderApp(projectPath(`analytics/models?${PERIOD}`));

    await waitFor(() => expect(cell("Highest cost")).toHaveTextContent("Unknown"));
  });

  it("breaks a project down by member and key without cost or request logs", async () => {
    signedIn({ session: whoAmI({ request_logs: false }), projectRights: without("costs.view") });
    projectData({ keys: [apiKey()], accounts: [serviceAccount()] });
    usage(
      USAGE,
      {
        member: [
          usageRow({ key: "user:usr_ada", requests: 30, input_tokens: 50 }),
          usageRow({ key: "service_account:sa_deploy", requests: 10, input_tokens: 9000 }),
        ],
        key: [usageRow({ key: "key_web", requests: 30 })],
      },
      {},
    );
    const { user, router } = await renderApp(projectPath(`analytics/members?${PERIOD}`));
    const table = await screen.findByRole("table", { name: "Members in the period" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(3));
    expect(within(table).getAllByRole("row")[2]).toHaveTextContent("ci-deploy");
    expect(cell("Most tokens")).toHaveTextContent("ci-deploy");
    expect(within(table).queryByRole("columnheader", { name: "Cost" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Keys" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("analytics/keys")));
    expect(await screen.findByRole("table", { name: "Keys in the period" })).toHaveTextContent("checkout-web");
  });

  it("names unknown projects by id, puts the smallest under Other and lists everything that shrank", async () => {
    signedIn({ projects: [project({ avatar_url: "https://avatars.example/checkout.png" }), project({ id: OTHER_PROJECT, name: "Support triage" })] });
    const ids = ["proj_a", "proj_b", "proj_c", "proj_d", "proj_e"];
    usage(
      ORG_USAGE,
      {
        project: [
          orgRow(PROJECT, { requests: 30 }),
          ...ids.map((id, i) => orgRow(id, { requests: 10 - i })),
          orgRow("proj_quiet", { requests: 0, input_tokens: 0, output_tokens: 0 }),
        ],
      },
      { project: [orgRow(PROJECT, { requests: 40 }), orgRow("proj_a", { requests: 20 }), orgRow("proj_b", { requests: 30 })] },
    );
    await renderApp(`/orgs/${ORG}/analytics/projects?${PERIOD}`);
    const table = await screen.findByRole("grid", { name: "Projects in the period" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(8));
    expect(within(table).getAllByRole("row")[7]).toHaveTextContent(/proj_quiet.*0.*—/);
    expect(screen.getByRole("img", { name: /^Share of requests:/ })).toHaveAccessibleName(/proj_d 7 .*Other 6/);
    expect(
      within(screen.getByRole("list", { name: "Shrank the most" }))
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual([expect.stringContaining("proj_b"), expect.stringContaining("proj_a"), expect.stringContaining("Checkout assistant")]);
  });

  it("opens the request logs for a key from a project's keys", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    on("GET", USAGE, () => ({ data: [usageRow({ key: "key_web", requests: 30 })] }));
    const { user, router } = await renderApp(projectPath("analytics/keys?period=today"));
    const table = await screen.findByRole("grid", { name: "Keys in the period" });
    expect(within(table).queryByRole("columnheader", { name: "Change" })).not.toBeInTheDocument();
    await user.click(await within(table).findByRole("row", { name: /checkout-web/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?key=key_web"));
  });

  it("labels usage that reached no model, and goes back to the overview with the period", async () => {
    signedIn();
    projectData();
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) => ({
      data: query.get("group_by") === "model" ? [usageRow({ key: "anthropic/", requests: 3 })] : [],
    }));
    const { user, router } = await renderApp(projectPath(`analytics/models?${PERIOD}`));
    const table = await screen.findByRole("grid", { name: "Models in the period" });
    expect(await within(table).findByText("No model")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Overview" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("analytics")));
    expect(router.state.location.search).toEqual({ from: "2026-10-01", to: "2026-10-14" });
  });

  it("loads a view again after it failed", async () => {
    signedIn();
    projectData();
    let fail = true;
    on("GET", USAGE, () => (fail ? apiError(500, "api_error", "ClickHouse is down.") : { data: [usageRow({ key: "openai/gpt-5.4-mini", requests: 7 })] }));
    const { user } = await renderApp(projectPath(`analytics/models?${PERIOD}`));
    const retry = await screen.findByRole("button", { name: "Try again" });
    fail = false;
    await user.click(retry);
    expect(await screen.findByRole("row", { name: /gpt-5\.4-mini/ })).toBeInTheDocument();
  });

  it("says when a project's view doesn't exist", async () => {
    signedIn();
    projectData();
    await renderApp(projectPath("analytics/routes"));
    expect(await screen.findByText(/doesn't exist|not found/i)).toBeInTheDocument();
  });
});
