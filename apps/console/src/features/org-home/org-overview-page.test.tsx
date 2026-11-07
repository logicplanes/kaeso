import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { atFixtureTime } from "~/test/clock";
import { ORG, OTHER_PROJECT, PROJECT, project, signedIn, withRights } from "~/test/fixtures";
import { credential, invite, member, OTHER_USER, usageRow } from "~/test/fixtures/org";
import { mcpServer, obj } from "~/test/fixtures/mcp";
import { auditEvent } from "~/test/fixtures/shell";
import { searchHit } from "~/test/fixtures/project";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

function usage() {
  on("GET", "orgs/:org/usage", ({ query }: { query: URLSearchParams }) => ({
    data: query.get("group_by") === "project" ? [usageRow(PROJECT, { requests: 30 }), usageRow(OTHER_PROJECT, { requests: 10 })] : [],
  }));
  on("GET", "orgs/:org/usage/timeline", { data: [] });
  on("GET", "orgs/:org/projects/:project/usage", ({ params }: { params: Record<string, string> }) => ({
    data: params.project === PROJECT ? [usageRow("2026-10-15", { requests: 12 })] : [usageRow("2026-10-15", { requests: 3 })],
  }));
}

function around() {
  on(
    "GET",
    "orgs/:org/credentials",
    page([
      credential({ last_used_at: "2026-10-15T11:00:00Z" }),
      credential({ id: "cred_chatgpt", name: "", provider: "anthropic", auth_error: "signed_out" }),
      credential({ id: "cred_plan", name: "Team plan", auth_error: "no_api_access" }),
      credential({ id: "cred_idle", name: "Idle" }),
    ]),
  );
  on("GET", "orgs/:org/mcp-servers", page([mcpServer(), mcpServer({ id: "mcp_docs", name: "Docs", status: "unreachable" })]));
  on("GET", "orgs/:org/members", page([member(), member({ user: { id: OTHER_USER, name: "Grace Hopper", avatar_url: "https://avatars.example/grace.png" } })]));
  on("GET", "orgs/:org/invites", page([invite()]));
  on(
    "GET",
    "orgs/:org/audit-log",
    page([
      auditEvent({ action: "key.create", after: obj({ name: "checkout-web" }) }),
      auditEvent({ id: "evt_2", action: "key.create", actor: { kind: "service_account", id: "sa_ci" }, target_kind: null, target_id: null }),
    ]),
  );
}

function models(n: number) {
  on("GET", "orgs/:org/projects/:project/models/search", {
    data: Array.from({ length: n }, (_, i) =>
      searchHit({ model_id: `model-${i}`, name: `Model ${i}`, release_date: `2026-10-${String(14 - i).padStart(2, "0")}` }),
    ),
    has_more: false,
    not_in_catalogue: 0,
    total: n,
  });
}

