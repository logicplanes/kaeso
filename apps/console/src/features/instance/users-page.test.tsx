import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { InstanceUserView } from "~/api/generated/types";
import { USER, signedIn } from "~/test/fixtures";
import { instanceUser } from "~/test/fixtures/instance-account";
import { hoverReason, notifications, paged } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/users";
const GRACE = instanceUser();
const ADA = instanceUser({ id: USER, name: "Ada Okafor", email: "owner@example.com", is_instance_admin: true });
const LINUS = instanceUser({ id: "usr_linus", name: "Linus Park", email: "linus@example.com", state: "deactivated" });

function users(list: InstanceUserView[] = [ADA, GRACE, LINUS]) {
  on("GET", "instance/users", ({ query }: { query: URLSearchParams }) =>
    paged(list.filter((u) => (!query.get("search") || u.name.includes(query.get("search") ?? "")) && (!query.get("state") || u.state === query.get("state")))),
  );
  for (const u of list) on("GET", `instance/users/${u.id}`, u);
}

const userQueries = () => requests("GET", "instance/users").map((r) => Object.fromEntries(r.query));
const table = () => screen.getByRole("table", { name: "Users" });

async function openUser(u: InstanceUserView) {
  users();
  on("GET", `instance/users/${u.id}`, u);
  const rendered = await renderApp(`${PATH}?user=${u.id}`);
  const sheet = await screen.findByRole("dialog", { name: u.name });
  await within(sheet).findByText(u.id);
  return { ...rendered, sheet };
}

