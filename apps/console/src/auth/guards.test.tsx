import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ALL_ORG_RIGHTS, ORG, membership, myMembership, org, signedIn, whoAmI } from "~/test/fixtures";
import { member, paged } from "~/test/fixtures/org";
import { ssoStart } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "~/query";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const CONTOSO = "org_contoso";
const DUE = "2026-10-22T12:00:00Z";

function inOrg(overrides: Parameters<typeof membership>[0]) {
  return signedIn({ session: whoAmI({ memberships: [membership(overrides)] }) });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("an organization that requires two-factor authentication", () => {
  it("asks the person to set it up instead of showing the organization", async () => {
    inOrg({ access: "needs_2fa" });
    await renderApp(`/orgs/${ORG}`);
    expect(
      await screen.findByText("Northwind Labs requires two-factor authentication. Set it up for your account, then sign in again with a code."),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Set up two-factor authentication" })).toHaveAttribute("href", "/settings/account/security");
  });
});

describe("an organization that requires single sign-on", () => {
  it("sends the person to the organization's identity provider", async () => {
    inOrg({ access: "needs_sso" });
    on("POST", "auth/oidc/start", ssoStart("https://idp.northwind.test/authorize"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("Sign in through Northwind Labs's identity provider")).toBeInTheDocument();
    expect(screen.getByText("Northwind Labs requires single sign-on. Your session works for your other organizations.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Continue with SSO" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.northwind.test/authorize"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ org: ORG }]);
  });

  it("says why single sign-on couldn't start", async () => {
    inOrg({ access: "needs_sso" });
    on("POST", "auth/oidc/start", apiError(422, "invalid_request", "Single sign-on isn't configured."));
    const { user } = await renderApp(`/orgs/${ORG}`);
    await user.click(await screen.findByRole("button", { name: "Continue with SSO" }));
    expect((await (await notifications()).findAllByText("Single sign-on isn't configured.")).length).toBeGreaterThan(0);
  });

  it("asks to try again when the answer to starting it can't be read", async () => {
    inOrg({ access: "needs_sso" });
    on("POST", "auth/oidc/start", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp(`/orgs/${ORG}`);
    await user.click(await screen.findByRole("button", { name: "Continue with SSO" }));
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
  });
});

describe("an organization scheduled for deletion", () => {
  it("lets someone who can delete it restore it", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: DUE });
    on("POST", "orgs/:org/restore", org());
    const { user } = await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("This organization is scheduled for deletion")).toBeInTheDocument();
    expect(screen.getByText("It's read-only and its keys stopped working; on 22 Oct 2026 it's deleted for good.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Restore organization" }));
    expect(await (await notifications()).findByText("Northwind Labs restored")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/restore").map((r) => r.params.org)).toEqual([ORG]);
  });

  it("says it's deleted soon when there's no date", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: null });
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("It's read-only and its keys stopped working; it's deleted for good soon.")).toBeInTheDocument();
  });

  it("names the owner to ask when the person can't restore it", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: DUE, rights: ["org.view", "members.view"] });
    on(
      "GET",
      "orgs/:org/members",
      paged([
        member(),
        member({
          user: { id: "usr_grace", name: "Grace Hopper" },
          grant: { role: { id: "developer", kind: "builtin", name: "Developer" }, rights: [], rights_added: [], rights_removed: [] },
        }),
      ]),
    );
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("It's read-only until 22 Oct 2026; ask Ada Okafor to restore it.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Restore organization" })).not.toBeInTheDocument();
  });

  it("names either of two owners without a date", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: null, rights: ["org.view", "members.view"] });
    on("GET", "orgs/:org/members", paged([member(), member({ user: { id: "usr_grace", name: "Grace Hopper" } })]));
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("It's read-only until it's deleted; ask Ada Okafor or Grace Hopper to restore it.")).toBeInTheDocument();
  });

  it("asks for an owner when the person can't see the members", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: DUE, rights: ["org.view"] });
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("It's read-only until 22 Oct 2026; ask an owner to restore it.")).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/members")).toHaveLength(0);
  });

  it("asks for an owner without a date when there are many", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: null, rights: ["org.view", "members.view"] });
    on(
      "GET",
      "orgs/:org/members",
      paged([member(), member({ user: { id: "usr_grace", name: "Grace Hopper" } }), member({ user: { id: "usr_linus", name: "Linus" } })]),
    );
    await renderApp(`/orgs/${ORG}`);
    await waitFor(() => expect(requests("GET", "orgs/:org/members")).toHaveLength(1));
    expect(await screen.findByText("It's read-only until it's deleted; ask an owner to restore it.")).toBeInTheDocument();
  });

  it("shows a shorter notice on the organization's settings pages", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: DUE });
    await renderApp(`/settings/orgs/${ORG}/general`);
    expect(await screen.findByText("It's read-only until it's deleted for good on 22 Oct 2026.")).toBeInTheDocument();
  });

  it("shows the shorter notice without a date", async () => {
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: null });
    await renderApp(`/settings/orgs/${ORG}/general`);
    expect(await screen.findByText("It's read-only until it's deleted for good.")).toBeInTheDocument();
  });
});