describe("the organization overview", () => {
  atFixtureTime();

  it("leads with the release this install runs and opens what's new from it", async () => {
    signedIn();
    usage();
    models(0);
    const { user } = await renderApp(`/orgs/${ORG}`);
    const news = await screen.findByRole("group", { name: "News" });
    expect(news).toHaveTextContent("Running now");
    expect(news).toHaveTextContent("Kaeso 40.4.2");
    expect(news).toHaveTextContent("Search on every list.");
    await user.click(within(news).getByRole("button", { name: "Read what's new" }));
    expect(await screen.findByRole("dialog")).toHaveTextContent("40.4.2");
  });

  it("lists the projects by today's requests and opens one", async () => {
    signedIn();
    usage();
    models(0);
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("list", { name: "Projects" });
    await waitFor(() => expect(within(list).getAllByRole("button")[0]).toHaveTextContent(/Checkout assistant.*12/));
    expect(within(list).getAllByRole("button")[1]).toHaveTextContent(/Support triage.*3/);
    await user.click(within(list).getByRole("button", { name: /Support triage/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("shows the two newest models as cards and every new one in a dialog", async () => {
    signedIn();
    usage();
    models(8);
    const { user } = await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Model 0" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Model 1" })).toBeInTheDocument();
    const more = screen.getByRole("list", { name: "More new models" });
    expect(within(more).getAllByRole("button")).toHaveLength(4);
    expect(requests("GET", "orgs/:org/projects/:project/models/search")[0]?.query.get("current")).toBe("true");
    await user.click(screen.getByRole("button", { name: "See all 8" }));
    const dialog = await screen.findByRole("dialog", { name: "New models" });
    expect(within(dialog).getAllByRole("button", { name: /Model \d/ })).toHaveLength(8);
  });

  it("lists only the projects for people who can't see the organization's usage", async () => {
    withRights(["org.view"]);
    models(0);
    await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("list", { name: "Projects" });
    await waitFor(() =>
      expect(
        within(list)
          .getAllByRole("button")
          .map((b) => b.textContent),
      ).toEqual(["CCheckout assistant", "SSupport triage"]),
    );
    expect(screen.queryByRole("heading", { name: "Activity" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/usage")).toHaveLength(0);
  });

  it("offers to create the first project in an organization without projects", async () => {
    signedIn({ projects: [] });
    usage();
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("No projects you can see")).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "Create project" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects`));
    expect(await screen.findByRole("dialog", { name: "Create project" })).toBeInTheDocument();
  });

  it("doesn't offer to create a project without the right", async () => {
    withRights(["org.view"]);
    on("GET", "orgs/:org/projects", page([]));
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("No projects you can see")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("adds up this month's cost by project beside the month's spend", async () => {
    signedIn();
    on("GET", "orgs/:org/usage", ({ query }: { query: URLSearchParams }) => ({
      data:
        query.get("group_by") === "project"
          ? [
              usageRow(PROJECT, { cost: "4" }),
              usageRow(OTHER_PROJECT, { cost: "1" }),
              usageRow("proj_gone", { cost: "0.5" }),
              usageRow("proj_free", { cost: "0" }),
            ]
          : [],
    }));
    on("GET", "orgs/:org/usage/timeline", { data: [] });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    models(0);
    await renderApp(`/orgs/${ORG}`);
    const bar = await screen.findByRole("img", { name: /^Cost by project:/ });
    expect(bar).toHaveAccessibleName(/Checkout assistant \$4\.00.*Support triage \$1\.00.*proj_gone \$0\.50/);
    expect(bar).not.toHaveAccessibleName(/proj_free/);
  });

  it("lists provider keys by last use with the ones that need attention", async () => {
    signedIn();
    usage();
    models(0);
    around();
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("list", { name: "Providers" });
    await waitFor(() => expect(within(list).getAllByRole("button")).toHaveLength(4));
    const [used, signedOut, plan, idle] = within(list).getAllByRole("button");
    expect(used).toHaveTextContent(/OpenAI.*OpenAI production.*1 h ago/);
    expect(signedOut).toHaveTextContent("Signed out");
    expect(plan).toHaveTextContent(/Team plan.*No API access/);
    expect(idle).toHaveTextContent("Never used");
    expect(screen.getByText("4 provider keys")).toBeInTheDocument();
    await user.click(idle as HTMLElement);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/providers`));
  });

  it("shows how each MCP server is doing and opens one", async () => {
    signedIn();
    usage();
    models(0);
    around();
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("list", { name: "MCP servers" });
    await waitFor(() => expect(within(list).getAllByRole("button")).toHaveLength(2));
    expect(within(list).getAllByRole("button")[0]).toHaveTextContent(/X.*2 min ago/);
    expect(within(list).getAllByRole("button")[1]).toHaveTextContent(/Docs.*Unreachable/);
    await user.click(within(list).getByRole("button", { name: /Docs/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?server=mcp_docs"));
    expect(router.state.location.pathname).toBe(`/orgs/${ORG}/mcp-servers`);
  });

  it("counts the members and the people invited, and opens the member settings", async () => {
    signedIn();
    usage();
    models(0);
    around();
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("list", { name: "Members" });
    await waitFor(() => expect(within(list).getAllByRole("button")).toHaveLength(2));
    expect(within(list).getAllByRole("button")[0]).toHaveTextContent(/Ada Okafor \(you\).*Owner/);
    expect(screen.getByText("2 members, 1 invited")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "2 members, 1 invited" })).toHaveAttribute("href", `/orgs/${ORG}/analytics/members`);
    await user.click(within(list).getByRole("button", { name: /Grace Hopper/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/members`));
  });

  it("lists recent changes by who made them and opens one in the audit log", async () => {
    signedIn();
    usage();
    models(0);
    around();
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    const list = await screen.findByRole("list", { name: "Recent changes" });
    await waitFor(() => expect(within(list).getAllByRole("button")).toHaveLength(2));
    expect(within(list).getAllByRole("button")[0]).toHaveTextContent("Ada Okafor · Key created · checkout-web");
    expect(within(list).getAllByRole("button")[1]).toHaveTextContent("Service account · Key created");
    await user.click(within(list).getAllByRole("button")[0] as HTMLElement);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/settings/orgs/${ORG}/audit-log`));
    expect(router.state.location.searchStr).toBe("?event=evt_1");
  });

  it("leaves out the new models when they can't load", async () => {
    signedIn();
    usage();
    on("GET", "orgs/:org/projects/:project/models/search", apiError(500, "api_error", "The catalogue is down."));
    await renderApp(`/orgs/${ORG}`);
    await screen.findByRole("list", { name: "Projects" });
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/models/search")).toHaveLength(1));
    expect(screen.queryByRole("heading", { name: "More new models" })).not.toBeInTheDocument();
  });

  it("announces a release newer than the one running", async () => {
    signedIn();
    usage();
    models(0);
    on("GET", "me/whats-new", {
      current: "40.4.2",
      seen: "40.4.2",
      releases: [
        {
          version: "40.5.0",
          published_at: "2026-10-14T12:00:00Z",
          prerelease: false,
          notes: "## Added",
          url: "https://github.com/logicplanes/kaeso/releases/tag/v40.5.0",
          art: { version: 1, seed: 42, style: "albers", palette: 3 },
        },
        { version: "40.4.1", published_at: null, prerelease: false, notes: "", url: "https://github.com/logicplanes/kaeso/releases/tag/v40.4.1", art: null },
      ],
      running: {
        version: "40.4.2",
        published_at: "2026-10-01T12:00:00Z",
        prerelease: false,
        notes: "Fixes.",
        url: "https://github.com/logicplanes/kaeso/releases/tag/v40.4.2",
        art: null,
      },
    });
    await renderApp(`/orgs/${ORG}`);
    const news = await screen.findByRole("group", { name: "News", hidden: true });
    expect(news).toHaveTextContent(/New release.*Kaeso 40\.5\.0/);
    expect(news).not.toHaveTextContent("Added");
  });

  it("cards models without a date, a context or prices, and opens the catalogue from the list and the dialog", async () => {
    signedIn({ projects: [project({ avatar_url: "https://avatars.example/checkout.png" })] });
    usage();
    on("GET", "orgs/:org/projects/:project/models/search", {
      data: [
        searchHit({ model_id: "mystery", name: "", release_date: null, context_limit: null, input_price: null, catalogue_model_id: "cm_mystery" }),
        searchHit({ provider: "anthropic", model_id: "claude-old", name: "claude-old", release_date: "2026-01-02" }),
        searchHit({ provider: "anthropic", model_id: "claude-plain", name: "claude-plain", release_date: null }),
      ],
      has_more: false,
      not_in_catalogue: 0,
      total: 3,
    });
    const { user, router } = await renderApp(`/orgs/${ORG}`);
    const mystery = (await screen.findByRole("heading", { name: "mystery" })).closest("article, section") as HTMLElement;
    expect(mystery).toHaveTextContent("OpenAI");
    expect(within(mystery).getByRole("link", { name: "Open in models" })).toBeInTheDocument();
    expect(screen.queryByText("New")).not.toBeInTheDocument();
    const more = screen.getByRole("list", { name: "More new models" });
    await user.click(within(more).getByRole("button", { name: /claude-plain/ }));
    const dialog = await screen.findByRole("dialog", { name: "New models" });
    await user.click(within(dialog).getByRole("button", { name: /claude-old/ }));
    await waitFor(() => expect(router.state.location.searchStr).toBe("?provider=anthropic"));
    expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/models`);
  });

  it("links to all the projects when there are more than it lists", async () => {
    signedIn({ projects: Array.from({ length: 7 }, (_, i) => project({ id: `proj_${i}`, name: `Project ${i}` })) });
    usage();
    models(0);
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("link", { name: "All 7" })).toHaveAttribute("href", `/orgs/${ORG}/projects`);
  });
});
