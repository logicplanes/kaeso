import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import type { InstanceOrgView } from "~/api/generated/types";
import { signedIn, whoAmI } from "~/test/fixtures";
import { instanceOrg, instanceUser } from "~/test/fixtures/instance-account";
import { hoverReason, notifications, paged } from "~/test/fixtures/org";
import { apiError, json, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/orgs";
const NORTHWIND = instanceOrg();
const CONTOSO = instanceOrg({
  id: "org_contoso",
  name: "Contoso",
  state: "scheduled_for_deletion",
  deletion_due_at: "2026-10-22T12:00:00Z",
  members: 12,
  projects: 3,
});
const FABRIKAM = instanceOrg({ id: "org_fabrikam", name: "Fabrikam", state: "scheduled_for_deletion", deletion_due_at: null });
const ORPHAN = instanceOrg({ id: "org_orphan", name: "Orphanage", owners: 0 });

function orgs(list: InstanceOrgView[] = [NORTHWIND, CONTOSO, FABRIKAM, ORPHAN]) {
  on("GET", "instance/orgs", ({ query }: { query: URLSearchParams }) =>
    paged(list.filter((o) => !query.get("search") || o.name.includes(query.get("search") ?? ""))),
  );
}

const orgQueries = () => requests("GET", "instance/orgs").map((r) => Object.fromEntries(r.query));
const table = () => screen.getByRole("table", { name: "Organizations" });

async function rowMenu(user: UserEvent, name: string) {
  await user.pointer({ keys: "[MouseRight]", target: await within(table()).findByText(name) });
  return screen.findByRole("menu");
}

describe("instance organizations list", () => {
  it("lists every organization with its state, members, projects and creation day", async () => {
    signedIn();
    orgs();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Organizations", level: 1 })).toBeInTheDocument();
    expect(await within(table()).findByText("Northwind Labs")).toBeInTheDocument();
    expect(within(table()).getByText(/^Deletion on 22 Oct/)).toBeInTheDocument();
    expect(within(table()).getByText("Scheduled for deletion")).toBeInTheDocument();
    expect(within(table()).getByText("No owner")).toBeInTheDocument();
    expect(within(table()).getByText("Active")).toBeInTheDocument();
    expect(within(table()).getByText("12")).toBeInTheDocument();
    expect(within(table()).getAllByText("15 Oct 2026")).toHaveLength(4);
    expect(orgQueries()[0]).toEqual({ limit: "100" });
  });

  it("searches by name through the URL", async () => {
    signedIn();
    orgs();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Northwind Labs");
    await user.type(screen.getByRole("searchbox", { name: "Search organizations" }), " Nobody {Enter}");
    expect(await screen.findByText("No organizations match")).toBeInTheDocument();
    expect(screen.getByText("Search by another name.")).toBeInTheDocument();
    expect(new URLSearchParams(window.location.search).get("q")).toBe("Nobody");
    expect(orgQueries().at(-1)).toEqual({ limit: "100", search: "Nobody" });
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(await within(table()).findByText("Northwind Labs")).toBeInTheDocument();
    expect(window.location.search).toBe("");
  });

  it("searches when the field loses focus", async () => {
    signedIn();
    orgs();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Northwind Labs");
    await user.type(screen.getByRole("searchbox", { name: "Search organizations" }), "Contoso");
    await user.tab();
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("q")).toBe("Contoso"));
    await waitFor(() => expect(within(table()).queryByText("Northwind Labs")).not.toBeInTheDocument());
  });

  it("says when there are no organizations", async () => {
    signedIn();
    orgs([]);
    await renderApp(PATH);
    expect(await screen.findByText("No organizations")).toBeInTheDocument();
  });

  it("sorts by deletion date through the URL", async () => {
    signedIn();
    orgs();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Northwind Labs");
    await user.click(screen.getByRole("button", { name: "State" }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("sort")).toBe("deletion"));
    await waitFor(() => expect(orgQueries().at(-1)).toEqual({ limit: "100", sort: "deletion" }));
    expect(screen.getByRole("button", { name: "State" }).closest('[role="columnheader"]')).toHaveAttribute("aria-sort", "ascending");
    await user.click(screen.getByRole("button", { name: "State" }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("sort")).toBeNull());
    await user.click(screen.getByRole("button", { name: "Name" }));
    expect(screen.getByRole("button", { name: "Name" }).closest('[role="columnheader"]')).toHaveAttribute("aria-sort", "ascending");
    expect(window.location.search).toBe("");
  });

  it("keeps the deletion order from the URL", async () => {
    signedIn();
    orgs();
    await renderApp(`${PATH}?sort=deletion&q=Contoso`);
    expect(await within(table()).findByText("Contoso")).toBeInTheDocument();
    expect(orgQueries()[0]).toEqual({ limit: "100", search: "Contoso", sort: "deletion" });
  });

  it("loads the next page on request", async () => {
    signedIn();
    on("GET", "instance/orgs", ({ query }: { query: URLSearchParams }) => (query.get("after") === "cur_2" ? paged([CONTOSO]) : paged([NORTHWIND], "cur_2")));
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Northwind Labs");
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await within(table()).findByText("Contoso")).toBeInTheDocument();
  });

  it("says when the organizations couldn't load", async () => {
    signedIn();
    on("GET", "instance/orgs", apiError(403, "permission_denied", "Only instance admins can list organizations."));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Couldn’t load these rows")).toBeInTheDocument();
    expect((await notifications().findAllByText("Only instance admins can list organizations.")).length).toBeGreaterThan(0);
    orgs();
    await user.click(within(table()).getByRole("button", { name: "Try again" }));
    expect(await within(table()).findByText("Northwind Labs")).toBeInTheDocument();
  });

  it("opens an organization from its row", async () => {
    signedIn();
    orgs();
    on("GET", "instance/orgs/:org", NORTHWIND);
    const { user } = await renderApp(PATH);
    await user.click(await within(table()).findByText("Northwind Labs"));
    await waitFor(() => expect(window.location.pathname).toBe("/settings/instance/orgs/org_northwind"));
  });
});

