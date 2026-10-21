import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, project, signedIn } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { hoverReason, invite, member, notifications, paged, fill } from "~/test/fixtures/org";
import { signedInReply } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/members?invite=true`;

type InviteBody = { email: string; role: string; project_roles: { project_id: string; role: string }[] };

async function openDialog() {
  const view = await renderApp(PATH);
  const dialog = await screen.findByRole("dialog", { name: "Invite members" });
  return { ...view, dialog };
}

describe("inviting members", () => {
  it("sends one invitation with a role and an extra project", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/invites", ({ body }: { body: InviteBody }) => invite({ email: body.email }));
    const { user, dialog, router } = await openDialog();
    expect(within(dialog).getByText("Who are you inviting?")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Add at least one email address");
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "Grace@Example.com");
    await user.keyboard("{Enter}");
    expect(await within(dialog).findByText("grace@example.com")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What can they do in the organization?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: /Developer/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("They join Checkout assistant, the default project; add others now or later.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add a project" }));
    expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveTextContent("Support triage");
    await user.click(within(dialog).getByRole("combobox", { name: "Role in the project" }));
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    expect(within(dialog).queryByRole("button", { name: "Add a project" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Send invitation" }));
    expect(await notifications().findByText("Invitation sent to grace@example.com")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites").map((r) => r.body)).toEqual([
      { email: "grace@example.com", role: "developer", project_roles: [{ project_id: OTHER_PROJECT, role: "viewer" }] },
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Invite members" })).not.toBeInTheDocument());
    expect(router.state.location.search).toEqual({});
  });

  it("takes an address that was typed but not added yet when Continue is pressed", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog } = await openDialog();
    await user.type(within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What can they do in the organization?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    expect(await within(dialog).findByText("grace@example.com")).toBeInTheDocument();
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
  });

  it("adds a project and takes it out again", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Add a project" }));
    await user.click(within(dialog).getByRole("button", { name: "Remove this project" }));
    expect(within(dialog).queryByRole("combobox", { name: "Project" })).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Add a project" })).toBeInTheDocument();
  });

  it("gives each extra project its own role", async () => {
    signedIn({
      projects: [
        project(),
        project({ id: OTHER_PROJECT, name: "Support triage", is_default: false }),
        project({ id: "proj_search", name: "Search ranking", is_default: false }),
        project({ id: "proj_billing", name: "Billing bot", is_default: false }),
      ],
    });
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/invites", ({ body }: { body: InviteBody }) => invite({ email: body.email }));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Add a project" }));
    await user.click(within(dialog).getByRole("button", { name: "Add a project" }));
    const second = () => within(dialog).getAllByRole("combobox", { name: "Project" })[1] as HTMLElement;
    expect(second()).toHaveTextContent("Search ranking");
    await user.click(second());
    await user.click(await screen.findByRole("option", { name: "Billing bot" }));
    expect(second()).toHaveTextContent("Billing bot");
    await user.click(within(dialog).getAllByRole("combobox", { name: "Role in the project" })[1] as HTMLElement);
    await user.click(await screen.findByRole("option", { name: /Viewer/ }));
    await user.click(within(dialog).getByRole("button", { name: "Send invitation" }));
    expect(await notifications().findByText("Invitation sent to grace@example.com")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites").map((r) => r.body)).toEqual([
      {
        email: "grace@example.com",
        role: "member",
        project_roles: [
          { project_id: OTHER_PROJECT, role: "developer" },
          { project_id: "proj_billing", role: "viewer" },
        ],
      },
    ]);
  });

  it("sends nothing when a fresh sign-in is asked for and cancelled", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/invites", apiError(403, "recent_sign_in_required", "Sign in again to do this."));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Send invitation" }));
    const again = await screen.findByRole("dialog", { name: "Sign in again to continue" });
    await user.click(within(again).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "Invite members" })).toBeInTheDocument();
    expect(screen.queryByText(/Sign in again to do this/)).not.toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites")).toHaveLength(1);
  });

  it("sends the rest once a fresh sign-in is given, without sending an address twice", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "me/sign-in-again", { password: true, magic_link: false, sso: [] });
    on("POST", "auth/password", signedInReply());
    let asked = false;
    on("POST", "orgs/:org/invites", ({ body }: { body: InviteBody }) => {
      if (body.email === "b@example.com" && !asked) {
        asked = true;
        return apiError(403, "recent_sign_in_required", "Sign in again to do this.");
      }
      return invite({ email: body.email });
    });
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "a@example.com, b@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Send 2 invitations" }));
    const again = await screen.findByRole("dialog", { name: "Sign in again to continue" });
    await fill(user, within(again).getByLabelText("Password"), "correct horse battery");
    await user.click(within(again).getByRole("button", { name: "Sign in" }));
    expect(await notifications().findByText("2 invitations sent")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites").map((r) => (r.body as InviteBody).email)).toEqual(["a@example.com", "b@example.com", "b@example.com"]);
  });

  it("refuses text that isn't an email address", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace");
    await user.keyboard("{Enter}");
    expect(await within(dialog).findByText("Not an email address; remove it or fix it")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "grace isn't an email address");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(within(dialog).getByText("Who are you inviting?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Remove grace" }));
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("What can they do in the organization?")).toBeInTheDocument();
  });

  it("stays on the addresses while the one being typed isn't an email address, whichever key moves on", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog } = await openDialog();
    const field = within(dialog).getByRole("textbox", { name: "Email addresses" });
    await fill(user, field, "ada@example.com");
    await user.keyboard("{Enter}");
    await user.type(field, "bob");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "bob isn't an email address");
    await user.click(field);
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(within(dialog).getByText("Who are you inviting?")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites")).toHaveLength(0);
  });

  it("sends several invitations at once and keeps the ones that failed", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/invites", ({ body }: { body: InviteBody }) =>
      body.email === "taken@example.com" ? apiError(409, "conflict", "Already a member.") : invite({ email: body.email }),
    );
    const { user, dialog } = await openDialog();
    const emails = within(dialog).getByRole("textbox", { name: "Email addresses" });
    await user.click(emails);
    await user.paste("A@example.com, taken@example.com; b@example.com nope bad a@example.com");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "nope, bad aren't email addresses");
    await user.click(within(dialog).getByRole("button", { name: "Remove nope" }));
    await user.click(within(dialog).getByRole("button", { name: "Remove bad" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Send 3 invitations" }));
    expect((await notifications().findAllByText("taken@example.com: Already a member.")).length).toBeGreaterThan(0);
    expect(await notifications().findByText("2 invitations sent")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites").map((r) => (r.body as InviteBody).email)).toEqual(["a@example.com", "taken@example.com", "b@example.com"]);
    expect(screen.getByRole("dialog", { name: "Invite members" })).toBeInTheDocument();
    expect(within(dialog).getByText("taken@example.com")).toBeInTheDocument();
    expect(within(dialog).queryByText("a@example.com")).not.toBeInTheDocument();
  });

  it("names the number of invitations on the send button", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/invites", apiError(422, "invalid_request", "That address can't be invited.", { param: "email" }));
    const { user, dialog } = await openDialog();
    await user.type(within(dialog).getByRole("textbox", { name: "Email addresses" }), "a@example.com{Enter}b@example.com{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Send 2 invitations" }));
    expect((await notifications().findAllByText("a@example.com: That address can't be invited.")).length).toBeGreaterThan(0);
    expect((await notifications().findAllByText("b@example.com: That address can't be invited.")).length).toBeGreaterThan(0);
    expect(notifications().queryByText(/invitations sent/)).not.toBeInTheDocument();
  });

  it("stops at an error that isn't about one address and keeps the addresses not sent yet", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("POST", "orgs/:org/invites", ({ body }: { body: InviteBody }) =>
      body.email === "a@example.com" ? invite({ email: body.email }) : apiError(503, "unavailable", "Kaeso is busy; try again."),
    );
    const { user, dialog } = await openDialog();
    await user.type(within(dialog).getByRole("textbox", { name: "Email addresses" }), "a@example.com{Enter}b@example.com{Enter}c@example.com{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Send 3 invitations" }));
    expect((await notifications().findAllByText("Kaeso is busy; try again.")).length).toBeGreaterThan(0);
    expect(notifications().queryByText(/b@example.com:/)).not.toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites").map((r) => (r.body as InviteBody).email)).toEqual(["a@example.com", "b@example.com"]);
    await waitFor(() => expect(within(dialog).queryByText("a@example.com")).not.toBeInTheDocument());
    expect(within(dialog).getByText("b@example.com")).toBeInTheDocument();
    expect(within(dialog).getByText("c@example.com")).toBeInTheDocument();
  });

  it("goes to sign-in when the session ended", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog, router } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    on("POST", "orgs/:org/invites", apiError(401, "authentication", "Sign in to continue."));
    on("GET", "auth/session", apiError(401, "authentication", "Sign in to continue."));
    await user.click(await within(dialog).findByRole("button", { name: "Send invitation" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/sign-in"));
    expect(notifications().queryByText(/grace@example.com:/)).not.toBeInTheDocument();
  });

  it("says when the organization has no other projects", async () => {
    signedIn({ projects: [project()] });
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("The organization has no other projects.")).toBeInTheDocument();
  });

  it("names no project when the organization has no default one", async () => {
    signedIn({ projects: [project({ is_default: false }), project({ id: OTHER_PROJECT, name: "Support triage", is_default: false })] });
    on("GET", "orgs/:org/members", paged([member()]));
    const { user, dialog } = await openDialog();
    await fill(user, within(dialog).getByRole("textbox", { name: "Email addresses" }), "grace@example.com");
    await user.keyboard("{Enter}");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("They join the default project; add others now or later.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add a project" }));
    await user.click(within(dialog).getByRole("combobox", { name: "Project" }));
    await user.click(await screen.findByRole("option", { name: "Support triage" }));
    expect(within(dialog).getByRole("combobox", { name: "Project" })).toHaveTextContent("Support triage");
    expect(within(dialog).getByRole("button", { name: "Add a project" })).toBeInTheDocument();
    expect(requests("GET", `orgs/:org/projects/${PROJECT}/my-rights`).length).toBeGreaterThan(0);
  });
});
