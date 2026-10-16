import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ALL_PROJECT_RIGHTS, USER, asAdmin, signedIn, without } from "~/test/fixtures";
import { hoverReason, member as orgMember, notifications, paged } from "~/test/fixtures/org";
import { grant, me, memberRights, projectData, projectMember, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const MEMBER = "orgs/:org/projects/:project/members/:user";

function oneMember(view = projectMember()) {
  on("GET", MEMBER, view);
  on("GET", `${MEMBER}/rights`, memberRights(["project.view", "models.view"], view.user.id));
}

describe("MembersPage", () => {
  it("lists the members by name with their role, state and join date", async () => {
    signedIn();
    projectData({
      members: [
        projectMember({ user: { id: "usr_zoe", name: "Zoe Quinn", email: null, state: "deleted" } }),
        me(),
        projectMember({ grant: grant("viewer", { rights_added: ["logs.export"] }) }),
        projectMember({ user: { id: "usr_linus", name: "Linus Park", email: "linus@example.com", state: "suspended" } }),
      ],
    });
    await renderApp(projectPath("members"));

    const grid = await screen.findByRole("grid", { name: "Members" });
    const rows = within(grid).getAllByRole("gridcell");
    expect(rows.map((r) => r.textContent)).toEqual([
      expect.stringContaining("Ada Okafor (you)"),
      expect.stringContaining("Grace Hopper"),
      expect.stringContaining("Linus Park"),
      expect.stringContaining("Zoe Quinn"),
    ]);
    expect(rows[0]).toHaveTextContent("Admin");
    expect(rows[1]).toHaveTextContent("Viewer · customized");
    expect(rows[2]).toHaveTextContent("Suspended");
    expect(rows[3]).toHaveTextContent("Deleted");
    expect(rows[1]).toHaveTextContent("15 Oct 2026");
  });

  it("says when the project has no members", async () => {
    signedIn();
    projectData();
    await renderApp(projectPath("members"));
    expect(await screen.findByText("No members")).toBeInTheDocument();
    expect(screen.getByText("Add organization members to give them rights in this project.")).toBeInTheDocument();
  });

  it("adds an organization member who isn't in the project yet", async () => {
    signedIn();
    projectData({ members: [me()] });
    on(
      "GET",
      "orgs/:org/members",
      paged([
        orgMember(),
        orgMember({ user: { id: "usr_grace", name: "Grace Hopper", email: "grace@example.com" } }),
        orgMember({ user: { id: "usr_linus", name: "Linus Park", email: "linus@example.com", state: "suspended" } }),
      ]),
    );
    on("POST", "orgs/:org/projects/:project/members", projectMember({ grant: grant("viewer") }));
    const { user } = await renderApp(projectPath("members"));

    await screen.findByRole("grid", { name: "Members" });
    await user.click(screen.getByRole("button", { name: "Add member" }));
    const dialog = await screen.findByRole("dialog", { name: "Add member" });
    expect(within(dialog).getByText("Add someone who is already in the organization; invite others to the organization first.")).toBeInTheDocument();
    const add = within(dialog).getByRole("button", { name: "Add member" });
    expect(add).toBeDisabled();
    await user.click(within(dialog).getByRole("combobox", { name: "Member" }));
    expect(await screen.findByRole("option", { name: /Grace Hopper/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Linus Park/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Ada Okafor/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: /Grace Hopper/ }));
    await user.click(within(dialog).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add member" }));

    expect(await notifications().findByText("Grace Hopper added to the project")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/projects/:project/members").map((r) => r.body)).toEqual([
      { user_id: "usr_grace", role: "viewer", rights_added: [], rights_removed: [] },
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add member" })).not.toBeInTheDocument());
  });

  it("adds a member without an email address from the c shortcut", async () => {
    signedIn();
    projectData({ members: [me({ user: { id: USER, name: "Ada Okafor", email: "owner@example.com", avatar_url: "/avatars/ada.webp" } })] });
    on(
      "GET",
      "orgs/:org/members",
      paged([orgMember(), orgMember({ user: { id: "usr_noor", name: "Noor Haddad", email: null, avatar_url: "/avatars/noor.webp" } })]),
    );
    on("POST", "orgs/:org/projects/:project/members", projectMember({ user: { id: "usr_noor", name: "Noor Haddad", email: null } }));
    const { user, container } = await renderApp(projectPath("members"));

    await screen.findByRole("grid", { name: "Members" });
    await waitFor(() => expect(container.querySelector('img[src="/avatars/ada.webp"]')).not.toBeNull());
    await user.keyboard("c");
    const dialog = await screen.findByRole("dialog", { name: "Add member" });
    await user.click(within(dialog).getByRole("combobox", { name: "Member" }));
    const option = await screen.findByRole("option", { name: "Noor Haddad" });
    await waitFor(() => expect(option.querySelector('img[src="/avatars/noor.webp"]')).not.toBeNull());
    await user.click(option);
    await user.click(within(dialog).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add member" }));

    expect(await notifications().findByText("Noor Haddad added to the project")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/projects/:project/members").map((r) => r.body)).toEqual([
      { user_id: "usr_noor", role: "viewer", rights_added: [], rights_removed: [] },
    ]);
  });

  it("opens the add dialog from ?add=true and says when everyone is already in", async () => {
    signedIn();
    projectData({ members: [me()] });
    on("GET", "orgs/:org/members", paged([orgMember()]));
    const { user } = await renderApp(projectPath("members?add=true"));

    const dialog = await screen.findByRole("dialog", { name: "Add member" });
    const picker = within(dialog).getByRole("combobox", { name: "Member" });
    await waitFor(() => expect(picker).toHaveAttribute("placeholder", "Every member is in this project"));
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add member" })).not.toBeInTheDocument());
  });

  it("closes the add dialog with Escape", async () => {
    signedIn();
    projectData({ members: [me()] });
    on("GET", "orgs/:org/members", paged([orgMember()]));
    const { user } = await renderApp(projectPath("members?add=true"));

    await screen.findByRole("dialog", { name: "Add member" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add member" })).not.toBeInTheDocument());
  });

  it("explains that choosing a member needs the organization's members", async () => {
    asAdmin(["org.view"], ALL_PROJECT_RIGHTS);
    projectData({ members: [me()] });
    await renderApp(projectPath("members?add=true"));

    const dialog = await screen.findByRole("dialog", { name: "Add member" });
    expect(
      within(dialog).getByText("Choosing a member means seeing the organization's members. You need View members; ask an owner or admin for it."),
    ).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/members")).toHaveLength(0);
  });

  it("shows the API's message when adding is refused", async () => {
    signedIn();
    projectData({ members: [me()] });
    on("GET", "orgs/:org/members", paged([orgMember({ user: { id: "usr_grace", name: "Grace Hopper", email: "grace@example.com" } })]));
    on("POST", "orgs/:org/projects/:project/members", apiError(409, "conflict", "Grace Hopper is already in this project."));
    const { user } = await renderApp(projectPath("members?add=true"));

    const dialog = await screen.findByRole("dialog", { name: "Add member" });
    await user.click(within(dialog).getByRole("combobox", { name: "Member" }));
    await user.click(await screen.findByRole("option", { name: /Grace Hopper/ }));
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect((await notifications().findAllByText("Grace Hopper is already in this project.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Add member" })).toBeInTheDocument();
  });

  it("disables adding without the right and says who can give it", async () => {
    signedIn({ projectRights: without("project.members.add") });
    projectData({ members: [me()] });
    const { user } = await renderApp(projectPath("members"));

    await screen.findByRole("grid", { name: "Members" });
    const add = screen.getByRole("button", { name: "Add member" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, add, "You need Add members; ask Lin Wei for it.");
  });

  it("opens a member's sheet from the list and closes it again", async () => {
    signedIn();
    projectData({ members: [me(), projectMember()] });
    oneMember();
    const { user, router } = await renderApp(projectPath("members"));

    await user.click(await screen.findByRole("gridcell", { name: /Grace Hopper/ }));
    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    await waitFor(() => expect(router.state.location.search).toEqual({ member: "usr_grace" }));
    expect(await within(sheet).findByText("grace@example.com")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(screen.queryByRole("dialog", { name: "Grace Hopper" })).not.toBeInTheDocument();
  });
});

describe("MemberSheet", () => {
  it("shows the member's details and where each right comes from", async () => {
    signedIn();
    projectData({ members: [projectMember({ user: { state: "suspended" } })] });
    oneMember(projectMember({ user: { state: "suspended" } }));
    await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    expect(await within(sheet).findByText("Joined")).toBeInTheDocument();
    expect(within(sheet).getByText("Account")).toBeInTheDocument();
    expect(within(sheet).getByText("Suspended")).toBeInTheDocument();
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toHaveTextContent("Developer");
    expect(within(sheet).getByRole("group", { name: "Grace Hopper's rights in this project" })).toHaveTextContent("View project");
    expect(within(sheet).getByRole("heading", { name: "Models" })).toBeInTheDocument();
    expect(within(sheet).getByText("No limits on this member; the project's limits apply.")).toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: "Remove from project" })).toBeInTheDocument();
  });

  it("says a member has no email address", async () => {
    signedIn();
    projectData({ members: [projectMember({ user: { email: null } })] });
    oneMember(projectMember({ user: { email: null } }));
    await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    expect(await within(sheet).findByText("Joined")).toBeInTheDocument();
    expect(within(sheet).getByText("None")).toBeInTheDocument();
    expect(within(sheet).queryByText("grace@example.com")).not.toBeInTheDocument();
  });

  it("names every source of the member's rights", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    on("GET", MEMBER, projectMember());
    on("GET", `${MEMBER}/rights`, {
      ...memberRights([]),
      rights: [
        { right: "project.view", sources: [{ kind: "project_role", role: { id: "developer", kind: "builtin", name: "Developer" } }] },
        { right: "project.members.view", sources: [{ kind: "project_role" }] },
        { right: "usage.view", sources: [{ kind: "org_role", role: { id: "auditor", kind: "builtin", name: "Auditor" } }] },
        { right: "costs.view", sources: [{ kind: "org_role" }] },
        { right: "keys.all.view", sources: [{ kind: "org_right", right: "projects.view.all" }] },
        { right: "logs.export", sources: [{ kind: "added" }] },
        { right: "models.view", sources: [{ kind: "owner" }] },
        { right: "limits.view", sources: [{ kind: "role" }] },
      ],
    });
    const { user } = await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    const rights = await within(sheet).findByRole("group", { name: "Grace Hopper's rights in this project" });
    for (const source of ["Developer", "Project role", "Auditor · organization", "Org role", "View all projects", "Added", "Owner", "Role"])
      expect(rights).toHaveTextContent(source);
    await user.click(within(sheet).getByRole("button", { name: /Every right/ }));
    expect(within(sheet).getAllByRole("group", { name: "Grace Hopper's rights in this project" })).toHaveLength(2);
  });

  it("changes the member's role and can discard the change", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    oneMember();
    on("PATCH", MEMBER, projectMember({ grant: grant("viewer") }));
    const { user } = await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    const role = await within(sheet).findByRole("combobox", { name: "Role" });
    expect(within(sheet).getByRole("button", { name: "Save role" })).toBeDisabled();
    await user.click(role);
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(sheet).getByRole("button", { name: "Cancel" }));
    expect(within(sheet).getByRole("combobox", { name: "Role" })).toHaveTextContent("Developer");

    await user.click(within(sheet).getByRole("combobox", { name: "Role" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(sheet).getByRole("button", { name: "Save role" }));
    expect(await notifications().findByText("Grace Hopper is Viewer in this project now")).toBeInTheDocument();
    expect(requests("PATCH", MEMBER).map((r) => r.body)).toEqual([{ role: "viewer", rights_added: [], rights_removed: [] }]);
  });

  it("removes the member after confirming", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    oneMember();
    on("DELETE", MEMBER, undefined);
    const { user, router } = await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    await user.click(await within(sheet).findByRole("button", { name: "Remove from project" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Remove Grace Hopper from the project?" });
    expect(within(confirm).getByText("Their keys in this project are archived and stop authenticating; they stay in the organization.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Remove from project" }));
    expect(await notifications().findByText("Grace Hopper removed from the project")).toBeInTheDocument();
    expect(requests("DELETE", MEMBER)).toHaveLength(1);
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("leaves the project from your own sheet and goes to the organization", async () => {
    signedIn();
    projectData({ members: [me()] });
    oneMember(me());
    on("DELETE", MEMBER, undefined);
    const { user, router } = await renderApp(projectPath(`members?member=${USER}`));

    const sheet = await screen.findByRole("dialog", { name: "Ada Okafor" });
    expect(await within(sheet).findByRole("heading", { name: "Leave this project" })).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Leave project" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Leave this project?" });
    const rightsChecks = requests("GET", "orgs/:org/projects/:project/my-rights").length;
    await user.click(within(confirm).getByRole("button", { name: "Leave project" }));
    expect(await notifications().findByText("You left the project")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.pathname).toBe("/orgs/org_northwind"));
    expect(requests("GET", "orgs/:org/projects/:project/my-rights").length).toBeGreaterThan(rightsChecks);
  });

  it("says when the member doesn't exist and goes back to the list", async () => {
    signedIn();
    projectData({ members: [] });
    on("GET", MEMBER, apiError(404, "not_found", "No such member."));
    on("GET", `${MEMBER}/rights`, apiError(404, "not_found", "No such member."));
    const { user, router } = await renderApp(projectPath("members?member=usr_gone"));

    expect(await screen.findByText("This member doesn't exist or was removed.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back to members" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("says when the member couldn't load", async () => {
    signedIn();
    projectData({ members: [] });
    on("GET", MEMBER, apiError(403, "permission_denied", "You can't see this member."));
    on("GET", `${MEMBER}/rights`, memberRights([]));
    await renderApp(projectPath("members?member=usr_grace"));

    expect(await screen.findByText("Couldn't load this member.")).toBeInTheDocument();
    expect((await notifications().findAllByText("You can't see this member.")).length).toBeGreaterThan(0);
  });

  it("keeps a member who holds rights you don't read-only", async () => {
    asAdmin(["org.view", "members.view"], without("logs.export"));
    projectData({ members: [projectMember({ grant: grant("developer", { rights: ["project.view", "logs.export"] }) })] });
    oneMember(projectMember({ grant: grant("developer", { rights: ["project.view", "logs.export"] }) }));
    const { user } = await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    expect(await within(sheet).findByRole("combobox", { name: "Role" })).toBeDisabled();
    const remove = within(sheet).getByRole("button", { name: "Remove from project" });
    expect(remove).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, remove, "Grace Hopper holds rights here you don't hold, so you can't change them");
  });

  it("says which right removing needs and hides limits without View limits", async () => {
    signedIn({ projectRights: without("project.members.remove", "limits.view", "limits.manage") });
    projectData({ members: [projectMember()] });
    oneMember();
    const { user } = await renderApp(projectPath("members?member=usr_grace"));

    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    const remove = await within(sheet).findByRole("button", { name: "Remove from project" });
    await hoverReason(user, remove, "You need Remove members; ask Lin Wei for it.");
    expect(within(sheet).queryByRole("heading", { name: "Models" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("heading", { name: "Limits" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/limits")).toHaveLength(0);
  });
});