describe("instance users list", () => {
  it("lists everyone with their email, state and creation day", async () => {
    signedIn();
    users();
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Users", level: 1 })).toBeInTheDocument();
    expect(await within(table()).findByText("Grace Hopper")).toBeInTheDocument();
    expect(within(table()).getByText("Instance admin")).toBeInTheDocument();
    expect(within(table()).getByText("linus@example.com")).toBeInTheDocument();
    expect(within(table()).getByText("Deactivated")).toBeInTheDocument();
    expect(within(table()).getAllByText("Active")).toHaveLength(2);
    expect(within(table()).getAllByText("15 Oct 2026")).toHaveLength(3);
    expect(userQueries()[0]).toEqual({ limit: "100" });
  });

  it("gives the creation day room for it in every language", async () => {
    signedIn();
    users();
    await renderApp(PATH, { locale: "zh-CN" });
    const row = (await within(await screen.findByRole("table", { name: "用户" })).findByText("Grace Hopper")).closest<HTMLElement>("[role=row]");
    expect(row?.style.gridTemplateColumns.split(" ").at(-1)).toBe("160px");
  });

  it("searches by name or email through the URL", async () => {
    signedIn();
    users();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Grace Hopper");
    const search = screen.getByRole("searchbox", { name: "Search users" });
    await user.type(search, "  Nobody {Enter}");
    expect(await screen.findByText("No users match")).toBeInTheDocument();
    expect(screen.getByText("Search by another name or email address.")).toBeInTheDocument();
    expect(new URLSearchParams(window.location.search).get("q")).toBe("Nobody");
    expect(userQueries().at(-1)).toEqual({ limit: "100", search: "Nobody" });
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(await within(table()).findByText("Grace Hopper")).toBeInTheDocument();
    expect(window.location.search).toBe("");
  });

  it("searches when the field loses focus", async () => {
    signedIn();
    users();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Grace Hopper");
    await user.type(screen.getByRole("searchbox", { name: "Search users" }), "Grace");
    await user.tab();
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("q")).toBe("Grace"));
    await waitFor(() => expect(within(table()).queryByText("Linus Park")).not.toBeInTheDocument());
  });

  it("filters by state through the URL and says when nobody is in it", async () => {
    signedIn();
    users([GRACE]);
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Grace Hopper");
    await user.click(screen.getByRole("tab", { name: "Deactivated" }));
    expect(await screen.findByText("No deactivated users")).toBeInTheDocument();
    expect(new URLSearchParams(window.location.search).get("state")).toBe("deactivated");
    expect(userQueries().at(-1)).toEqual({ limit: "100", state: "deactivated" });
    await user.click(screen.getByRole("tab", { name: "Deleted" }));
    expect(await screen.findByText("No deleted users")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await within(table()).findByText("Grace Hopper")).toBeInTheDocument();
    expect(window.location.search).toBe("");
  });

  it("says when there are no active users", async () => {
    signedIn();
    users([]);
    await renderApp(`${PATH}?state=active`);
    expect(await screen.findByText("No active users")).toBeInTheDocument();
  });

  it("says when there are no users", async () => {
    signedIn();
    users([]);
    await renderApp(PATH);
    expect(await screen.findByText("No users")).toBeInTheDocument();
  });

  it("loads the next page on request", async () => {
    signedIn();
    on("GET", "instance/users", ({ query }: { query: URLSearchParams }) => (query.get("after") === "cur_2" ? paged([LINUS]) : paged([ADA, GRACE], "cur_2")));
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Grace Hopper");
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await within(table()).findByText("Linus Park")).toBeInTheDocument();
    expect(userQueries().at(-1)).toEqual({ limit: "100", after: "cur_2" });
  });

  it("shows a user's picture and leaves the email empty when there is none", async () => {
    signedIn();
    users([ADA, instanceUser({ avatar_url: "/avatars/grace.webp", email: null })]);
    await renderApp(PATH);
    const row = (await within(table()).findByText("Grace Hopper")).closest("[role=row]") as HTMLElement;
    expect(row).not.toHaveTextContent("grace@example.com");
    expect(within(table()).getByText("owner@example.com")).toBeInTheDocument();
  });

  it("clears the search when the field is emptied, with Enter or by leaving it", async () => {
    signedIn();
    users();
    const { user } = await renderApp(`${PATH}?q=Grace`);
    await within(table()).findByText("Grace Hopper");
    const search = screen.getByRole("searchbox", { name: "Search users" });
    await user.clear(search);
    await user.type(search, "   {Enter}");
    await waitFor(() => expect(window.location.search).toBe(""));
    expect(await within(table()).findByText("Linus Park")).toBeInTheDocument();

    await user.type(search, "Linus");
    await user.tab();
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("q")).toBe("Linus"));
    await user.clear(search);
    await user.tab();
    await waitFor(() => expect(window.location.search).toBe(""));
  });

  it("opens the invitation with the create shortcut", async () => {
    signedIn();
    users();
    const { user } = await renderApp(PATH);
    await within(table()).findByText("Grace Hopper");
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: "Invite user" })).toBeInTheDocument();
  });

  it("says when the users couldn't load", async () => {
    signedIn();
    on("GET", "instance/users", apiError(403, "permission_denied", "Only instance admins can list users."));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    expect((await notifications().findAllByText("Only instance admins can list users.")).length).toBeGreaterThan(0);
    users();
    await user.click(within(table()).getByRole("button", { name: "Try again" }));
    expect(await within(table()).findByText("Grace Hopper")).toBeInTheDocument();
  });

  it("opens a user's sheet from the row", async () => {
    signedIn();
    users();
    const { user } = await renderApp(PATH);
    await user.click(await within(table()).findByText("Grace Hopper"));
    const sheet = await screen.findByRole("dialog", { name: "Grace Hopper" });
    expect(await within(sheet).findByText("usr_grace")).toBeInTheDocument();
    expect(new URLSearchParams(window.location.search).get("user")).toBe("usr_grace");
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("user")).toBeNull());
  });
});