describe("an organization whose deletion comes due", () => {
  const fakeClock = () => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"], shouldAdvanceTime: true });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("looks for the organization's memberships again every minute once the date has passed", async () => {
    fakeClock();
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: new Date(Date.now() - 1000).toISOString() });
    await renderApp(`/orgs/${ORG}`);
    await screen.findByText("This organization is scheduled for deletion");
    await vi.advanceTimersByTimeAsync(1000);
    const before = requests("GET", "me/memberships").length;
    await vi.advanceTimersByTimeAsync(60_000);
    await waitFor(() => expect(requests("GET", "me/memberships")).toHaveLength(before + 1));
    await vi.advanceTimersByTimeAsync(60_000);
    await waitFor(() => expect(requests("GET", "me/memberships")).toHaveLength(before + 2));
  });

  it("looks for the organization's memberships again as soon as the date passes", async () => {
    fakeClock();
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: new Date(Date.now() + 120_000).toISOString() });
    await renderApp(`/orgs/${ORG}`);
    await screen.findByText("This organization is scheduled for deletion");
    const before = requests("GET", "me/memberships").length;
    await vi.advanceTimersByTimeAsync(110_000);
    expect(requests("GET", "me/memberships")).toHaveLength(before);
    await vi.advanceTimersByTimeAsync(10_000);
    await waitFor(() => expect(requests("GET", "me/memberships")).toHaveLength(before + 1));
  });

  it("doesn't look again while the date is weeks away", async () => {
    fakeClock();
    inOrg({ org_state: "scheduled_for_deletion", org_deletion_due_at: new Date(Date.now() + 30 * 86_400_000).toISOString() });
    await renderApp(`/orgs/${ORG}`);
    await screen.findByText("This organization is scheduled for deletion");
    const before = requests("GET", "me/memberships").length;
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(requests("GET", "me/memberships")).toHaveLength(before);
  });
});

describe("leaving an organization that's gone", () => {
  it("goes home from an organization the person doesn't belong to", async () => {
    signedIn();
    await renderApp(`/orgs/${CONTOSO}`);
    expect(await (await notifications()).findByText("You don't belong to that organization")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("says the person no longer belongs to an organization they were in", async () => {
    const both = whoAmI({ memberships: [membership(), membership({ org_id: CONTOSO, org_name: "Contoso", rights: ALL_ORG_RIGHTS })] });
    signedIn({ session: both, memberships: [myMembership(), myMembership({ org_id: CONTOSO, org_name: "Contoso", projects: [] })] });
    await renderApp(`/orgs/${CONTOSO}`);
    await waitFor(() => expect(requests("GET", "auth/session").length).toBeGreaterThan(0));
    on("GET", "auth/session", whoAmI());
    on("GET", "me/memberships", { data: [myMembership()] });
    await queryClient.invalidateQueries({ queryKey: ["session"] });
    expect(await (await notifications()).findByText("You no longer belong to Contoso")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("says an organization scheduled for deletion was deleted", async () => {
    const both = whoAmI({ memberships: [membership(), membership({ org_id: CONTOSO, org_name: "Contoso", org_state: "scheduled_for_deletion" })] });
    signedIn({ session: both, memberships: [myMembership(), myMembership({ org_id: CONTOSO, org_name: "Contoso", projects: [] })] });
    await renderApp(`/orgs/${CONTOSO}`);
    await screen.findByText("This organization is scheduled for deletion");
    on("GET", "auth/session", whoAmI());
    on("GET", "me/memberships", { data: [myMembership()] });
    await queryClient.invalidateQueries({ queryKey: ["session"] });
    expect(await (await notifications()).findByText("Contoso was deleted")).toBeInTheDocument();
  });

  it("stays when the fresh session still has the organization", async () => {
    const session = signedIn();
    const both = whoAmI({ memberships: [membership(), membership({ org_id: CONTOSO, org_name: "Contoso" })] });
    let calls = 0;
    on("GET", "auth/session", () => (++calls === 1 ? session : both));
    await renderApp(`/orgs/${CONTOSO}`);
    await waitFor(() => expect(calls).toBe(2));
    await waitFor(() => expect(queryClient.getQueryData<typeof both>(["session"])?.memberships).toHaveLength(2));
    expect(window.location.pathname).toBe(`/orgs/${CONTOSO}`);
    expect(screen.queryByText("You don't belong to that organization")).not.toBeInTheDocument();
  });

  it("stays when the session can't be checked again", async () => {
    const session = signedIn();
    let calls = 0;
    on("GET", "auth/session", () => (++calls === 1 ? session : apiError(429, "rate_limited", "Too many requests; slow down.")));
    await renderApp(`/orgs/${CONTOSO}`);
    expect((await (await notifications()).findAllByText("Too many requests; slow down.")).length).toBeGreaterThan(0);
    expect(window.location.pathname).toBe(`/orgs/${CONTOSO}`);
    expect(screen.queryByText("You don't belong to that organization")).not.toBeInTheDocument();
  });
});
