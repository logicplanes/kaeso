import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_ORG_RIGHTS, ORG, OTHER_PROJECT, project, signedIn, withRights } from "~/test/fixtures";
import { hoverReason, notifications, usageRow } from "~/test/fixtures/org";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/orgs/${ORG}/projects`;
const PROJECTS = "orgs/:org/projects";

const list = () => screen.findByRole("grid", { name: "Projects" });
const titles = async () =>
  within(await list())
    .getAllByRole("gridcell")
    .map((c) => c.textContent);

describe("organization projects", () => {
  it("lists the projects by name, each with no actions of its own", async () => {
    signedIn({
      projects: [project({ id: "proj_zeta", name: "Zeta", avatar_url: "/avatars/zeta.webp" }), project({ id: OTHER_PROJECT, name: "Alpha" }), project()],
    });
    const { user, router } = await renderApp(PATH);
    const zeta = await within(await list()).findByRole("gridcell", { name: /Zeta/ });
    expect(zeta.querySelector('img[src="/avatars/zeta.webp"]')).not.toBeNull();
    expect(await titles()).toEqual(["AAlphaCreated 15 Oct 2026", "CCheckout assistantCreated 15 Oct 2026", "ZetaCreated 15 Oct 2026"]);
    expect(within(await list()).queryByRole("button")).not.toBeInTheDocument();
    expect(requests("GET", PROJECTS)[0]?.query.get("include_archived")).toBe("true");
    await user.click(within(await list()).getByRole("gridcell", { name: /Alpha/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${OTHER_PROJECT}`));
  });

  it("says how many requests each project served in the last 30 days", async () => {
    signedIn({ projects: [project(), project({ id: OTHER_PROJECT, name: "Support triage" })] });
    on("GET", "orgs/:org/usage", { data: [usageRow(OTHER_PROJECT, { requests: 12_345 })] });
    await renderApp(PATH);
    expect(await within(await list()).findByText("12,345 requests in 30 days")).toBeInTheDocument();
    expect(within(await list()).getByText("No requests in 30 days")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/usage")[0]?.query.get("group_by")).toBe("project");
  });

  it("asks for no usage from people who can't see the organization's usage", async () => {
    withRights(["org.view"]);
    await renderApp(PATH);
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    expect(requests("GET", "orgs/:org/usage")).toHaveLength(0);
  });

  it("finds projects by name and says when none match", async () => {
    signedIn({ projects: [project(), project({ id: OTHER_PROJECT, name: "Support triage" })] });
    const { user } = await renderApp(PATH);
    await within(await list()).findByRole("gridcell", { name: /Support triage/ });
    const search = screen.getByRole("searchbox", { name: "Search projects" });
    await user.type(search, "SUPPORT");
    await waitFor(() => expect(within(screen.getByRole("grid", { name: "Projects" })).getAllByRole("gridcell")).toHaveLength(1));
    await user.clear(search);
    await user.type(search, "billing");
    expect(await screen.findByText("No projects match")).toBeInTheDocument();
    expect(screen.getByText("Search by another project name.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(await within(await list()).findByRole("gridcell", { name: /Support triage/ })).toBeInTheDocument();
  });

  it("lists only the archived projects under Archived, after Active and before All", async () => {
    signedIn({ projects: [project(), project({ id: "proj_old", name: "Old pilot", archived_at: "2026-09-01T00:00:00Z" })] });
    const { user } = await renderApp(PATH);
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Active", "Archived", "All"]);
    const headers = screen.getAllByRole("banner").filter((h) => within(h).queryByRole("heading", { name: "Projects" }));
    expect(headers.map((h) => within(h).getAllByRole("tab").length)).toEqual([3]);
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await within(await list()).findByRole("gridcell", { name: /Old pilot/ })).toBeInTheDocument();
    expect(within(await list()).queryByRole("gridcell", { name: /Checkout assistant/ })).not.toBeInTheDocument();
  });

  it("says when no project is archived", async () => {
    signedIn();
    const { user } = await renderApp(PATH);
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await screen.findByText("No archived projects")).toBeInTheDocument();
  });

  it("shows archived projects under All", async () => {
    signedIn();
    on("GET", PROJECTS, ({ query }: { query: URLSearchParams }) =>
      page(
        query.get("include_archived") === "true"
          ? [project(), project({ id: "proj_old", name: "Old pilot", archived_at: "2026-09-01T00:00:00Z" })]
          : [project()],
      ),
    );
    const { user } = await renderApp(PATH);
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    expect(screen.getByRole("tab", { name: "Active" })).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("tab", { name: "All" }));
    const old = await within(await list()).findByRole("gridcell", { name: /Old pilot/ });
    expect(old).toHaveTextContent("Archived");
    expect(old).toHaveTextContent("Archived 1 Sep 2026");
  });

  it("says when there are no projects to see", async () => {
    withRights(["org.view"]);
    on("GET", PROJECTS, page([]));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No projects you can see")).toBeInTheDocument();
    expect(screen.getByText("Projects you belong to are listed here.")).toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Create project" });
    await hoverReason(user, create, "You need Create projects; ask an owner or admin for it.");
    await user.click(create);
    expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("says when the projects couldn't load", async () => {
    signedIn();
    on("GET", PROJECTS, apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("The projects couldn't load")).toBeInTheDocument();
    expect(screen.queryByText("No projects you can see")).not.toBeInTheDocument();
  });

  it("creates a project with the trimmed name and opens it", async () => {
    signedIn();
    on("POST", PROJECTS, ({ body }: { body: { name: string } }) => project({ id: "proj_new", name: body.name }));
    const { user, router } = await renderApp(PATH);
    await list();
    await user.click(screen.getByRole("button", { name: "Create project" }));
    const dialog = await screen.findByRole("dialog", { name: "Create project" });
    expect(within(dialog).getByText("You become its admin and can add members, models and keys inside it.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Create project" }), "Give the project a name");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "  Billing bot  ");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await notifications().findByText("Billing bot created")).toBeInTheDocument();
    expect(requests("POST", PROJECTS).map((r) => r.body)).toEqual([{ name: "Billing bot" }]);
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/proj_new`));
  });

  it("keeps the dialog open and shows the API's message when creating fails", async () => {
    signedIn();
    on("POST", PROJECTS, apiError(409, "conflict", "A project with this name exists."));
    const { user } = await renderApp(PATH);
    await list();
    await user.click(screen.getByRole("button", { name: "Create project" }));
    const dialog = await screen.findByRole("dialog", { name: "Create project" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Checkout assistant");
    await user.click(within(dialog).getByRole("button", { name: "Create project" }));
    expect(await notifications().findAllByText("A project with this name exists.")).not.toHaveLength(0);
    expect(screen.getByRole("dialog", { name: "Create project" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument());
  });

  it("opens the create dialog from ?create=true and drops it from the URL on close", async () => {
    signedIn();
    const { user, router } = await renderApp(`${PATH}?create=true`);
    const dialog = await screen.findByRole("dialog", { name: "Create project" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("keeps the order in the URL when the create dialog closes", async () => {
    signedIn();
    const { user, router } = await renderApp(`${PATH}?create=true&sort=requests`);
    const dialog = await screen.findByRole("dialog", { name: "Create project" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ sort: "requests" }));
  });

  it("opens the projects from their old settings address without the create dialog", async () => {
    signedIn();
    const { router } = await renderApp(`/settings/orgs/${ORG}/projects`);
    await waitFor(() => expect(router.state.location.pathname).toBe(PATH));
    expect(router.state.location.search).toEqual({});
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("ignores ?create=true for people who can't create projects", async () => {
    withRights(["org.view"]);
    await renderApp(`${PATH}?create=true`);
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    expect(screen.queryByRole("dialog", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("creates a project with the c shortcut", async () => {
    signedIn();
    const { user } = await renderApp(PATH);
    await within(await list()).findByRole("gridcell", { name: /Checkout assistant/ });
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: "Create project" })).toBeInTheDocument();
  });

  it("offers to create the first project in an organization without projects", async () => {
    signedIn({ projects: [] });
    on("POST", PROJECTS, ({ body }: { body: { name: string } }) => project({ id: "proj_new", name: body.name }));
    const { user, router } = await renderApp(PATH);
    expect(await screen.findByText("No projects you can see")).toBeInTheDocument();
    const [, inEmpty] = screen.getAllByRole("button", { name: "Create project" });
    await user.click(inEmpty as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Create project" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Checkout assistant");
    await user.click(within(dialog).getByRole("button", { name: "Create project" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/proj_new`));
  });

  describe("sorted by usage", () => {
    const FOUR = [
      project({ id: "proj_a", name: "Alpha" }),
      project({ id: "proj_b", name: "Bravo" }),
      project({ id: "proj_c", name: "Charlie" }),
      project({ id: "proj_d", name: "Delta", archived_at: "2026-09-01T00:00:00Z" }),
    ];
    const names = async () => (await titles()).map((t) => t?.replace(/^.|(\d[\d,]* requests?|No requests) in 30 days.*$/g, ""));

    function usage() {
      on("GET", PROJECTS, page(FOUR));
      on("GET", "orgs/:org/usage", {
        data: [
          usageRow("proj_c", { requests: 5, cost: "0.50" }),
          usageRow("proj_b", { requests: 5, cost: "9.25" }),
          usageRow("proj_d", { requests: 40, cost: "1.00" }),
        ],
      });
    }

    it("puts the busiest projects first, ties and projects without requests by name, and keeps the order in the URL", async () => {
      signedIn();
      usage();
      const { user, router } = await renderApp(PATH);
      expect((await within(await list()).findAllByText("5 requests in 30 days")).length).toBe(2);
      expect(await names()).toEqual(["Alpha", "Bravo", "Charlie"]);
      await user.click(screen.getByRole("combobox", { name: "Sort" }));
      await user.click(await screen.findByRole("option", { name: "Most requests" }));
      await waitFor(() => expect(router.state.location.search).toEqual({ sort: "requests" }));
      expect(await names()).toEqual(["Bravo", "Charlie", "Alpha"]);
      await user.click(screen.getByRole("tab", { name: "All" }));
      await waitFor(async () => expect(await names()).toEqual(["Delta", "Bravo", "Charlie", "Alpha"]));
      await user.click(screen.getByRole("combobox", { name: "Sort" }));
      await user.click(await screen.findByRole("option", { name: "Name" }));
      await waitFor(() => expect(router.state.location.search).toEqual({}));
      expect(await names()).toEqual(["Alpha", "Bravo", "Charlie", "Delta"]);
    });

    it("puts the costliest projects first from ?sort=cost", async () => {
      signedIn();
      usage();
      await renderApp(`${PATH}?sort=cost`);
      expect((await within(await list()).findAllByText("5 requests in 30 days")).length).toBe(2);
      expect(await names()).toEqual(["Bravo", "Charlie", "Alpha"]);
      expect(screen.getByRole("combobox", { name: "Sort" })).toHaveTextContent("Highest cost");
    });

    it("offers no cost order without View organization costs, and sorts ?sort=cost by name", async () => {
      withRights(ALL_ORG_RIGHTS.filter((r) => r !== "costs.org.view"));
      usage();
      const { user } = await renderApp(`${PATH}?sort=cost`);
      expect((await within(await list()).findAllByText("5 requests in 30 days")).length).toBe(2);
      expect(await names()).toEqual(["Alpha", "Bravo", "Charlie"]);
      await user.click(screen.getByRole("combobox", { name: "Sort" }));
      expect(await screen.findByRole("option", { name: "Most requests" })).toBeInTheDocument();
      expect(screen.queryByRole("option", { name: "Highest cost" })).not.toBeInTheDocument();
    });

    it("offers no order and sorts by name without View organization usage", async () => {
      withRights(["org.view"]);
      on("GET", PROJECTS, page(FOUR));
      await renderApp(`${PATH}?sort=requests`);
      await within(await list()).findByRole("gridcell", { name: /Charlie/ });
      expect((await titles()).map((t) => t?.slice(1, 6))).toEqual(["Alpha", "Bravo", "Charl"]);
      expect(screen.queryByRole("combobox", { name: "Sort" })).not.toBeInTheDocument();
    });
  });
});
