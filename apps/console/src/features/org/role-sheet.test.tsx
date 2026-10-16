import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { RoleView } from "~/api/generated/types";
import { ALL_ORG_RIGHTS, ORG, asAdmin, catalogue, membership, roles, signedIn, whoAmI } from "~/test/fixtures";
import { OTHER_USER, THIRD_USER, hoverReason, member, notifications, paged, roleGrant, fill } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const sheetOf = (key: string) => `/settings/orgs/${ORG}/members?tab=roles&role=${key}`;
const LEAD = "role_01m507nbc5es18k4kb8aky91f6";

const holders = [
  member(),
  member({ user: { id: OTHER_USER, name: "Grace Hopper", email: "grace@example.com" }, grant: roleGrant("admin", "Admin") }),
  member({ user: { id: THIRD_USER, name: "Linus Park", email: null, avatar_url: "/avatars/linus.webp" }, grant: roleGrant("admin", "Admin") }),
];

const opsRole: RoleView = {
  ...(roles[0] as RoleView),
  id: "role_ops",
  kind: "custom",
  name: "Operations",
  description: "",
  holders: 0,
  rights: ["org.view", "members.view"],
  project_rights: ["project.view"],
  updated_at: null,
};

function withRoles(list: RoleView[]) {
  on("GET", "orgs/:org/roles", { data: list });
}

const matrix = (scope: HTMLElement, label: string) =>
  within(scope)
    .getAllByLabelText(label)
    .find((el) => within(el).queryAllByRole("checkbox").length > 0) as HTMLElement;

async function openSheet(key: string, name: string) {
  const view = await renderApp(sheetOf(key));
  const sheet = await screen.findByRole("dialog", { name });
  await within(sheet).findByText("Applies to");
  return { ...view, sheet };
}

describe("a built-in role's sheet", () => {
  it("describes the role and lists who has it", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet, user, router } = await openSheet("org:admin", "Admin");
    expect(within(sheet).getByText("Built-in")).toBeInTheDocument();
    expect(within(sheet).getByText("The organization")).toBeInTheDocument();
    expect(within(sheet).getByText("30 rights, 31 in projects")).toBeInTheDocument();
    expect(within(sheet).getByRole("link", { name: "1 member or account" })).toHaveAttribute("href", `/settings/orgs/${ORG}/members?role=admin`);
    expect(within(sheet).getByText(/Manages the organization and every project/)).toBeInTheDocument();
    expect(within(sheet).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    expect(within(sheet).getByText("What it gives in every project of the organization.")).toBeInTheDocument();
    const who = await within(sheet).findByRole("grid", { name: "Members with Admin" });
    await waitFor(() => expect(within(who).getAllByRole("gridcell")).toHaveLength(2));
    expect(within(sheet).queryByRole("button", { name: "Save role" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Delete role" })).not.toBeInTheDocument();
    await user.click(within(who).getByText("Grace Hopper"));
    await waitFor(() => expect(router.state.location.search).toEqual({ member: OTHER_USER }));
  });

  it("opens the sheet while the roles load", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    on("GET", "orgs/:org/roles", () => new Promise(() => {}));
    await renderApp(sheetOf("org:admin"));
    const sheet = await screen.findByRole("dialog", { name: "Role" });
    expect(within(sheet).queryByText("Applies to")).not.toBeInTheDocument();
  });

  it("describes the role while the people who hold it load", async () => {
    signedIn();
    on("GET", "orgs/:org/members", () => new Promise(() => {}));
    const { sheet } = await openSheet("org:admin", "Admin");
    expect(within(sheet).getByText("Applies to")).toBeInTheDocument();
  });

  it("says every right for the owner role", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet } = await openSheet("org:owner", "Owner");
    expect(within(sheet).getByText("Every right, now and in later releases")).toBeInTheDocument();
    expect(within(sheet).getByText("Owners hold every right in every project.")).toBeInTheDocument();
  });

  it("says when nobody has an organization role", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet } = await openSheet("org:billing", "Billing");
    expect(within(sheet).getAllByText("Nobody yet").length).toBeGreaterThan(0);
    expect(await within(sheet).findByText("No members have it")).toBeInTheDocument();
    expect(within(sheet).getByText("Give it from a member's sheet, or when inviting someone.")).toBeInTheDocument();
  });

  it("counts a project role's holders without listing them", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet } = await openSheet("project:viewer", "Viewer");
    expect(within(sheet).getByText("A project")).toBeInTheDocument();
    expect(within(sheet).getByText("4 members or accounts")).toBeInTheDocument();
    expect(within(sheet).queryByRole("link", { name: "4 members or accounts" })).not.toBeInTheDocument();
    expect(within(sheet).getByText("In a project")).toBeInTheDocument();
    expect(within(sheet).queryByText("Who has it")).not.toBeInTheDocument();
  });

  it("starts a custom role from a duplicate", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet, user, router } = await openSheet("org:admin", "Admin");
    await user.click(within(sheet).getByRole("button", { name: "Duplicate" }));
    const dialog = await screen.findByRole("dialog", { name: "Create role" });
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("Admin copy");
    expect(within(dialog).getByRole("radio", { name: /Organization role/ })).toBeChecked();
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles" }));
  });

  it("starts a project role from a duplicate of a project role", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet, user } = await openSheet("project:viewer", "Viewer");
    await user.click(within(sheet).getByRole("button", { name: "Duplicate" }));
    const dialog = await screen.findByRole("dialog", { name: "Create role" });
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("Viewer copy");
    expect(within(dialog).getByRole("radio", { name: /Project role/ })).toBeChecked();
  });

  it("shows the role's rights once the catalogue arrives", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const waiting: (() => void)[] = [];
    on("GET", "orgs/:org/rights", () => new Promise((resolve) => waiting.push(() => resolve(catalogue))));
    const { sheet } = await openSheet("org:admin", "Admin");
    expect(within(sheet).queryByLabelText("Rights of Admin")).not.toBeInTheDocument();
    for (const release of waiting) release();
    expect(await within(sheet).findAllByLabelText("Rights of Admin")).not.toHaveLength(0);
  });

  it("says when the role doesn't exist", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { user, router } = await renderApp(sheetOf("org:gone"));
    const sheet = await screen.findByRole("dialog", { name: "Role" });
    expect(await within(sheet).findByText("This role doesn't exist or was deleted.")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Back to roles" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles" }));
  });
});