describe("organizations scheduled for deletion", () => {
  it("offers nothing in the menu of an active organization", async () => {
    signedIn();
    orgs();
    const { user } = await renderApp(PATH);
    await user.pointer({ keys: "[MouseRight]", target: await within(table()).findByText("Northwind Labs") });
    expect(screen.queryByRole("menuitem", { name: "Restore organization" })).not.toBeInTheDocument();
  });

  it("restores one from its menu", async () => {
    signedIn();
    orgs();
    on("POST", "instance/orgs/:org/restore", { ...CONTOSO, state: "active", deletion_due_at: null });
    const { user } = await renderApp(PATH);
    const menu = await rowMenu(user, "Contoso");
    await user.click(within(menu).getByRole("menuitem", { name: "Restore organization" }));
    expect(await notifications().findByText("Contoso restored")).toBeInTheDocument();
    expect(requests("POST", "instance/orgs/:org/restore").map((r) => r.params)).toEqual([{ org: "org_contoso" }]);
  });

  it("deletes one now once its name is typed, staying on the list", async () => {
    signedIn();
    orgs();
    on("DELETE", "instance/orgs/:org", () => new Response(null, { status: 202 }));
    const { user } = await renderApp(PATH);
    const menu = await rowMenu(user, "Contoso");
    await user.click(within(menu).getByRole("menuitem", { name: "Delete now" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Contoso now" });
    expect(within(dialog).getByText("Projects, keys, provider keys, settings, usage and logs, for good.")).toBeInTheDocument();
    expect(within(dialog).getByText("22 Oct 2026")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const typed = within(dialog).getByRole("textbox", { name: "Organization name" });
    await user.type(typed, "Cont");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Delete now" }), "Type Contoso to confirm");
    await user.type(typed, "oso{Enter}");
    expect(await notifications().findByText("Contoso is deleted within minutes")).toBeInTheDocument();
    expect(requests("DELETE", "instance/orgs/:org").map((r) => r.params)).toEqual([{ org: "org_contoso" }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Contoso now" })).not.toBeInTheDocument());
    expect(window.location.pathname).toBe(PATH);
  });

  it("closes the dialog and says why when deleting now is refused", async () => {
    signedIn();
    orgs();
    on("DELETE", "instance/orgs/:org", apiError(409, "conflict", "A deletion is already running."));
    const { user } = await renderApp(PATH);
    const menu = await rowMenu(user, "Fabrikam");
    await user.click(within(menu).getByRole("menuitem", { name: "Delete now" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Fabrikam now" });
    expect(within(dialog).queryByText("Scheduled for")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Organization name" }), "Fabrikam{Enter}");
    expect((await notifications().findAllByText("A deletion is already running.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete Fabrikam now" })).not.toBeInTheDocument());
  });

  it("offers only restoring when the install doesn't allow deleting organizations", async () => {
    signedIn({ session: whoAmI({ org_deletion: { allowed: false, grace_days: 7 } }) });
    orgs();
    const { user } = await renderApp(PATH);
    const menu = await rowMenu(user, "Contoso");
    expect(within(menu).getByRole("menuitem", { name: "Restore organization" })).toBeInTheDocument();
    expect(within(menu).queryByRole("menuitem", { name: "Delete now" })).not.toBeInTheDocument();
  });
});

describe("creating an organization", () => {
  const people = [instanceUser(), instanceUser({ id: "usr_linus", name: "Linus Park", email: null })];

  it("creates one with an owner and opens it", async () => {
    signedIn();
    orgs();
    on("GET", "instance/users", paged(people));
    on("POST", "instance/orgs", ({ body }: { body: { name: string } }) => instanceOrg({ id: "org_new", name: body.name }));
    on("GET", "instance/orgs/:org", instanceOrg({ id: "org_new", name: "Initech" }));
    const { user } = await renderApp(PATH);
    await user.click(screen.getAllByRole("button", { name: "Create organization" })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Create organization" }), "Give the organization a name");
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "  Initech ");
    await user.click(within(dialog).getByRole("combobox", { name: "Owner (optional)" }));
    expect(await screen.findByRole("option", { name: /Linus Park/ })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: /Grace Hopper/ }));
    await user.click(within(dialog).getByRole("button", { name: "Create organization" }));
    expect(await notifications().findByText("Initech created")).toBeInTheDocument();
    expect(requests("POST", "instance/orgs").map((r) => r.body)).toEqual([{ name: "Initech", owner: "usr_grace" }]);
    expect(requests("GET", "instance/users").map((r) => r.query.get("state"))).toContain("active");
    await waitFor(() => expect(window.location.pathname).toBe("/settings/instance/orgs/org_new"));
  });

  it("opens from the URL and creates one without an owner from the keyboard", async () => {
    signedIn();
    orgs();
    on("GET", "instance/users", paged(people));
    on("POST", "instance/orgs", instanceOrg({ id: "org_new", name: "Initech" }));
    on("GET", "instance/orgs/:org", instanceOrg({ id: "org_new", name: "Initech" }));
    const { user } = await renderApp(`${PATH}?create=true`);
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "instance/orgs")).toHaveLength(0);
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Initech");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    await waitFor(() => expect(requests("POST", "instance/orgs").map((r) => r.body)).toEqual([{ name: "Initech" }]));
  });

  it("keeps the dialog open and shows why when creating is refused", async () => {
    signedIn();
    orgs();
    on("GET", "instance/users", paged([]));
    on("POST", "instance/orgs", json({ error: { type: "conflict", message: "That name is taken.", param: "name", right: null } }, { status: 409 }));
    const { user } = await renderApp(PATH);
    await user.click(screen.getAllByRole("button", { name: "Create organization" })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Northwind Labs");
    await user.click(within(dialog).getByRole("button", { name: "Create organization" }));
    expect((await screen.findAllByText("That name is taken.")).length).toBeGreaterThan(0);
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create organization" })).not.toBeInTheDocument());
  });

  it("opens from the c shortcut and shows the pictures of organizations and users", async () => {
    signedIn();
    orgs([instanceOrg({ avatar_url: "/avatars/northwind.webp" })]);
    on("GET", "instance/users", paged([instanceUser({ avatar_url: "/avatars/grace.webp" })]));
    const { user } = await renderApp(PATH);
    const name = await within(table()).findByText("Northwind Labs");
    expect(name.parentElement?.querySelector('img[src="/avatars/northwind.webp"]')).not.toBeNull();
    await user.keyboard("c");
    const dialog = await screen.findByRole("dialog", { name: "Create organization" });
    await user.click(within(dialog).getByRole("combobox", { name: "Owner (optional)" }));
    const option = await screen.findByRole("option", { name: /Grace Hopper/ });
    expect(option.querySelector('img[src="/avatars/grace.webp"]')).not.toBeNull();
  });

  it("closes with Escape and opens again from the narrow button", async () => {
    signedIn();
    orgs();
    on("GET", "instance/users", paged([]));
    const { user } = await renderApp(`${PATH}?create=true`);
    await screen.findByRole("dialog", { name: "Create organization" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Create organization" })).not.toBeInTheDocument());
    await user.click(screen.getAllByRole("button", { name: "Create organization" })[1] as HTMLElement);
    expect(await screen.findByRole("dialog", { name: "Create organization" })).toBeInTheDocument();
    expect(requests("POST", "instance/orgs")).toHaveLength(0);
  });
});
