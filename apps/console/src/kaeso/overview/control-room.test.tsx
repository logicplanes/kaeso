import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { TimelineBucket, UsageRow } from "~/api/generated/types";
import { atFixtureTime } from "~/test/clock";
import { ORG, OTHER_PROJECT, PROJECT, project, signedIn, whoAmI, withRights, without } from "~/test/fixtures";
import { member, usageRow as orgRow } from "~/test/fixtures/org";
import { apiKey, catalogueModel, failedLog, limit, listed, projectData, projectPath, serviceAccount, usageRow } from "~/test/fixtures/project";
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
const BEFORE: Rows = { day: [usageRow({ key: "2026-09-20", requests: 20, errors: 1, cost: "1", tool_calls: 2 })] };

describe("the analytics control room", () => {
  it("charts the period by the hour against the period before, and morphs into each metric", async () => {
    signedIn();
    projectData({ models: [catalogueModel()], keys: [apiKey()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD, BEFORE);
    on("GET", `${USAGE}/timeline`, ({ query }: { query: URLSearchParams }) =>
      query.get("from") === "2026-10-01T00:00:00.000Z"
        ? { data: [bucket("2026-10-13T09:00:00Z")] }
        : { data: [bucket("2026-09-20T09:00:00Z", { requests: 20, errors: 1 })] },
    );
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByRole("heading", { name: "Requests per hour, UTC" })).toBeInTheDocument();
    await waitFor(() => expect(tab(/^Requests/)).toHaveTextContent(/↑100%.*40/));
    expect(requests("GET", `${USAGE}/timeline`).map((r) => r.query.get("step"))).toContain("hour");
    expect(cell("Requests per hour, UTC")).toHaveTextContent(/This period.*40.*Failed requests.*4.*vs\. prior 14 days.*20/);
    expect(tab(/^Error rate/)).toHaveTextContent("4 failed");
    const keys = within(cell("Keys by requests")).getByRole("list");
    await waitFor(() =>
      expect(
        within(keys)
          .getAllByRole("button")
          .map((b) => b.textContent),
      ).toEqual([expect.stringContaining("checkout-web"), expect.stringContaining("Playground")]),
    );
    expect(within(cell("Models by requests")).getAllByRole("button")[2]).toHaveTextContent("No model");

    await user.click(tab(/^Tokens/));
    expect(await screen.findByRole("heading", { name: "Tokens per hour, UTC" })).toBeInTheDocument();
    expect(cell("Tokens per hour, UTC")).toHaveTextContent(/Input.*Cache read.*Cache write.*Output.*Reasoning/);
    expect(screen.getByRole("heading", { name: "Models by tokens" })).toBeInTheDocument();

    await user.click(tab(/^Cost/));
    expect(await screen.findByRole("heading", { name: "Models by cost" })).toBeInTheDocument();
    expect(cell("Models by cost")).toHaveTextContent(/Claude Sonnet 4\.5.*75%.*\$1\.50/);

    await user.click(tab(/^Error rate/));
    expect(await screen.findByRole("heading", { name: "Failed requests per hour, UTC" })).toBeInTheDocument();
    expect(cell("Models by failed requests")).toHaveTextContent(/Claude Sonnet 4\.5.*10%.*3/);

    await user.click(tab(/^Tool calls/));
    expect(await screen.findByRole("heading", { name: "Models by tool calls" })).toBeInTheDocument();

    await user.click(within(cell("Models by tool calls")).getByRole("button", { name: /gpt-5\.4-mini/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("logs")));
    expect(router.state.location.searchStr).toBe("?model=gpt-5.4-mini");
  });

  it("counts the requests the spend leaves out because no price covers them", async () => {
    signedIn();
    projectData({ models: [catalogueModel()], keys: [apiKey()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, { ...THIS_PERIOD, day: [usageRow({ key: "2026-10-13", requests: 40, cost: "2", unpriced: 3 })] }, BEFORE);
    on("GET", `${USAGE}/timeline`, () => ({ data: [bucket("2026-10-13T09:00:00Z")] }));
    await renderApp(projectPath(`analytics?${PERIOD}`));

    await waitFor(() => expect(cell("Spend")).toHaveTextContent(/\$2\.00.*3 requests unknown/));
  });

  it("opens the request logs for a key from its rank", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage(USAGE, THIS_PERIOD);
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}&metric=tokens`));
    expect(await screen.findByRole("heading", { name: "Tokens per day, UTC" })).toBeInTheDocument();
    const keys = within(cell("Keys by tokens")).getByRole("list");
    await user.click(await within(keys).findByRole("button", { name: /checkout-web/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?key=key_web"));
  });

  describe("on the current day", () => {
    atFixtureTime();

    it("charts today by the minute", async () => {
      signedIn();
      projectData();
      on("GET", "orgs/:org/projects/:project/logs", listed([]));
      usage(USAGE, { day: [usageRow({ key: "2026-10-15", requests: 40 })] }, {}, "2026-10-15");
      on("GET", `${USAGE}/timeline`, { data: [bucket("2026-10-15T11:57:00Z", { cost: null }), bucket("2026-10-15T11:58:00Z")] });
      await renderApp(projectPath("analytics?period=today"));

      expect(await screen.findByRole("heading", { name: "Requests per minute, UTC" })).toBeInTheDocument();
      await waitFor(() => expect(tab(/^Requests/)).toHaveTextContent(/40.*So far today/));
      expect(requests("GET", `${USAGE}/timeline`).map((r) => r.query.get("step"))).toContain("minute");
    });

    it("sets today against the 13 days before when request logs are off", async () => {
      signedIn({ session: whoAmI({ request_logs: false }) });
      projectData();
      usage(
        USAGE,
        { day: [usageRow({ key: "2026-10-15", requests: 40 })] },
        { day: [usageRow({ key: "2026-10-10", requests: 12 }), usageRow({ key: "2026-10-15", requests: 40 })] },
        "2026-10-15",
      );
      const { user } = await renderApp(projectPath("analytics?period=today"));

      expect(await screen.findByRole("heading", { name: "Requests per day, the last 14 days, UTC" })).toBeInTheDocument();
      expect(requests("GET", `${USAGE}/timeline`)).toHaveLength(0);
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
    });

    it("forecasts the month's spend, shows the budget and the split by model, and opens a recent error", async () => {
      signedIn();
      projectData({ limits: [limit()], models: [catalogueModel()] });
      on(
        "GET",
        "orgs/:org/projects/:project/logs",
        listed([failedLog(), failedLog({ id: "log_2", status: 500, error_kind: null, requested_model: "fast-alias" })]),
      );
      on("GET", `${USAGE}/timeline`, { data: [] });
      const priced = ["a", "b", "c", "d", "e"].map((m, i) => usageRow({ key: `openai/model-${m}`, requests: 10, cost: String(5 - i) }));
      usage(
        USAGE,
        {
          day: [usageRow({ key: "2026-10-01", cost: "3" }), usageRow({ key: "2026-10-10", cost: "7" }), usageRow({ key: "2026-10-15", cost: "1" })],
          model: priced,
        },
        {},
        "2026-09-16",
      );
      const { user, router } = await renderApp(projectPath("analytics?period=30d"));

      const spend = (await screen.findByRole("heading", { name: "Forecast for October" })).closest("section") as HTMLElement;
      await waitFor(() => expect(spend).toHaveTextContent("$11.00 so far, at the last 7 days' pace"));
      expect(spend).toHaveTextContent("$12.50 of $50.00");
      expect(within(spend).getByRole("link", { name: "Limits" })).toHaveAttribute("href", projectPath("limits"));
      expect(within(spend).getByRole("img", { name: /^Cost by model:/ })).toHaveAccessibleName(/model-a.*model-b.*model-c.*Other \$3\.00/);

      const failed = cell("Failed requests");
      expect(await within(failed).findByRole("button", { name: /fast-alias/ })).toBeInTheDocument();
      await user.click(within(failed).getByRole("button", { name: /429/ }));
      await waitFor(() => expect(router.state.location.searchStr).toBe("?log=log_1"));
    });
  });

  it("ranks an organization's projects and opens one from either list", async () => {
    signedIn();
    usage(ORG_USAGE, {
      project: [orgRow(PROJECT, { requests: 30, errors: 3 }), orgRow(OTHER_PROJECT, { requests: 10, errors: 1 }), orgRow("proj_gone", { requests: 1 })],
      model: [orgRow("anthropic/claude-sonnet-4-5", { requests: 30 })],
    });
    on("GET", `${ORG_USAGE}/timeline`, { data: [] });
    const { user, router } = await renderApp(`/orgs/${ORG}/analytics?${PERIOD}&metric=errors`);

    expect(await screen.findByRole("heading", { name: "Projects by failed requests" })).toBeInTheDocument();
    await user.click(tab(/^Requests/));
    const failed = cell("Failed requests by project");
    await waitFor(() => expect(within(failed).getAllByRole("button")[0]).toHaveTextContent(/Checkout assistant.*10%/));
    expect(within(cell("Projects by requests")).getAllByRole("button")[2]).toHaveTextContent("proj_gone");
    expect(within(cell("Models by requests")).queryByRole("button")).not.toBeInTheDocument();
    await user.click(within(failed).getByRole("button", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));

    await router.navigate({ href: `/orgs/${ORG}/analytics?${PERIOD}` });
    await user.click(await within(cell("Projects by requests")).findByRole("button", { name: /Checkout assistant/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}`));
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