describe("a custom role's sheet", () => {
  it("renames a role that people hold once the change is confirmed", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    on("PATCH", "orgs/:org/roles/:role", ({ body }: { body: { name: string } }) => ({ ...roles.find((r) => r.id === LEAD), name: body.name }));
    const { sheet, user } = await openSheet(`project:${LEAD}`, "Support lead");
    expect(within(sheet).getByText("Custom")).toBeInTheDocument();
    expect(within(sheet).getByText("Changed")).toBeInTheDocument();
    const name = within(sheet).getByRole("textbox", { name: "Name" });
    expect(within(sheet).getByRole("button", { name: "Save role" })).toBeDisabled();
    await user.clear(name);
    await fill(user, name, "  Team lead ");
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Change Support lead for 1 member or service account?" });
    expect(within(confirm).getByText("Changes apply at once to everyone with this role.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Save role" }));
    expect(await notifications().findByText("Team lead saved for 1 member or service account")).toBeInTheDocument();
    const lead = roles.find((r) => r.id === LEAD) as RoleView;
    expect(requests("PATCH", `orgs/:org/roles/${LEAD}`).map((r) => r.body)).toEqual([
      { name: "Team lead", description: lead.description, rights: lead.rights },
    ]);
  });

  it("asks for a name before saving a role without one", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet, user } = await openSheet(`project:${LEAD}`, "Support lead");
    await user.clear(within(sheet).getByRole("textbox", { name: "Name" }));
    const save = within(sheet).getByRole("button", { name: "Save role" });
    expect(save).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, save, "Name the role");
    await user.click(save);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/roles/:role")).toHaveLength(0);
  });

  it("closes the confirmation and keeps the draft when the save is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    on("PATCH", "orgs/:org/roles/:role", apiError(422, "invalid_request", "That name is taken.", { param: "name" }));
    const { sheet, user } = await openSheet(`project:${LEAD}`, "Support lead");
    const name = within(sheet).getByRole("textbox", { name: "Name" });
    await user.type(name, " 2");
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Change Support lead for 1 member or service account?" });
    await user.click(within(confirm).getByRole("button", { name: "Save role" }));
    expect((await notifications().findAllByText("That name is taken.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(name).toHaveValue("Support lead 2");
  });

  it("won't delete a role people hold", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet, user } = await openSheet(`project:${LEAD}`, "Support lead");
    const remove = within(sheet).getByRole("button", { name: "Delete role" });
    expect(remove).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, remove, "Held by 1 member or service account; give them another role first");
  });

  it("saves an unused role's rights straight away and discards a draft", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, opsRole]);
    on("PATCH", "orgs/:org/roles/:role", { ...opsRole, description: "Runs the install" });
    const { sheet, user } = await openSheet("org:role_ops", "Operations");
    expect(within(sheet).getByText("2 rights, 1 in projects")).toBeInTheDocument();
    expect(within(sheet).getAllByText("Nobody yet").length).toBeGreaterThan(0);
    const description = within(sheet).getByRole("textbox", { name: /Description/ });
    await fill(user, description, "Throwaway");
    await user.click(within(sheet).getByRole("button", { name: "Cancel" }));
    expect(description).toHaveValue("");
    await fill(user, description, "Runs the install");
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    expect(await notifications().findByText("Operations saved")).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(requests("PATCH", "orgs/:org/roles/role_ops").map((r) => r.body)).toEqual([
      { name: "Operations", description: "Runs the install", rights: ["org.view", "members.view"], project_rights: ["project.view"] },
    ]);
  });

  it("changes an unused role's rights in the organization and in every project", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, opsRole]);
    on("PATCH", "orgs/:org/roles/:role", opsRole);
    const { sheet, user } = await openSheet("org:role_ops", "Operations");
    const orgRights = matrix(sheet, "Rights of Operations");
    await user.click(within(orgRights).getByRole("checkbox", { name: /^Edit organization/ }));
    await user.click(within(orgRights).getByRole("checkbox", { name: "View members" }));
    const projectRights = matrix(sheet, "Rights of Operations in every project");
    await user.click(within(projectRights).getByRole("checkbox", { name: /^Edit project/ }));
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    expect(await notifications().findByText("Operations saved")).toBeInTheDocument();
    const body = requests("PATCH", "orgs/:org/roles/role_ops")[0]?.body as { rights: string[]; project_rights: string[] };
    expect([...body.rights].sort()).toEqual(["org.update", "org.view"]);
    expect([...body.project_rights].sort()).toEqual(["project.update", "project.view"]);
  });

  it("deletes an unused role once confirmed", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, opsRole]);
    on("DELETE", "orgs/:org/roles/:role", undefined);
    const { sheet, user, router } = await openSheet("org:role_ops", "Operations");
    await user.click(within(sheet).getByRole("button", { name: "Delete role" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete Operations?" });
    expect(within(confirm).getByText("Nobody holds it; it leaves every role picker in the organization.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Delete role" }));
    expect(await notifications().findByText("Operations deleted")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/roles/role_ops")).toHaveLength(1);
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "roles" }));
  });

  it("keeps the role and says why when the deletion is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, opsRole]);
    on("DELETE", "orgs/:org/roles/:role", apiError(409, "conflict", "An invitation still uses it."));
    const { sheet, user, router } = await openSheet("org:role_ops", "Operations");
    await user.click(within(sheet).getByRole("button", { name: "Delete role" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete Operations?" });
    await user.click(within(confirm).getByRole("button", { name: "Delete role" }));
    expect((await notifications().findAllByText("An invitation still uses it.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(router.state.location.search).toEqual({ tab: "roles", role: "org:role_ops" });
  });

  it("locks a role that holds rights you don't hold", async () => {
    asAdmin();
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, { ...opsRole, rights: ["org.view", "billing.manage"], description: "Pays the bills" }]);
    const { sheet, user } = await openSheet("org:role_ops", "Operations");
    expect(within(sheet).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    expect(within(sheet).getByText("Pays the bills")).toBeInTheDocument();
    await hoverReason(user, within(sheet).getByRole("button", { name: "Delete role" }), "You can't change it: you don't hold Manage billing");
  });

  it("lets an admin change a role that holds only their rights", async () => {
    asAdmin();
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, opsRole]);
    const { sheet } = await openSheet("org:role_ops", "Operations");
    expect(within(sheet).getByRole("textbox", { name: "Name" })).toHaveValue("Operations");
  });

  it("locks a project role that holds rights you don't hold in every project", async () => {
    signedIn({
      session: whoAmI({
        memberships: [
          membership({
            rights: ALL_ORG_RIGHTS.filter((r) => r !== "billing.manage"),
            project_rights: ["project.view", "project.members.view", "api.request", "models.view", "logs.metadata.view", "logs.content.view", "usage.view"],
            role: { id: "admin", kind: "builtin", name: "Admin" },
            preset: "admin",
          }),
        ],
      }),
    });
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet, user } = await openSheet(`project:${LEAD}`, "Support lead");
    expect(within(sheet).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
    await hoverReason(user, within(sheet).getByRole("button", { name: "Delete role" }), "You can't change it: you don't hold View costs");
  });

  it("lets an admin change a project role made of rights they hold in every project", async () => {
    asAdmin();
    on("GET", "orgs/:org/members", paged(holders));
    const { sheet } = await openSheet(`project:${LEAD}`, "Support lead");
    expect(within(sheet).getByRole("textbox", { name: "Name" })).toHaveValue("Support lead");
  });

  it("explains that managing roles needs a right", async () => {
    asAdmin(["org.view", "members.view"]);
    on("GET", "orgs/:org/members", paged(holders));
    withRoles([...roles, opsRole]);
    const { sheet, user } = await openSheet("org:role_ops", "Operations");
    await hoverReason(user, within(sheet).getByRole("button", { name: "Duplicate" }), "You need Manage roles; ask an owner or admin for it.");
    expect(within(sheet).getByRole("button", { name: "Delete role" })).toHaveAttribute("aria-disabled", "true");
    expect(within(sheet).queryByRole("textbox", { name: "Name" })).not.toBeInTheDocument();
  });
});
