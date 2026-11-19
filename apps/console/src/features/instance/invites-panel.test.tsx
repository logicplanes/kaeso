import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { USER, signedIn } from "~/test/fixtures";
import { instanceUser } from "~/test/fixtures/instance-account";
import { invite, notifications, paged } from "~/test/fixtures/org";
import { apiError, noContent, on, requests } from "~/test/network";
import { atFixtureTime } from "~/test/clock";
import { renderApp } from "~/test/render";

const PATH = "/settings/instance/users?tab=invites";
const INVITES = "instance/users/invites";

const instanceInvite = (o: Parameters<typeof invite>[0] = {}) => invite({ role: null, preset: null, ...o });
const PENDING = instanceInvite({ id: "inv_ada", email: "ada@example.com", expires_at: "2026-10-16T09:00:00Z" });
const LATER = instanceInvite({
  id: "inv_lin",
  email: "lin@example.com",
  invited_by: "usr_gone",
  expires_at: "2026-11-01T12:00:00Z",
  created_at: "2026-10-14T12:00:00Z",
});
const ACCEPTED = instanceInvite({ id: "inv_old", email: "old@example.com", status: "accepted", invited_by: null, created_at: "2026-10-01T12:00:00Z" });

// A state this console doesn't know yet still shows, in a neutral badge.
const ODD = instanceInvite({ id: "inv_odd", email: "odd@example.com", status: "bounced", created_at: "2026-09-01T12:00:00Z" });

function api() {
  on("GET", "instance/users", paged([]));
  on("GET", `instance/users/${USER}`, instanceUser({ id: USER, name: "Ada Okafor" }));
  on("GET", "instance/users/usr_gone", apiError(404, "not_found", "No such user."));
  on("GET", INVITES, ({ query }: { query: URLSearchParams }) => paged(query.get("pending") === "false" ? [PENDING, LATER, ACCEPTED, ODD] : [PENDING, LATER]));
}

const list = () => screen.findByRole("grid", { name: "Invitations" });
const row = async (email: string) => (await within(await list()).findByText(email)).closest('[role="row"]') as HTMLElement;

describe("invitations to the install", () => {
  atFixtureTime();

  it("lists the pending invitations with who sent them and when they expire, and every one under All", async () => {
    signedIn();
    api();
    const { user } = await renderApp(PATH);
    expect(screen.getByRole("tab", { name: "Invitations" })).toHaveAttribute("aria-selected", "true");
    expect(await within(await row("ada@example.com")).findByText(/Invited by Ada Okafor/)).toBeInTheDocument();
    expect(within(await row("ada@example.com")).getByText(/expires in/)).toBeInTheDocument();
    expect(await within(await row("lin@example.com")).findByText(/Invited by an instance admin/)).toBeInTheDocument();
    expect(within(await list()).queryByText("old@example.com")).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await within(await row("old@example.com")).findByText("Accepted")).toBeInTheDocument();
    expect(within(await row("old@example.com")).queryByRole("button")).not.toBeInTheDocument();
    expect(await row("odd@example.com")).toHaveTextContent("Bounced");
    expect(requests("GET", INVITES).map((r) => r.query.get("pending"))).toEqual(["true", "false"]);
  });

  it("resends and revokes a pending invitation", async () => {
    signedIn();
    api();
    on("POST", `${INVITES}/:invite/resend`, PENDING);
    on("DELETE", `${INVITES}/:invite`, noContent());
    const { user } = await renderApp(PATH);
    await user.click(within(await row("lin@example.com")).getByRole("button", { name: "Actions for the invitation to lin@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Resend" }));
    expect(await notifications().findByText("Invitation sent to ada@example.com again")).toBeInTheDocument();
    expect(requests("POST", `${INVITES}/:invite/resend`).map((r) => r.params.invite)).toEqual(["inv_lin"]);

    await user.click(within(await row("ada@example.com")).getByRole("button", { name: "Actions for the invitation to ada@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Revoke" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Revoke the invitation to ada@example.com?" });
    await user.click(within(confirm).getByRole("button", { name: "Revoke invitation" }));
    expect(await notifications().findByText("Invitation revoked")).toBeInTheDocument();
    expect(requests("DELETE", `${INVITES}/:invite`).map((r) => r.params.invite)).toEqual(["inv_ada"]);
  });

  it("closes the confirmation when revoking is refused", async () => {
    signedIn();
    api();
    on("DELETE", `${INVITES}/:invite`, apiError(404, "not_found", "No such pending invite."));
    const { user } = await renderApp(PATH);
    await user.click(within(await row("ada@example.com")).getByRole("button", { name: "Actions for the invitation to ada@example.com" }));
    await user.click(await screen.findByRole("menuitem", { name: "Revoke" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Revoke invitation" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("says when there are no invitations, pending or at all", async () => {
    signedIn();
    on("GET", "instance/users", paged([]));
    on("GET", INVITES, paged([]));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No pending invitations")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "All" }));
    expect(await screen.findByText("No invitations yet")).toBeInTheDocument();
  });

  it("lists the invitations from every page", async () => {
    signedIn();
    on("GET", "instance/users", paged([]));
    on("GET", INVITES, ({ query }: { query: URLSearchParams }) => (query.get("after") === "page2" ? paged([LATER]) : paged([PENDING], "page2")));
    await renderApp(PATH);
    expect(await row("ada@example.com")).toBeInTheDocument();
    expect(await row("lin@example.com")).toBeInTheDocument();
    expect(requests("GET", INVITES).map((r) => r.query.get("after"))).toEqual([null, "page2"]);
  });

  it("says when the invitations couldn't load", async () => {
    signedIn();
    on("GET", "instance/users", paged([]));
    on("GET", INVITES, apiError(503, "unavailable", "Kaeso cannot reach its database right now; try again shortly."));
    await renderApp(PATH);
    expect(await screen.findByText("The invitations couldn't load")).toBeInTheDocument();
  });

  it("switches between users and invitations through the URL", async () => {
    signedIn();
    api();
    const { user, router } = await renderApp("/settings/instance/users");
    await screen.findByRole("heading", { name: "Users", level: 1 });
    await user.click(screen.getByRole("tab", { name: "Invitations" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tab: "invites" }));
    await user.click(screen.getByRole("tab", { name: "Users" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });
});
