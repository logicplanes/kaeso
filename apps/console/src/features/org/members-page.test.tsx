import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, roles, signedIn, withRights } from "~/test/fixtures";
import { OTHER_USER, THIRD_USER, hoverReason, member, notifications, paged, roleGrant } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/members`;

const people = [
  member({ user: { id: THIRD_USER, name: "Linus Park", email: null, state: "suspended" }, grant: roleGrant("member", "Member") }),
  member(),
  member({
    user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" },
    grant: { ...roleGrant("admin", "Admin"), rights_added: ["billing.manage"] },
  }),
];

const rowNames = (grid: HTMLElement) =>
  within(grid)
    .getAllByRole("gridcell")
    .map((cell) => cell.textContent);

describe("organization members", () => {
  it("lists the members by name with their role, state and join date", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Members", level: 1 })).toBeInTheDocument();
    const grid = await screen.findByRole("grid", { name: "Members" });
    await waitFor(() => expect(within(grid).getAllByRole("gridcell")).toHaveLength(3));
    const [ada, grace, linus] = within(grid).getAllByRole("gridcell") as [HTMLElement, HTMLElement, HTMLElement];
    expect(ada).toHaveTextContent("Ada Okafor (you)");
    expect(ada).toHaveTextContent("owner@example.com");
    expect(ada).toHaveTextContent("Owner");
    expect(grace).toHaveTextContent("Admin · customized");
    expect(linus).toHaveTextContent("Suspended");
    expect(linus).toHaveTextContent("Member");
    expect(linus).toHaveTextContent("Joined 15 Oct 2026");
  });

  it("marks a member whose account was deleted", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member({ user: { id: THIRD_USER, name: "Linus Park", state: "deleted" }, grant: roleGrant("member", "Member") })]));
    await renderApp(PATH);
    const grid = await screen.findByRole("grid", { name: "Members" });
    expect(await within(grid).findByRole("gridcell", { name: /Linus Park/ })).toHaveTextContent("Deleted");
  });

  it("lists the members from every page", async () => {
    signedIn();
    on("GET", "orgs/:org/members", ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "page2" ? paged(people.slice(0, 1)) : paged(people.slice(1, 2), "page2"),
    );
    await renderApp(PATH);
    const grid = await screen.findByRole("grid", { name: "Members" });
    await waitFor(() => expect(rowNames(grid)).toEqual([expect.stringContaining("Ada Okafor"), expect.stringContaining("Linus Park")]));
    expect(requests("GET", "orgs/:org/members").map((r) => r.query.get("after"))).toEqual([null, "page2"]);
  });

  it("filters by role through the URL", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    const { user, router } = await renderApp(PATH);
    const grid = await screen.findByRole("grid", { name: "Members" });
    await waitFor(() => expect(within(grid).getAllByRole("gridcell")).toHaveLength(3));
    await user.click(screen.getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: "Admin" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ role: "admin" }));
    await waitFor(() => expect(rowNames(screen.getByRole("grid", { name: "Members" }))).toEqual([expect.stringContaining("Grace Hopper")]));
    await user.click(screen.getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: "Every role" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when no member has the chosen role", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    await renderApp(`${PATH}?role=billing`);
    expect(await screen.findByText("No members with the Billing role")).toBeInTheDocument();
    expect(screen.getByText("Choose another role, or every role, to see the others.")).toBeInTheDocument();
  });

  it("keeps a role from the URL that the organization doesn't have", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    await renderApp(`${PATH}?preset=gone`);
    expect(await screen.findByText("No members with the gone role")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Role" })).toHaveTextContent("gone");
  });

  it("groups built-in and custom roles in the filter", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    on("GET", "orgs/:org/roles", {
      data: [...roles, { ...roles[0], id: "role_ops", kind: "custom", name: "Operations", holders: 0, updated_at: null }],
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("combobox", { name: "Role" }));
    expect(await screen.findByRole("option", { name: "Operations" })).toBeInTheDocument();
    expect(screen.getByText("Custom roles")).toBeInTheDocument();
    expect(screen.getByText("Built-in roles")).toBeInTheDocument();
  });

  it("says when the organization has no members to show", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([]));
    await renderApp(PATH);
    expect(await screen.findByText("No members")).toBeInTheDocument();
  });

  it("says when the members couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/members", apiError(403, "forbidden", "The members store is down."));
    await renderApp(PATH);
    expect(await screen.findByText("The members couldn't load")).toBeInTheDocument();
    expect((await notifications().findAllByText("The members store is down.")).length).toBeGreaterThan(0);
  });

  it("hides the page from people who can't view members", async () => {
    withRights(["org.view"], { role: "member" });
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the members here")).toBeInTheDocument();
    expect(screen.getByText("You need View members; ask an owner or admin for it.")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/members")).toHaveLength(0);
  });

  it("opens a member's sheet from the list and closes it again", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    on("GET", "orgs/:org/members/:user", ({ params }: { params: Record<string, string> }) => people.find((p) => p.user.id === params.user));
    on("GET", "orgs/:org/members/:user/rights", {
      user_id: OTHER_USER,
      org: { role: roleGrant("admin", "Admin").role, rights: [], removed: [] },
      every_project: [],
      projects: [],
    });
    const { user, router } = await renderApp(PATH);
    const grid = await screen.findByRole("grid", { name: "Members" });
    await user.click(await within(grid).findByText("Grace Hopper"));
    await waitFor(() => expect(router.state.location.search).toEqual({ member: OTHER_USER }));
    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("switches between the tabs through the URL", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    on("GET", "orgs/:org/invites", paged([]));
    const { user, router } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "Invites" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "invites" }));
    expect(await screen.findByText("No pending invitations")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Roles" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles" }));
    expect(await screen.findByRole("button", { name: "Create role" })).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Members" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(await screen.findByRole("button", { name: "Invite member" })).toBeInTheDocument();
  });

  it("sends the old invites address to the invites tab", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    on("GET", "orgs/:org/invites", paged([]));
    const { router } = await renderApp(`/settings/orgs/${ORG}/invites`);
    expect(await screen.findByText("No pending invitations")).toBeInTheDocument();
    expect(router.state.location.pathname).toBe(PATH);
    expect(router.state.location.search).toEqual({ tab: "invites" });
  });

  it("opens the invite dialog from the button and from the URL", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Invite member" }));
    const dialog = await screen.findByRole("dialog", { name: "Invite members" });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Invite members" })).not.toBeInTheDocument());
  });

  it("opens the invite dialog straight away from ?invite=true", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(people));
    const { router, user } = await renderApp(`${PATH}?invite=true`);
    expect(await screen.findByRole("dialog", { name: "Invite members" })).toBeInTheDocument();
    expect(router.state.location.search).toEqual({ invite: true });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("opens the role dialog straight away from ?create=true", async () => {
    signedIn();
    const { router, user } = await renderApp(`${PATH}?tab=roles&create=true`);
    expect(await screen.findByRole("dialog", { name: "Create role" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles" }));
  });

  it("explains why people can't invite or create roles without the rights", async () => {
    withRights(["org.view", "members.view"], { role: "member" });
    on("GET", "orgs/:org/members", paged(people));
    const { user } = await renderApp(PATH);
    const invite = await screen.findByRole("button", { name: "Invite member" });
    expect(invite).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, invite, "You need Invite members; ask an owner or admin for it.");
    await user.click(screen.getByRole("tab", { name: "Roles" }));
    const create = await screen.findByRole("button", { name: "Create role" });
    await hoverReason(user, create, "You need Manage roles; ask an owner or admin for it.");
  });
});
