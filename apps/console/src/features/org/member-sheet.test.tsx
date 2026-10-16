import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ALL_ORG_RIGHTS, ORG, PROJECT, USER, membership, signedIn, whoAmI } from "~/test/fixtures";
import { OTHER_USER, THIRD_USER, hoverReason, member, memberRights, membersData, notifications, roleGrant } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/members`;
const sheetOf = (id: string) => `${PATH}?member=${id}`;

const ada = member();
const grace = member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" }, grant: roleGrant("developer", "Developer") });
const linus = member({ user: { id: THIRD_USER, name: "Linus Park", email: null, state: "suspended" }, grant: roleGrant("member", "Member") });

function asAdmin(rights: string[] = ALL_ORG_RIGHTS.filter((r) => r !== "org.delete" && r !== "org.ownership.transfer" && r !== "billing.manage")) {
  return signedIn({ session: whoAmI({ memberships: [membership({ rights, role: { id: "admin", kind: "builtin", name: "Admin" }, preset: "admin" })] }) });
}

async function openSheet(id: string, name: string) {
  const view = await renderApp(sheetOf(id));
  const sheet = await screen.findByRole("dialog", { name });
  await within(sheet).findByText("User id");
  return { ...view, sheet };
}

describe("a member's sheet", () => {
  it("shows the member's details, access and projects", async () => {
    signedIn();
    membersData([ada, grace, linus], (user) =>
      memberRights({
        user_id: user,
        every_project: [{ right: "project.view", sources: [{ kind: "org_role", role: { id: "developer", kind: "builtin", name: "Developer" } }] }],
        projects: [
          {
            project: { id: PROJECT, name: "Checkout assistant", avatar_url: null, is_default: false },
            role: { id: "admin", kind: "builtin", name: "Admin" },
            rights: [],
            removed: [],
          },
          { project: { id: "proj_support", name: "Support triage", avatar_url: null, is_default: false }, role: null, rights: [], removed: [] },
        ],
      }),
    );
    const { sheet } = await openSheet(OTHER_USER, "Grace Hopper");
    expect(within(sheet).getByText("grace@example.com")).toBeInTheDocument();
    expect(within(sheet).getByText(OTHER_USER)).toBeInTheDocument();
    expect(within(sheet).getByText("15 Oct 2026")).toBeInTheDocument();
    expect(within(sheet).queryByText("Account")).not.toBeInTheDocument();
    expect(within(sheet).getByText("In every project")).toBeInTheDocument();
    const projects = within(sheet).getByRole("grid", { name: "Grace Hopper's projects" });
    expect(within(projects).getByText("Checkout assistant")).toBeInTheDocument();
    expect(within(projects).getByText("Admin")).toBeInTheDocument();
    expect(within(projects).getByText("Through their organization role")).toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: "Transfer ownership" })).toBeInTheDocument();
    expect(requests("GET", `orgs/:org/members/${OTHER_USER}/rights`)).toHaveLength(1);
  });

  it("opens the member in a project from their projects", async () => {
    signedIn();
    membersData([ada, grace], (user) =>
      memberRights({
        user_id: user,
        projects: [
          { project: { id: PROJECT, name: "Checkout assistant", avatar_url: "/avatars/p.webp", is_default: false }, role: null, rights: [], removed: [] },
        ],
      }),
    );
    const { sheet, user, router } = await openSheet(OTHER_USER, "Grace Hopper");
    await user.click(within(within(sheet).getByRole("grid", { name: "Grace Hopper's projects" })).getByText("Checkout assistant"));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}/projects/${PROJECT}/members`));
    expect(router.state.location.search).toEqual({ member: OTHER_USER });
  });

  it("names a suspended member's account state and missing email", async () => {
    signedIn();
    membersData([ada, grace, linus]);
    const { sheet } = await openSheet(THIRD_USER, "Linus Park");
    expect(within(sheet).getByText("Account")).toBeInTheDocument();
    expect(within(sheet).getByText("Suspended")).toBeInTheDocument();
    expect(within(sheet).getByText("In no projects")).toBeInTheDocument();
    expect(within(sheet).getByText("Add them from a project's Members page.")).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Transfer ownership" })).not.toBeInTheDocument();
  });

  it("changes the member's role", async () => {
    signedIn();
    membersData([ada, grace]);
    on("PATCH", "orgs/:org/members/:user", ({ body }: { body: { role: string } }) => ({ ...grace, grant: roleGrant(body.role, "Auditor") }));
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    const save = within(sheet).getByRole("button", { name: "Save role" });
    expect(save).toBeDisabled();
    await user.click(within(sheet).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Auditor/ }));
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    expect(await notifications().findByText("Grace Hopper is Auditor now")).toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/members/:user").map((r) => r.body)).toEqual([{ role: "auditor", rights_added: [], rights_removed: [] }]);
  });

  it("discards a role change when cancelled", async () => {
    signedIn();
    membersData([ada, grace]);
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    await user.click(within(sheet).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Auditor/ }));
    await user.click(within(sheet).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(within(sheet).getByRole("button", { name: "Save role" })).toBeDisabled());
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toHaveTextContent("Developer");
    expect(requests("PATCH", "orgs/:org/members/:user")).toHaveLength(0);
  });

  it("makes the member an owner in your place", async () => {
    signedIn();
    membersData([ada, grace]);
    on("POST", "orgs/:org/ownership-transfer", { ...grace, grant: roleGrant("owner", "Owner") });
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    await user.click(within(sheet).getByRole("button", { name: "Transfer ownership" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Make Grace Hopper an owner?" });
    expect(within(confirm).getByText("They become an owner with every right, and you become an admin.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Transfer ownership" }));
    expect(await notifications().findByText("Grace Hopper is an owner now; you're an admin")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/ownership-transfer").map((r) => r.body)).toEqual([{ user_id: OTHER_USER }]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("closes the transfer and says why when it's refused", async () => {
    signedIn();
    membersData([ada, grace]);
    on("POST", "orgs/:org/ownership-transfer", apiError(409, "conflict", "Grace must set up two-factor first."));
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    await user.click(within(sheet).getByRole("button", { name: "Transfer ownership" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Make Grace Hopper an owner?" });
    await user.click(within(confirm).getByRole("button", { name: "Transfer ownership" }));
    expect((await notifications().findAllByText("Grace must set up two-factor first.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("removes the member from the organization", async () => {
    signedIn();
    membersData([ada, grace]);
    on("DELETE", "orgs/:org/members/:user", undefined);
    const { sheet, user, router } = await openSheet(OTHER_USER, "Grace Hopper");
    await user.click(within(sheet).getByRole("button", { name: "Remove from organization" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove Grace Hopper from the organization?" });
    expect(within(confirm).getByText("They leave every project, and their user keys in the organization are archived.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Remove member" }));
    expect(await notifications().findByText("Grace Hopper removed from the organization")).toBeInTheDocument();
    expect(requests("DELETE", `orgs/:org/members/${OTHER_USER}`)).toHaveLength(1);
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("keeps the member and says why when the removal is refused", async () => {
    signedIn();
    membersData([ada, grace]);
    on("DELETE", "orgs/:org/members/:user", apiError(409, "conflict", "Grace owns the billing account."));
    const { sheet, user, router } = await openSheet(OTHER_USER, "Grace Hopper");
    await user.click(within(sheet).getByRole("button", { name: "Remove from organization" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove Grace Hopper from the organization?" });
    await user.click(within(confirm).getByRole("button", { name: "Remove member" }));
    expect((await notifications().findAllByText("Grace owns the billing account.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(router.state.location.search).toEqual({ member: OTHER_USER });
  });

  it("lets you leave the organization yourself", async () => {
    signedIn();
    membersData([ada, member({ user: { id: OTHER_USER, name: "Grace Hopper" } })]);
    on("DELETE", "orgs/:org/members/:user", undefined);
    const { sheet, user } = await openSheet(USER, "Ada Okafor");
    expect(within(sheet).getByRole("heading", { name: "Leave the organization" })).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Leave organization" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Leave the organization?" });
    await user.click(within(confirm).getByRole("button", { name: "Leave organization" }));
    expect(await notifications().findByText("You left the organization")).toBeInTheDocument();
    expect(requests("DELETE", `orgs/:org/members/${USER}`)).toHaveLength(1);
  });

  it("keeps you as the last owner from leaving", async () => {
    signedIn();
    membersData([ada, linus]);
    const { sheet, user } = await openSheet(USER, "Ada Okafor");
    const leave = within(sheet).getByRole("button", { name: "Leave organization" });
    expect(leave).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, leave, "An organization keeps at least one owner; make someone else an owner first");
    await user.click(leave);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("lets you leave without the right to remove members", async () => {
    asAdmin(ALL_ORG_RIGHTS.filter((r) => r !== "members.remove" && r !== "org.delete" && r !== "org.ownership.transfer" && r !== "billing.manage"));
    membersData([member({ user: { id: OTHER_USER, name: "Grace Hopper" } }), member({ grant: roleGrant("admin", "Admin") })]);
    const { sheet, user } = await openSheet(USER, "Ada Okafor");
    await user.click(within(sheet).getByRole("button", { name: "Leave organization" }));
    expect(await screen.findByRole("alertdialog", { name: "Leave the organization?" })).toBeInTheDocument();
  });

  it("keeps the last owner", async () => {
    signedIn();
    membersData([member({ user: { id: OTHER_USER, name: "Grace Hopper" } }), linus]);
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    const remove = within(sheet).getByRole("button", { name: "Remove from organization" });
    expect(remove).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, remove, "An organization keeps at least one owner; make someone else an owner first");
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toBeDisabled();
  });

  it("lets only owners change an owner", async () => {
    asAdmin();
    membersData([ada, member({ user: { id: OTHER_USER, name: "Grace Hopper" } })]);
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    await hoverReason(user, within(sheet).getByRole("button", { name: "Remove from organization" }), "Only owners can change or remove an owner");
  });

  it("lets nobody change a member who holds rights they don't", async () => {
    asAdmin();
    const billing = member({ user: { id: OTHER_USER, name: "Grace Hopper" }, grant: roleGrant("billing", "Billing", ["org.view", "billing.manage"]) });
    membersData([ada, billing]);
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    await hoverReason(
      user,
      within(sheet).getByRole("button", { name: "Remove from organization" }),
      "Grace Hopper holds rights you don't hold, so you can't change them",
    );
    expect(within(sheet).queryByRole("button", { name: "Transfer ownership" })).not.toBeInTheDocument();
  });

  it("lets an admin change a member whose grant lists no project rights", async () => {
    asAdmin();
    membersData([ada, member({ user: { id: OTHER_USER, name: "Grace Hopper" }, grant: { ...roleGrant("member", "Member"), project_rights: null } })]);
    const { sheet } = await openSheet(OTHER_USER, "Grace Hopper");
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toBeEnabled();
    expect(within(sheet).getByRole("button", { name: "Remove from organization" })).not.toHaveAttribute("aria-disabled", "true");
  });

  it("shows a member opened from a link before the member list has loaded", async () => {
    signedIn();
    membersData([ada, grace]);
    on("GET", "orgs/:org/members", () => new Promise(() => undefined));
    const { sheet } = await openSheet(OTHER_USER, "Grace Hopper");
    expect(within(sheet).getByText("grace@example.com")).toBeInTheDocument();
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toBeEnabled();
  });

  it("still shows the member when their access couldn't load", async () => {
    signedIn();
    membersData([ada, grace]);
    on("GET", "orgs/:org/members/:user/rights", apiError(403, "forbidden", "The gateway failed."));
    const { sheet } = await openSheet(OTHER_USER, "Grace Hopper");
    expect(within(sheet).getByText("grace@example.com")).toBeInTheDocument();
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toBeEnabled();
    expect(within(sheet).getByRole("button", { name: "Remove from organization" })).toBeInTheDocument();
  });

  it("names the right a member needs to remove someone", async () => {
    asAdmin(["org.view", "members.view"]);
    membersData([ada, grace]);
    const { sheet, user } = await openSheet(OTHER_USER, "Grace Hopper");
    await hoverReason(user, within(sheet).getByRole("button", { name: "Remove from organization" }), "You need Remove members; ask an owner or admin for it.");
  });

  it("says when the member doesn't exist", async () => {
    signedIn();
    membersData([ada]);
    const { user, router } = await renderApp(sheetOf("usr_gone"));
    const sheet = await screen.findByRole("dialog", { name: "Member" });
    expect(await within(sheet).findByText("This member doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Back to members" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when the member can't be shown", async () => {
    signedIn();
    membersData([ada]);
    on("GET", "orgs/:org/members/:user", apiError(403, "forbidden", "The gateway failed."));
    await renderApp(sheetOf(OTHER_USER));
    const sheet = await screen.findByRole("dialog", { name: "Member" });
    expect(await within(sheet).findByText("This member can't be shown right now.")).toBeInTheDocument();
  });
});
