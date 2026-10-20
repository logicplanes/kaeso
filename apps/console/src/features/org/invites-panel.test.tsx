import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, signedIn, withRights } from "~/test/fixtures";
import { OTHER_USER, invite, member, notifications, paged } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/members?tab=invites`;
const HOUR = 3_600_000;
const DAY = 24 * HOUR;
const fromNow = (ms: number) => new Date(Date.now() + ms).toISOString();
const inviteQueries = () => requests("GET", "orgs/:org/invites").map((r) => r.query.get("pending"));

async function cellOf(email: string) {
  const grid = await screen.findByRole("grid", { name: "Invitations" });
  return within(grid).findByRole("gridcell", { name: new RegExp(email) });
}

describe("organization invitations", () => {
  it("lists pending invitations, newest first, with who sent them and when they expire", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member(), member({ user: { id: OTHER_USER, name: "Grace Hopper" } })]));
    on(
      "GET",
      "orgs/:org/invites",
      paged([
        invite({ id: "inv_old", email: "old@example.com", created_at: "2026-10-01T12:00:00Z", expires_at: fromNow(5 * DAY), invited_by: null }),
        invite({
          id: "inv_new",
          email: "new@example.com",
          created_at: "2026-10-14T12:00:00Z",
          expires_at: fromNow(5 * HOUR),
          invited_by: OTHER_USER,
          project_roles: [
            { project_id: PROJECT, role: { id: "developer", kind: "builtin", name: "Developer" }, preset: "developer" },
            { project_id: OTHER_PROJECT, role: { id: "viewer", kind: "builtin", name: "Viewer" }, preset: "viewer" },
          ],
        }),
        invite({ id: "inv_mid", email: "mid@example.com", created_at: "2026-10-10T12:00:00Z", expires_at: fromNow(-HOUR), invited_by: "usr_left", role: null }),
      ]),
    );
    await renderApp(PATH);
    const grid = await screen.findByRole("grid", { name: "Invitations" });
    await waitFor(() => expect(within(grid).getAllByRole("gridcell")).toHaveLength(3));
    const [newest, middle, oldest] = within(grid).getAllByRole("gridcell") as [HTMLElement, HTMLElement, HTMLElement];
    expect(newest).toHaveTextContent("new@example.com");
    expect(newest).toHaveTextContent("Invited by Grace Hopper · expires in 5 hours");
    expect(newest).toHaveTextContent("Checkout assistant · Developer");
    expect(newest).toHaveTextContent("+1 project");
    expect(newest).toHaveTextContent("Sent 14 Oct 2026");
    expect(middle).toHaveTextContent("Invited by a former member · expired");
    expect(middle).not.toHaveTextContent("Member");
    expect(oldest).toHaveTextContent("Invited by an instance admin · expires in 5 days");
    expect(oldest).toHaveTextContent("Member");
    expect(inviteQueries()).toEqual(["true"]);
  });

  it("offers the pending invitations first and every invitation under All", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([]));
    await renderApp(PATH);
    expect(await screen.findByRole("tablist", { name: "Invitations" })).toBeInTheDocument();
    expect(
      within(screen.getByRole("tablist", { name: "Invitations" }))
        .getAllByRole("tab")
        .map((tab) => tab.textContent),
    ).toEqual(["Pending", "All"]);
  });

  it("shows every invitation with its state under All", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", ({ query }: { query: URLSearchParams }) =>
      query.get("pending") === "true"
        ? paged([])
        : paged([
            invite({ id: "inv_a", email: "accepted@example.com", status: "accepted" }),
            invite({
              id: "inv_r",
              email: "revoked@example.com",
              status: "revoked",
              project_roles: [{ project_id: "proj_gone", role: { id: "viewer", kind: "builtin", name: "Viewer" }, preset: "viewer" }],
            }),
            invite({ id: "inv_e", email: "expired@example.com", status: "expired" }),
            invite({ id: "inv_x", email: "odd@example.com", status: "bounced" }),
          ]),
    );
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No pending invitations")).toBeInTheDocument();
    expect(screen.getByText("Accepted, revoked and expired invitations are under All.")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await cellOf("accepted@example.com")).toHaveTextContent("Accepted");
    expect(await cellOf("revoked@example.com")).toHaveTextContent("Revoked");
    expect(await cellOf("revoked@example.com")).toHaveTextContent("A project · Viewer");
    expect(await cellOf("expired@example.com")).toHaveTextContent("Expired");
    expect(screen.queryByRole("button", { name: /Actions for the invitation/ })).not.toBeInTheDocument();
    expect(inviteQueries()).toEqual(["true", "false"]);
  });

  it("lists the invitations from every page", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "page2"
        ? paged([invite({ id: "inv_2", email: "later@example.com", created_at: "2026-10-01T12:00:00Z" })])
        : paged([invite()], "page2"),
    );
    await renderApp(PATH);
    expect(await cellOf("grace@example.com")).toBeInTheDocument();
    expect(await cellOf("later@example.com")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/invites").map((r) => r.query.get("after"))).toEqual([null, "page2"]);
  });

  it("says when the organization never invited anyone", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("tab", { name: "All" }));
    expect(await screen.findByText("No invitations yet")).toBeInTheDocument();
    expect(screen.getByText("Invite people by email; each link works for 21 days.")).toBeInTheDocument();
  });

  it("says when the invitations couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", apiError(403, "forbidden", "The invitations store is down."));
    await renderApp(PATH);
    expect(await screen.findByText("The invitations couldn't load")).toBeInTheDocument();
  });

  it("sends an invitation again", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([invite()]));
    on("POST", "orgs/:org/invites/:invite/resend", invite());
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for the invitation to grace@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Resend" }));
    expect(await notifications().findByText("Invitation sent to grace@example.com again")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/invites/inv_1/resend")).toHaveLength(1);
  });

  it("revokes an invitation once confirmed", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([invite()]));
    on("DELETE", "orgs/:org/invites/:invite", undefined);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for the invitation to grace@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Revoke" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Revoke the invitation to grace@example.com?" });
    expect(within(confirm).getByText("The link in the email stops working; you can invite the address again later.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Revoke invitation" }));
    expect(await notifications().findByText("Invitation revoked")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/invites/inv_1")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("closes the confirmation and says why when the revoke is refused", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([invite()]));
    on("DELETE", "orgs/:org/invites/:invite", apiError(409, "conflict", "The invitation was accepted already."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for the invitation to grace@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Revoke" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Revoke the invitation to grace@example.com?" });
    await user.click(within(confirm).getByRole("button", { name: "Revoke invitation" }));
    expect((await notifications().findAllByText("The invitation was accepted already.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("cancels a revoke", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([invite()]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for the invitation to grace@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Revoke" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Revoke the invitation to grace@example.com?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("DELETE", "orgs/:org/invites/:invite")).toHaveLength(0);
  });

  it("offers only the actions the person has the rights for", async () => {
    withRights(["org.view", "members.view", "members.invites.revoke"], { role: "member" });
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([invite()]));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Actions for the invitation to grace@example.com" }));
    expect(await screen.findByRole("menuitem", { name: "Revoke" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Resend" })).not.toBeInTheDocument();
  });

  it("offers no actions without the rights to resend or revoke", async () => {
    withRights(["org.view", "members.view"], { role: "member" });
    on("GET", "orgs/:org/members", paged([member()]));
    on("GET", "orgs/:org/invites", paged([invite()]));
    await renderApp(PATH);
    expect(await cellOf("grace@example.com")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Actions for the invitation to grace@example.com" })).not.toBeInTheDocument();
  });
});
