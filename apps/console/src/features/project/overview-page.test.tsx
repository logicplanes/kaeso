import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { UsageRow } from "~/api/generated/types";
import { signedIn, without } from "~/test/fixtures";
import { apiKey, catalogueModel, failedLog, limit, listed, projectData, projectMember, projectPath, serviceAccount, usageRow } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const USAGE = "orgs/:org/projects/:project/usage";
const PERIOD = "from=2026-10-01&to=2026-10-14";
const breakdown = () => screen.findByRole("table", { name: "Usage breakdown" });
const bodyRows = (table: HTMLElement) => within(table).getAllByRole("row").slice(1);

function usage(by: Partial<Record<string, UsageRow[] | Response>> = {}): void {
  on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
  on("GET", USAGE, ({ query }: { query: URLSearchParams }) => {
    const rows = by[query.get("group_by") ?? "day"] ?? [];
    return rows instanceof Response ? rows : { data: rows };
  });
}

describe("OverviewPage", () => {
  it("breaks usage down by model with names from the catalogue", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({
      day: [usageRow({ key: "2026-10-13" })],
      model: [
        usageRow({ key: "openai/gpt-5.4-mini", requests: 40, errors: 4, cost: null }),
        usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 120, errors: 0, cost: "2.5" }),
        usageRow({ key: "anthropic/", requests: 3, errors: 0, cost: "0.001" }),
      ],
    });
    await renderApp(projectPath(`?${PERIOD}`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(3));
    const [first, second, third] = bodyRows(table);
    expect(first).toHaveTextContent("Claude Sonnet 4.5");
    expect(first).toHaveTextContent("120");
    expect(first).toHaveTextContent("$2.50");
    expect(second).toHaveTextContent("gpt-5.4-mini");
    expect(second).toHaveTextContent("—");
    expect(third).toHaveTextContent("No model");
    expect(third).toHaveTextContent("<$0.01");
    expect(within(table).getByRole("columnheader", { name: "Model" })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "Cost" })).toBeInTheDocument();
    expect(screen.getByText("Requests, errors, tokens and cost in the period")).toBeInTheDocument();
    expect((await screen.findAllByText("Cost by model")).length).toBeGreaterThan(0);
    expect(requests("GET", USAGE).map((r) => Object.fromEntries(r.query))).toContainEqual({ from: "2026-10-01", to: "2026-10-14", group_by: "model" });
  });

  it("groups by key, member and day from the segmented control", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()] });
    usage({
      day: [usageRow({ key: "2026-10-12", requests: 5 }), usageRow({ key: "2026-10-13", requests: 9 })],
      key: [usageRow({ key: "key_web", requests: 10 }), usageRow({ key: "playground", requests: 4 }), usageRow({ key: "key_gone", requests: 1 })],
      member: [
        usageRow({ key: "user:usr_grace", requests: 8 }),
        usageRow({ key: "service_account:sa_deploy", requests: 6 }),
        usageRow({ key: "user:usr_ada", requests: 2 }),
      ],
    });
    const { user, router } = await renderApp(projectPath(`?${PERIOD}`));

    await breakdown();
    await user.click(screen.getByRole("radio", { name: "By key" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ by: "key" }));
    let table = await breakdown();
    await waitFor(() =>
      expect(bodyRows(table).map((r) => r.textContent)).toEqual([
        expect.stringContaining("checkout-web"),
        expect.stringContaining("Playground"),
        expect.stringContaining("key_gone"),
      ]),
    );
    expect(within(table).getByRole("columnheader", { name: "Key" })).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "By member" }));
    table = await breakdown();
    await waitFor(() =>
      expect(bodyRows(table).map((r) => r.textContent)).toEqual([
        expect.stringContaining("Grace Hopper"),
        expect.stringContaining("ci-deploy (service account)"),
        expect.stringContaining("Ada Okafor (you)"),
      ]),
    );
    expect(within(table).getByRole("columnheader", { name: "Member" })).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "By day" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ by: "day" }));
    table = await breakdown();
    await waitFor(() => expect(bodyRows(table).map((r) => r.textContent)).toEqual([expect.stringContaining("Oct 13"), expect.stringContaining("Oct 12")]));
    expect(within(table).getByRole("columnheader", { name: "Day" })).toBeInTheDocument();
  });

  it("says when the period has no requests, and offers a retry when the rows fail", async () => {
    signedIn();
    projectData();
    let calls = 0;
    on("GET", `${USAGE}/span`, { first_day: null, last_day: null });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "key" && ++calls === 1 ? apiError(403, "permission_denied", "Usage by key is hidden.") : { data: [] },
    );
    const { user } = await renderApp(projectPath(`?${PERIOD}&by=key`));

    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    await user.click(within(await breakdown()).getByRole("button", { name: "Try again" }));
    expect(await within(await breakdown()).findByText("No requests in this period.")).toBeInTheDocument();
  });

  it("offers a retry across the whole row when the rows fail without the right to see cost", async () => {
    signedIn({ projectRights: without("costs.view") });
    projectData();
    usage({ model: apiError(500, "internal", "Usage is down.") });
    await renderApp(projectPath(`?${PERIOD}`));

    const table = await breakdown();
    expect(await within(table).findByText("Couldn't load these rows")).toBeInTheDocument();
    expect(within(table).getByText("Couldn't load these rows").closest("td")).toHaveAttribute("colspan", "4");
  });

  it("leaves cost out without the right to see it", async () => {
    signedIn({ projectRights: without("costs.view", "models.view") });
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5" })] });
    await renderApp(projectPath(`?${PERIOD}`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)[0]).toHaveTextContent("claude-sonnet-4-5"));
    expect(bodyRows(table)[0]).not.toHaveTextContent("Claude Sonnet 4.5");
    expect(within(table).queryByRole("columnheader", { name: "Cost" })).not.toBeInTheDocument();
    expect(screen.getByText("Requests, errors and tokens in the period")).toBeInTheDocument();
    expect(screen.queryByText("Cost by model")).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/models")).toHaveLength(0);
  });

  it("shows the project's limits, recent errors and counts beside the charts", async () => {
    signedIn();
    projectData({
      models: [catalogueModel()],
      keys: [apiKey(), apiKey({ id: "key_old", state: "archived" })],
      limits: [
        limit(),
        limit({ id: "lim_rate", kind: "request_rate", period: "minute", amount: "60", used: "12", resets_at: null }),
        limit({ id: "lim_tokens", kind: "token_budget", period: "total", amount: "1000000", used: null, resets_at: null }),
      ],
    });
    on("GET", "orgs/:org/projects/:project/logs", listed([failedLog(), failedLog({ id: "log_2", status: 500, error_kind: null, requested_model: "gpt-x" })]));
    usage();
    const { user, router } = await renderApp(projectPath(`?${PERIOD}`));

    expect(await screen.findByText("$12.50 of $50.00")).toBeInTheDocument();
    expect(screen.getByText("12 of 60")).toBeInTheDocument();
    expect(screen.getByText("Over the last minute")).toBeInTheDocument();
    expect(screen.getByText("0 of 1,000,000")).toBeInTheDocument();
    expect(screen.getByText("Never resets")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Limits" }).closest("section")).toHaveTextContent(/Resets \S/);

    const errors = screen.getByRole("grid", { name: "Recent errors" });
    expect(within(errors).getByRole("gridcell", { name: /Rate limited/ })).toBeInTheDocument();
    expect(within(errors).getByRole("gridcell", { name: /gpt-x/ })).toBeInTheDocument();
    const project = screen.getByRole("grid", { name: "This project" });
    expect(within(project).getByRole("gridcell", { name: /Models/ })).toHaveTextContent("1");
    expect(within(project).getByRole("gridcell", { name: /Keys/ })).toHaveTextContent("1");
    expect(requests("GET", "orgs/:org/projects/:project/limits")[0]?.query.get("level")).toBe("project");
    expect(Object.fromEntries(requests("GET", "orgs/:org/projects/:project/logs")[0]?.query ?? [])).toEqual({ failed: "true", limit: "5" });

    await user.click(within(errors).getByRole("gridcell", { name: /Rate limited/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("logs")));
    expect(router.state.location.search).toEqual({ log: "log_1" });
  });

  it("opens the keys from the project's counts, which include every page of active keys", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/keys", ({ query }: { query: URLSearchParams }) =>
      query.get("after")
        ? listed([apiKey({ id: "key_two", name: "second-page" }), apiKey({ id: "key_old", name: "archived", state: "archived" })])
        : listed([apiKey()], "cur_2"),
    );
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage();
    const { user, router } = await renderApp(projectPath(`?${PERIOD}`));

    expect(await screen.findByText("No project limits. Keys and members can still have their own.")).toBeInTheDocument();
    expect(screen.getByText("No failed requests.")).toBeInTheDocument();
    const keys = within(screen.getByRole("grid", { name: "This project" })).getByRole("gridcell", { name: /Keys/ });
    expect(keys).toHaveTextContent("2");
    await user.click(keys);
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("keys")));
  });

  it("hides the limits, errors and key count without the rights", async () => {
    signedIn({ projectRights: without("limits.view", "logs.metadata.view", "keys.all.view") });
    projectData({ models: [catalogueModel()] });
    usage();
    await renderApp(projectPath(`?${PERIOD}`));

    const project = await screen.findByRole("grid", { name: "This project" });
    expect(within(project).getByRole("gridcell", { name: /Models/ })).toBeInTheDocument();
    expect(within(project).queryByRole("gridcell", { name: /Keys/ })).not.toBeInTheDocument();
    expect(screen.queryByText("Limits")).not.toBeInTheDocument();
    expect(screen.queryByText("Recent errors")).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/limits")).toHaveLength(0);
    expect(requests("GET", "orgs/:org/projects/:project/logs")).toHaveLength(0);
  });

  it("leaves out the project's counts when neither can be seen", async () => {
    signedIn({ projectRights: without("limits.view", "logs.metadata.view", "keys.all.view", "models.view") });
    projectData();
    usage();
    await renderApp(projectPath(`?${PERIOD}`));

    await screen.findByRole("table", { name: "Usage breakdown" });
    expect(screen.queryByText("This project")).not.toBeInTheDocument();
  });

  it("reads usage from the first day when the whole span is chosen", async () => {
    signedIn();
    projectData();
    usage();
    await renderApp(projectPath("?period=all"));

    await breakdown();
    await waitFor(() => expect(requests("GET", USAGE).map((r) => r.query.get("from"))).toContain("2026-09-01"));
    expect(requests("GET", USAGE).every((r) => r.query.get("from") === "2026-09-01")).toBe(true);
  });

  it("loads the daily usage again from a chart that couldn't load", async () => {
    signedIn();
    projectData();
    let failing = true;
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "day" && failing
        ? apiError(403, "permission_denied", "Usage is hidden for now.")
        : { data: [usageRow({ key: "2026-10-13", requests: 7 })] },
    );
    const { user } = await renderApp(projectPath(`?${PERIOD}&by=day`));

    const requestsChart = (await screen.findByRole("heading", { name: "Requests" })).closest("section") as HTMLElement;
    expect(await within(requestsChart).findByText("Couldn't load this chart")).toBeInTheDocument();
    expect(await within(await breakdown()).findByText("Couldn't load these rows")).toBeInTheDocument();
    failing = false;
    await user.click(within(requestsChart).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(within(requestsChart).queryByText("Couldn't load this chart")).not.toBeInTheDocument());
    await waitFor(() => expect(bodyRows(screen.getByRole("table", { name: "Usage breakdown" }))[0]).toHaveTextContent("Oct 13"));
  });

  it("loads the daily rows again from the breakdown", async () => {
    signedIn();
    projectData();
    let failing = true;
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "day" && failing
        ? apiError(403, "permission_denied", "Usage is hidden for now.")
        : { data: [usageRow({ key: "2026-10-13", requests: 7 })] },
    );
    const { user } = await renderApp(projectPath(`?${PERIOD}&by=day`));

    expect(await within(await breakdown()).findByText("Couldn't load these rows")).toBeInTheDocument();
    failing = false;
    await user.click(within(await breakdown()).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(bodyRows(screen.getByRole("table", { name: "Usage breakdown" }))[0]).toHaveTextContent("Oct 13"));
  });

  it("loads the cost by model again when it couldn't load", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    let calls = 0;
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "model" && ++calls === 1
        ? apiError(403, "permission_denied", "Usage is hidden for now.")
        : { data: query.get("group_by") === "model" ? [usageRow({ key: "anthropic/claude-sonnet-4-5", cost: "2.5" })] : [] },
    );
    const { user } = await renderApp(projectPath(`?${PERIOD}`));

    const share = (await screen.findByRole("heading", { name: "Cost by model" })).closest("section") as HTMLElement;
    expect(await within(share).findByText("Couldn't load this chart")).toBeInTheDocument();
    await user.click(within(share).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(within(share).queryByText("Couldn't load this chart")).not.toBeInTheDocument());
    expect(calls).toBe(2);
  });

  it("chooses a period from the picker", async () => {
    signedIn();
    projectData();
    usage();
    const { user, router } = await renderApp(projectPath(`?${PERIOD}`));

    await breakdown();
    await user.click(screen.getByRole("button", { name: "Period" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose a date range" });
    await user.click(within(dialog).getByRole("option", { name: "Last 7 days" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ period: "7d" }));
  });
});
