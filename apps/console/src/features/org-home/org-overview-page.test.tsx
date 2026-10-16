import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { UsageRow } from "~/api/generated/types";
import { ALL_ORG_RIGHTS, ORG, OTHER_PROJECT, PROJECT, project, signedIn, withRights } from "~/test/fixtures";
import { member, notifications, serviceAccount, usageRow } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PERIOD = "from=2026-10-01&to=2026-10-07";
const USAGE = "orgs/:org/usage";

function usage(rows: Partial<Record<string, { data: UsageRow[] } | Response>>) {
  on("GET", USAGE, ({ query }: { query: URLSearchParams }) => rows[query.get("group_by") ?? ""] ?? { data: [] });
}

const wrap = (rows: UsageRow[]) => ({ data: rows });

const breakdown = () => screen.findByRole("table", { name: "Usage breakdown" });
const usageQueries = () => requests("GET", USAGE).map((r) => Object.fromEntries(r.query));

describe("organization overview", () => {
  it("shows the period's usage, the cost by project and the projects ordered by requests", async () => {
    signedIn({
      projects: [
        project({ id: OTHER_PROJECT, name: "Support triage", is_default: false }),
        project(),
        project({ id: "proj_old", name: "Old pilot", is_default: false, archived_at: "2026-09-01T00:00:00Z" }),
        project({ id: "proj_quiet", name: "Quiet archive", is_default: false, archived_at: "2026-09-01T00:00:00Z" }),
      ],
    });
    usage({
      day: wrap([usageRow("2026-10-02", { requests: 45, cost: "2.75" })]),
      project: wrap([
        usageRow(PROJECT, { requests: 30, errors: 3, cost: "2.00" }),
        usageRow(OTHER_PROJECT, { requests: 10, cost: "0.50" }),
        usageRow("proj_old", { requests: 5, errors: 5, cost: "0.25" }),
        usageRow("proj_deleted", { requests: 0, cost: null }),
      ]),
    });
    const { user, router } = await renderApp(`/orgs/${ORG}?${PERIOD}`);

    const metrics = await screen.findByRole("list", { name: "Usage in the period" });
    await waitFor(() => expect(metrics).toHaveTextContent("45"));
    expect(metrics).toHaveTextContent("$2.75");
    expect((await screen.findAllByText("Cost by project")).length).toBeGreaterThan(0);

    const table = await breakdown();
    await within(table).findByRole("link", { name: "Checkout assistant" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((r) => within(r).getAllByRole("cell")[0]?.textContent)).toEqual([
      "CCheckout assistant",
      "SSupport triage",
      "OOld pilotArchived",
      "proj_deleted",
    ]);
    expect(rows[0]).toHaveTextContent("10.0%3");
    expect(rows[2]).toHaveTextContent("100%5");
    expect(rows[3]).toHaveTextContent("—");
    expect(within(table).getByRole("link", { name: "Checkout assistant" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);

    const list = screen.getByRole("grid", { name: "Projects" });
    await waitFor(() =>
      expect(
        within(list)
          .getAllByRole("gridcell")
          .map((c) => c.textContent),
      ).toEqual(["CCheckout assistantDefault30 requests", "SSupport triage10 requests", "OOld pilotArchived5 requests"]),
    );
    expect(screen.getByText("Requests in the period; open one to work in it")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "All projects" })).toHaveAttribute("href", `/orgs/${ORG}/projects`);

    expect(usageQueries()).toEqual(
      expect.arrayContaining([
        { from: "2026-10-01", to: "2026-10-07", group_by: "day" },
        { from: "2026-09-24", to: "2026-09-30", group_by: "day" },
        { from: "2026-10-01", to: "2026-10-07", group_by: "project" },
      ]),
    );
    expect(requests("GET", "orgs/:org/projects").some((r) => r.query.get("include_archived") === "true")).toBe(true);

    await user.click(within(list).getByRole("gridcell", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("groups the breakdown by model and keeps the choice in the URL", async () => {
    signedIn();
    usage({
      model: wrap([usageRow("openai/gpt-5-mini", { requests: 20 }), usageRow("unknown", { requests: 2 })]),
    });
    const { user, router } = await renderApp(`/orgs/${ORG}?${PERIOD}`);
    await breakdown();
    await user.click(screen.getByRole("radio", { name: "By model" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ by: "model", from: "2026-10-01", to: "2026-10-07" }));
    const table = await breakdown();
    expect(await within(table).findByRole("columnheader", { name: "Model" })).toBeInTheDocument();
    expect(await within(table).findByText("No model")).toBeInTheDocument();
    expect(within(table).getAllByRole("row")[1]).toHaveTextContent("gpt-5-mini");
    expect(usageQueries()).toContainEqual({ from: "2026-10-01", to: "2026-10-07", group_by: "model" });
  });

  it("names members, former members and service accounts in the breakdown by member", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page([member(), member({ user: { id: "usr_grace", name: "Grace Hopper", email: "grace@example.com" } })]));
    on("GET", "orgs/:org/projects/:project/service-accounts", ({ params }: { params: Record<string, string> }) =>
      page(params.project === PROJECT ? [serviceAccount()] : []),
    );
    usage({
      member: wrap([
        usageRow("user:usr_ada", { requests: 50 }),
        usageRow("user:usr_grace", { requests: 40 }),
        usageRow("user:usr_gone", { requests: 30 }),
        usageRow("service_account:sa_bot", { requests: 20 }),
        usageRow("service_account:sa_lost", { requests: 10 }),
      ]),
    });
    await renderApp(`/orgs/${ORG}?${PERIOD}&by=member`);
    const table = await breakdown();
    expect(await within(table).findByText("Ada Okafor (you)")).toBeInTheDocument();
    const cells = () =>
      within(table)
        .getAllByRole("row")
        .slice(1)
        .map((r) => within(r).getAllByRole("cell")[0]?.textContent);
    await waitFor(() =>
      expect(cells()).toEqual([
        "AAda Okafor (you)",
        "GGrace Hopper",
        "Former memberusr_gone",
        "Nightly botService account in Checkout assistant",
        "Service accountsa_lost",
      ]),
    );
    expect(within(table).getByRole("columnheader", { name: "Member" })).toBeInTheDocument();
    expect(
      requests("GET", "orgs/:org/projects/:project/service-accounts")
        .map((r) => r.params.project)
        .sort(),
    ).toEqual([PROJECT, OTHER_PROJECT]);
  });

  it("names a service account by id when its project's accounts couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/members", page([member()]));
    on("GET", "orgs/:org/projects/:project/service-accounts", apiError(403, "forbidden", "The gateway failed."));
    usage({ member: wrap([usageRow("service_account:sa_bot", { requests: 20 })]) });
    await renderApp(`/orgs/${ORG}?${PERIOD}&by=member`);
    const table = await breakdown();
    expect(await within(table).findByText("sa_bot")).toBeInTheDocument();
    expect(within(table).getAllByRole("row")[1]).toHaveTextContent("Service accountsa_bot");
  });

  it("shows the pictures of projects and members", async () => {
    signedIn({ projects: [project({ avatar_url: "/avatars/checkout.webp" })] });
    on("GET", "orgs/:org/members", page([member({ user: { avatar_url: "/avatars/ada.webp" } })]));
    usage({ project: wrap([usageRow(PROJECT, { requests: 3 })]), member: wrap([usageRow("user:usr_ada", { requests: 3 })]) });
    const { user } = await renderApp(`/orgs/${ORG}?${PERIOD}`);
    const table = await breakdown();
    const link = await within(table).findByRole("link", { name: "Checkout assistant" });
    expect(link.parentElement?.querySelector('img[src="/avatars/checkout.webp"]')).not.toBeNull();
    const list = screen.getByRole("grid", { name: "Projects" });
    await waitFor(() =>
      expect(
        within(list)
          .getByRole("gridcell", { name: /Checkout assistant/ })
          .querySelector('img[src="/avatars/checkout.webp"]'),
      ).not.toBeNull(),
    );
    await user.click(screen.getByRole("radio", { name: "By member" }));
    const ada = await within(table).findByText("Ada Okafor (you)");
    expect(ada.parentElement?.querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull();
  });

  it("orders rows with as many requests by name", async () => {
    signedIn();
    usage({ model: wrap([usageRow("openai/gpt-5-mini", { requests: 20 }), usageRow("anthropic/claude-haiku", { requests: 20 })]) });
    await renderApp(`/orgs/${ORG}?${PERIOD}&by=model`);
    const table = await breakdown();
    await within(table).findByText(/claude-haiku/);
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows[0]).toHaveTextContent("claude-haiku");
    expect(rows[1]).toHaveTextContent("gpt-5-mini");
  });

  it("names projects by id when the projects couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/projects", ({ query }: { query: URLSearchParams }) =>
      query.get("include_archived") === "true" ? apiError(403, "forbidden", "The gateway failed.") : page([project()]),
    );
    usage({ project: wrap([usageRow(PROJECT, { requests: 12, cost: "1.25" })]) });
    await renderApp(`/orgs/${ORG}?${PERIOD}`);
    const table = await breakdown();
    expect(await within(table).findByText(PROJECT)).toBeInTheDocument();
    expect(within(table).queryByRole("link", { name: "Checkout assistant" })).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByText(PROJECT).some((label) => !table.contains(label))).toBe(true));
  });

  it("labels members by id when the person can't see the member list", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "members.view"));
    usage({ member: wrap([usageRow("user:usr_grace", { requests: 4 })]) });
    await renderApp(`/orgs/${ORG}?${PERIOD}&by=member`);
    const table = await breakdown();
    expect(await within(table).findByText("usr_grace")).toBeInTheDocument();
    expect(within(table).getAllByRole("row")[1]).toHaveTextContent("Memberusr_grace");
    expect(requests("GET", "orgs/:org/members")).toHaveLength(0);
  });

  it("says when the period has no requests and when the breakdown fails", async () => {
    signedIn();
    usage({ model: apiError(400, "invalid_request", "The breakdown can't group by model yet.") });
    const { user } = await renderApp(`/orgs/${ORG}?${PERIOD}`);
    const table = await breakdown();
    expect(await within(table).findByText("No requests in this period.")).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "By model" }));
    expect(await within(table).findByText("The breakdown couldn't load; the notification says why.")).toBeInTheDocument();
    expect(await notifications().findAllByText("The breakdown can't group by model yet.")).not.toHaveLength(0);
  });

  it("loads the charts again after they failed", async () => {
    signedIn();
    const refused = apiError(400, "invalid_request", "Usage is being rebuilt.");
    usage({ day: refused, project: refused });
    const { user } = await renderApp(`/orgs/${ORG}?${PERIOD}`);
    await breakdown();
    expect(await notifications().findAllByText("Usage is being rebuilt.")).not.toHaveLength(0);
    const main = screen.getByRole("main");
    await waitFor(() => expect(within(main).getAllByRole("button", { name: "Try again" })).toHaveLength(3));
    usage({ day: wrap([usageRow("2026-10-03", { requests: 12, cost: "0.40" })]), project: wrap([usageRow(PROJECT, { requests: 12, cost: "0.40" })]) });
    const before = usageQueries().length;
    await user.click(within(main).getAllByRole("button", { name: "Try again" })[0] as HTMLElement);
    await waitFor(() => expect(within(main).getAllByRole("button", { name: "Try again" })).toHaveLength(1));
    await user.click(within(main).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(within(main).queryAllByRole("button", { name: "Try again" })).toHaveLength(0));
    expect(usageQueries().slice(before)).toEqual([
      { from: "2026-10-01", to: "2026-10-07", group_by: "day" },
      { from: "2026-10-01", to: "2026-10-07", group_by: "project" },
    ]);
    expect(screen.getByRole("list", { name: "Usage in the period" })).toHaveTextContent("12");
  });

  it("leaves out costs for people who can't see them", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "costs.org.view"));
    usage({ project: wrap([usageRow(PROJECT, { requests: 3, cost: "1.00" })]) });
    await renderApp(`/orgs/${ORG}?${PERIOD}`);
    const table = await breakdown();
    await within(table).findByRole("link", { name: "Checkout assistant" });
    expect(screen.getByText("Requests, errors and tokens in the period")).toBeInTheDocument();
    expect(within(table).queryByRole("columnheader", { name: "Cost" })).not.toBeInTheDocument();
    expect(screen.queryAllByText("Cost by project")).toHaveLength(0);
    expect(table).not.toHaveTextContent("$1.00");
  });

  it("spans the empty breakdown across the columns without costs", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "costs.org.view"));
    usage({});
    await renderApp(`/orgs/${ORG}?${PERIOD}&by=model`);
    const table = await breakdown();
    const empty = await within(table).findByText("No requests in this period.");
    expect(empty.closest("td")).toHaveAttribute("colspan", "4");
  });

  it("lists only the projects for people who can't see the organization's usage", async () => {
    withRights(["org.view"]);
    await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("grid", { name: "Projects" });
    await within(list).findByRole("gridcell", { name: /Checkout assistant/ });
    expect(
      within(list)
        .getAllByRole("gridcell")
        .map((c) => c.textContent),
    ).toEqual(["CCheckout assistantDefault", "SSupport triage"]);
    expect(screen.getByText("Open one to work in it")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Usage breakdown" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Period" })).not.toBeInTheDocument();
    expect(requests("GET", USAGE)).toHaveLength(0);
    expect(requests("GET", "orgs/:org/usage/span")).toHaveLength(0);
  });

  it("starts all time at the first day with usage and compares with nothing", async () => {
    signedIn();
    on("GET", "orgs/:org/usage/span", { first_day: "2026-10-10", last_day: "2026-10-15" });
    usage({});
    await renderApp(`/orgs/${ORG}?period=all`);
    await breakdown();
    await waitFor(() => expect(usageQueries()).toContainEqual(expect.objectContaining({ from: "2026-10-10", group_by: "project" })));
    expect(usageQueries().every((q) => q.from === "2026-10-10")).toBe(true);
  });

  it("changes the period from the picker", async () => {
    signedIn();
    usage({});
    const { user, router } = await renderApp(`/orgs/${ORG}?${PERIOD}&by=model`);
    await breakdown();
    await user.click(screen.getByRole("button", { name: /Period/ }));
    await user.click(await screen.findByRole("option", { name: "Last 7 days" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ period: "7d", by: "model" }));
  });
});