describe("inviting a user", () => {
  it("sends an invitation to a valid address", async () => {
    signedIn();
    users();
    on("POST", "instance/users/invites", ({ body }: { body: { email: string } }) => ({ email: body.email }));
    const { user } = await renderApp(PATH);
    await user.click(screen.getByRole("button", { name: "Invite user" }));
    const dialog = await screen.findByRole("dialog", { name: "Invite user" });
    await hoverReason(user, within(dialog).getByRole("button", { name: "Send invitation" }), "Enter an email address");
    await user.type(within(dialog).getByRole("textbox", { name: "Email" }), " new@example.com ");
    await user.click(within(dialog).getByRole("button", { name: "Send invitation" }));
    expect(await notifications().findByText("Invitation sent to new@example.com")).toBeInTheDocument();
    expect(requests("POST", "instance/users/invites").map((r) => r.body)).toEqual([{ email: "new@example.com" }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Invite user" })).not.toBeInTheDocument());
  });

  it("opens from the URL, submits with the keyboard and keeps the dialog when refused", async () => {
    signedIn();
    users();
    on("POST", "instance/users/invites", apiError(409, "conflict", "That address already has an account."));
    const { user, router } = await renderApp(`${PATH}?invite=true`);
    const dialog = await screen.findByRole("dialog", { name: "Invite user" });
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "instance/users/invites")).toHaveLength(0);
    await user.type(within(dialog).getByRole("textbox", { name: "Email" }), "grace@example.com");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect((await notifications().findAllByText("That address already has an account.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Invite user" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Invite user" })).not.toBeInTheDocument());
    expect(router.state.location.search).toEqual({});
  });
});

describe("closing the invite dialog", () => {
  it("closes with Escape without sending", async () => {
    signedIn();
    users();
    const { user } = await renderApp(`${PATH}?invite=true`);
    await screen.findByRole("dialog", { name: "Invite user" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Invite user" })).not.toBeInTheDocument());
    expect(requests("POST", "instance/users/invites")).toHaveLength(0);
  });
});

describe("a user's sheet", () => {
  it("shows the user's details", async () => {
    signedIn();
    const { sheet } = await openUser(GRACE);
    expect(within(sheet).getByText("grace@example.com")).toBeInTheDocument();
    expect(within(sheet).getByText("Active")).toBeInTheDocument();
    expect(within(sheet).getByText("Off")).toBeInTheDocument();
    expect(within(sheet).getByRole("switch", { name: "Instance admin" })).not.toBeChecked();
    expect(within(sheet).queryByRole("button", { name: "Reset two-factor" })).not.toBeInTheDocument();
  });

  it("lists the organizations a user belongs to, with their role in each", async () => {
    signedIn();
    on("GET", `instance/users/${GRACE.id}/orgs`, {
      data: [
        { id: "org_acme", name: "Acme", state: "active", role: { id: "admin", kind: "builtin", name: "Admin" } },
        { id: "org_old", name: "Old pilot", state: "scheduled_for_deletion", role: { id: "member", kind: "builtin", name: "Member" } },
      ],
      has_more: false,
      next_cursor: null,
    });
    const { sheet, user, router } = await openUser(GRACE);
    const orgs = await within(sheet).findByRole("grid", { name: "Organizations" });
    const acme = await within(orgs).findByRole("gridcell", { name: /Acme/ });
    expect(acme).toHaveTextContent("Admin");
    expect(within(orgs).getByRole("gridcell", { name: /Old pilot/ })).toHaveTextContent("Scheduled for deletion");
    await user.click(acme);
    await waitFor(() => expect(router.state.location.pathname).toBe("/settings/instance/orgs/org_acme"));
  });

  it("says when a user is in no organization", async () => {
    signedIn();
    on("GET", `instance/users/${GRACE.id}/orgs`, { data: [], has_more: false, next_cursor: null });
    const { sheet } = await openUser(GRACE);
    expect(await within(sheet).findByText("In no organization")).toBeInTheDocument();
    expect(within(sheet).getByRole("button", { name: "Delete user" })).toBeInTheDocument();
  });

  it("shows None for a user without an email", async () => {
    signedIn();
    const { sheet } = await openUser(instanceUser({ email: null }));
    expect(within(sheet).getByText("None")).toBeInTheDocument();
  });

  it("deactivates a user after confirming", async () => {
    signedIn();
    on("POST", "instance/users/:user/deactivate", instanceUser({ state: "deactivated" }));
    const { user, sheet } = await openUser(GRACE);
    await user.click(within(sheet).getByRole("button", { name: "Deactivate" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Deactivate Grace Hopper?" });
    await user.click(within(confirm).getByRole("button", { name: "Deactivate" }));
    expect(await notifications().findByText("Grace Hopper deactivated")).toBeInTheDocument();
    expect(requests("POST", "instance/users/:user/deactivate").map((r) => r.params)).toEqual([{ user: "usr_grace" }]);
  });

  it("closes the confirmation and shows why when deactivating is refused", async () => {
    signedIn();
    on("POST", "instance/users/:user/deactivate", apiError(409, "conflict", "Grace is the last owner of an organization."));
    const { user, sheet } = await openUser(GRACE);
    await user.click(within(sheet).getByRole("button", { name: "Deactivate" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Deactivate Grace Hopper?" });
    await user.click(within(confirm).getByRole("button", { name: "Deactivate" }));
    expect((await notifications().findAllByText("Grace is the last owner of an organization.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("reactivates a deactivated user", async () => {
    signedIn();
    on("POST", "instance/users/:user/reactivate", { ...LINUS, state: "active" });
    const { user, sheet } = await openUser(LINUS);
    expect(within(sheet).queryByRole("button", { name: "Deactivate" })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Reactivate" }));
    expect(await notifications().findByText("Linus Park reactivated")).toBeInTheDocument();
    expect(requests("POST", "instance/users/:user/reactivate")).toHaveLength(1);
  });

  it("ends every session after confirming", async () => {
    signedIn();
    on("POST", "instance/users/:user/sessions/revoke", undefined);
    const { user, sheet } = await openUser(GRACE);
    await user.click(within(sheet).getByRole("button", { name: "End sessions" }));
    const confirm = await screen.findByRole("alertdialog", { name: "End Grace Hopper's sessions?" });
    expect(within(confirm).getByText("They're signed out everywhere and get an email about it.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "End sessions" }));
    expect(await notifications().findByText("Every session ended")).toBeInTheDocument();
    expect(requests("POST", "instance/users/:user/sessions/revoke")).toHaveLength(1);
  });

  it("closes the confirmation when ending sessions is refused", async () => {
    signedIn();
    on("POST", "instance/users/:user/sessions/revoke", apiError(422, "invalid_request", "Grace has no sessions."));
    const { user, sheet } = await openUser(GRACE);
    await user.click(within(sheet).getByRole("button", { name: "End sessions" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "End sessions" }));
    expect((await notifications().findAllByText("Grace has no sessions.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("resets two-factor authentication for a user who has it", async () => {
    signedIn();
    on("POST", "instance/users/:user/reset-2fa", undefined);
    const { user, sheet } = await openUser(instanceUser({ totp_enabled: true }));
    expect(within(sheet).getByText("On")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Reset two-factor" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Reset Grace Hopper's two-factor authentication?" });
    await user.click(within(confirm).getByRole("button", { name: "Reset two-factor" }));
    expect(await notifications().findByText("Two-factor authentication reset")).toBeInTheDocument();
    expect(requests("POST", "instance/users/:user/reset-2fa")).toHaveLength(1);
  });

  it("closes the confirmation when the reset is refused", async () => {
    signedIn();
    on("POST", "instance/users/:user/reset-2fa", apiError(409, "conflict", "Two-factor is already off."));
    const { user, sheet } = await openUser(instanceUser({ totp_enabled: true }));
    await user.click(within(sheet).getByRole("button", { name: "Reset two-factor" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Reset two-factor" }));
    expect((await notifications().findAllByText("Two-factor is already off.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("makes a user an instance admin and takes it away again", async () => {
    signedIn();
    on("PUT", "instance/users/:user/instance-admin", ({ body }: { body: { is_instance_admin: boolean } }) => {
      on("GET", "instance/users/usr_grace", instanceUser({ is_instance_admin: body.is_instance_admin }));
      return instanceUser({ is_instance_admin: body.is_instance_admin });
    });
    const { user, sheet } = await openUser(GRACE);
    await user.click(within(sheet).getByRole("switch", { name: "Instance admin" }));
    expect(await notifications().findByText("Grace Hopper is an instance admin")).toBeInTheDocument();
    await waitFor(() => expect(within(sheet).getByRole("switch", { name: "Instance admin" })).toBeChecked());
    await user.click(within(sheet).getByRole("switch", { name: "Instance admin" }));
    expect(await notifications().findByText("Grace Hopper is no longer an instance admin")).toBeInTheDocument();
    expect(requests("PUT", "instance/users/:user/instance-admin").map((r) => r.body)).toEqual([{ is_instance_admin: true }, { is_instance_admin: false }]);
  });

  it("deletes a user once their email is typed", async () => {
    signedIn();
    on("DELETE", "instance/users/:user", undefined);
    const { user, sheet } = await openUser(GRACE);
    await user.click(within(sheet).getByRole("button", { name: "Delete user" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete Grace Hopper?" });
    const confirmButton = within(confirm).getByRole("button", { name: "Delete user" });
    expect(confirmButton).toBeDisabled();
    await user.type(within(confirm).getByRole("textbox", { name: "Type grace@example.com to confirm" }), "grace@example.com");
    await user.click(within(confirm).getByRole("button", { name: "Delete user" }));
    expect(await notifications().findByText("User deleted")).toBeInTheDocument();
    expect(requests("DELETE", "instance/users/:user").map((r) => r.params)).toEqual([{ user: "usr_grace" }]);
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("user")).toBeNull());
  });

  it("asks for the name of a user without an email and shows why a deletion is refused", async () => {
    signedIn();
    on("DELETE", "instance/users/:user", apiError(409, "conflict", "Grace still owns an organization."));
    const { user, sheet } = await openUser(instanceUser({ email: null }));
    await user.click(within(sheet).getByRole("button", { name: "Delete user" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Delete Grace Hopper?" });
    await user.type(within(confirm).getByRole("textbox", { name: "Type Grace Hopper to confirm" }), "Grace Hopper");
    await user.click(within(confirm).getByRole("button", { name: "Delete user" }));
    expect((await notifications().findAllByText("Grace still owns an organization.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("won't let you deactivate or delete yourself", async () => {
    signedIn();
    const { user, sheet } = await openUser(ADA);
    await hoverReason(user, within(sheet).getByRole("button", { name: "Deactivate" }), "You can't deactivate yourself");
    await hoverReason(user, within(sheet).getByRole("button", { name: "Delete user" }), "Delete your own account in your profile");
    expect(within(sheet).getByRole("switch", { name: "Instance admin" })).toBeChecked();
  });

  it("offers no actions for a deleted user", async () => {
    signedIn();
    const { sheet } = await openUser(instanceUser({ state: "deleted" }));
    expect(within(sheet).getByText("Deleted")).toBeInTheDocument();
    expect(within(sheet).queryByRole("switch", { name: "Instance admin" })).not.toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Delete user" })).not.toBeInTheDocument();
  });

  it("says when the user doesn't exist", async () => {
    signedIn();
    users();
    on("GET", "instance/users/usr_gone", apiError(404, "not_found", "No such user."));
    const { user } = await renderApp(`${PATH}?user=usr_gone`);
    expect(await screen.findByText("This user doesn't exist.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back to users" }));
    await waitFor(() => expect(new URLSearchParams(window.location.search).get("user")).toBeNull());
  });

  it("says when the user can't be shown", async () => {
    signedIn();
    users();
    on("GET", "instance/users/usr_grace", apiError(403, "permission_denied", "Not allowed."));
    await renderApp(`${PATH}?user=usr_grace`);
    expect(await screen.findByText("This user can't be shown right now.")).toBeInTheDocument();
  });
});
