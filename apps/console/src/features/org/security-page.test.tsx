import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { SecurityView } from "~/api/generated/types";
import { ORG, membership, signedIn, whoAmI } from "~/test/fixtures";
import { hoverReason, notifications, security, sso, fill } from "~/test/fixtures/org";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/security`;
const MEMBER_ROLE = { id: "member", kind: "builtin", name: "Member" };

type SsoBody = { issuer: string; client_id: string; client_secret?: string; domains: string[]; default_role: string | null };

function serveSecurity(initial: SecurityView) {
  let current = initial;
  on("GET", "orgs/:org/security", () => current);
  on("PUT", "orgs/:org/security/sso", ({ body }: { body: SsoBody }) => {
    current = {
      ...current,
      sso: sso({ issuer: body.issuer, client_id: body.client_id, domains: body.domains }),
      sso_default_role: body.default_role ? MEMBER_ROLE : null,
    };
    return current;
  });
}

function withRights(rights: string[], overrides: Parameters<typeof whoAmI>[0] = {}) {
  return signedIn({
    session: whoAmI({ memberships: [membership({ rights, role: { id: "admin", kind: "builtin", name: "Admin" }, preset: "admin" })], ...overrides }),
  });
}

describe("organization security", () => {
  it("shows that no identity provider is connected and keys may live forever", async () => {
    signedIn();
    serveSecurity(security());
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Security", level: 1 })).toBeInTheDocument();
    expect(await screen.findByText("Not connected.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Require single sign-on" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "Require two-factor authentication" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "No limit" })).toBeChecked();
    expect(screen.getByText("Keys may never expire.")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Maximum key lifetime" })).not.toBeInTheDocument();
  });

  it("shows the connected identity provider", async () => {
    signedIn();
    serveSecurity(security({ sso: sso({ domains: ["northwind.test", "northwind.example"] }), sso_default_role: MEMBER_ROLE }));
    await renderApp(PATH);
    expect(await screen.findByText("https://login.northwind.test")).toBeInTheDocument();
    expect(screen.getByText("client-123")).toBeInTheDocument();
    expect(screen.getByText("northwind.test")).toBeInTheDocument();
    expect(screen.getByText("northwind.example")).toBeInTheDocument();
    expect(screen.getByText("People without an invitation join as Member.")).toBeInTheDocument();
    expect(screen.getByText("Registered at the provider as the sign-in callback.")).toBeInTheDocument();
    expect(screen.getByText("mk_sso")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Edit" })).toBeInTheDocument();
  });

  it("says only invited people join when no role is set for new members", async () => {
    signedIn();
    serveSecurity(security({ sso: sso() }));
    await renderApp(PATH);
    expect(await screen.findByText("Only invited people can join through the provider.")).toBeInTheDocument();
  });

  it("says when the security settings couldn't load", async () => {
    signedIn();
    on("GET", "orgs/:org/security", apiError(403, "forbidden", "The gateway failed."));
    await renderApp(PATH);
    expect(await screen.findByText("The security settings couldn't load")).toBeInTheDocument();
  });

  it("hides the page from people who can't view the organization", async () => {
    withRights([]);
    serveSecurity(security());
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the security settings here")).toBeInTheDocument();
  });

  it("locks every setting for people without the rights", async () => {
    withRights(["org.view"]);
    serveSecurity(security({ max_key_lifetime_days: 30 }));
    const { user } = await renderApp(PATH);
    const connect = await screen.findByRole("button", { name: "Connect an identity provider" });
    await hoverReason(user, connect, "You need Manage SSO; ask an owner or admin for it.");
    await hoverReason(user, screen.getByRole("switch", { name: "Require single sign-on" }), "You need Require SSO; ask an owner or admin for it.");
    await hoverReason(
      user,
      screen.getByRole("switch", { name: "Require two-factor authentication" }),
      "You need Require two-factor; ask an owner or admin for it.",
    );
    await hoverReason(user, screen.getByRole("switch", { name: "No limit" }), "You need Set key lifetime; ask an owner or admin for it.");
    expect(screen.getByRole("textbox", { name: "Maximum key lifetime" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("says who can change the provider to people who can't manage it", async () => {
    withRights(["org.view"]);
    serveSecurity(security({ sso: sso() }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("https://login.northwind.test")).toBeInTheDocument();
    await hoverReason(user, screen.getByRole("button", { name: "Edit" }), "You need Manage SSO; ask an owner or admin for it.");
    await hoverReason(user, screen.getByRole("button", { name: "Disconnect" }), "You need Manage SSO; ask an owner or admin for it.");
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });
});

describe("connecting an identity provider", () => {
  it("walks through the issuer, the app, the domains and who may join", async () => {
    signedIn();
    serveSecurity(security());
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Connect an identity provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Connect an identity provider" });
    const next = () => within(dialog).getByRole("button", { name: "Continue" });
    await hoverReason(user, next(), "Enter the issuer URL");
    const issuer = within(dialog).getByRole("textbox", { name: "Issuer URL" });
    await fill(user, issuer, "login.example.com");
    await hoverReason(user, next(), "The issuer is a URL, like https://login.example.com");
    await user.clear(issuer);
    await fill(user, issuer, " https://login.example.com ");
    await user.click(next());
    expect(await within(dialog).findByText("The app's client id and secret.")).toBeInTheDocument();
    await hoverReason(user, next(), "Enter the client id");
    await fill(user, within(dialog).getByRole("textbox", { name: "Client id" }), " app-1 ");
    await hoverReason(user, next(), "Enter the client secret");
    await fill(user, within(dialog).getByLabelText("Client secret"), "s3cret");
    await user.click(next());
    await hoverReason(user, await within(dialog).findByRole("button", { name: "Continue" }), "Add at least one email domain");
    const domains = within(dialog).getByRole("combobox", { name: "Email domains" });
    await fill(user, domains, "Example.com nope");
    await user.keyboard("{Enter}");
    expect(await notifications().findByText("nope isn't a domain, like example.com")).toBeInTheDocument();
    expect(await within(dialog).findByText("example.com")).toBeInTheDocument();
    await user.click(next());
    expect(await within(dialog).findByText("Can people join without an invitation?")).toBeInTheDocument();
    expect(within(dialog).getByText("Only invited people can join through the provider.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("combobox", { name: "People without an invitation" }));
    expect(screen.queryByRole("option", { name: /Join as Owner/ })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("option", { name: /Join as Member/ }));
    expect(within(dialog).getByText(/Sees the organization and its members/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Connect" }));
    expect(await notifications().findByText("Identity provider connected")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/sso").map((r) => r.body)).toEqual([
      { issuer: "https://login.example.com", client_id: "app-1", domains: ["example.com"], default_role: "member", client_secret: "s3cret" },
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Connect an identity provider" })).not.toBeInTheDocument());
    expect(await screen.findByText("Register this redirect URL at the provider.")).toBeInTheDocument();
    expect(screen.getByText("People without an invitation join as Member.")).toBeInTheDocument();
  });

  it("refuses text that isn't a domain", async () => {
    signedIn();
    serveSecurity(security());
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Connect an identity provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Connect an identity provider" });
    await fill(user, within(dialog).getByRole("textbox", { name: "Issuer URL" }), "https://login.example.com");
    await user.keyboard("{Enter}");
    await fill(user, await within(dialog).findByRole("textbox", { name: "Client id" }), "app-1");
    await fill(user, within(dialog).getByLabelText("Client secret"), "s3cret");
    await user.keyboard("{Enter}");
    await fill(user, await within(dialog).findByRole("combobox", { name: "Email domains" }), "bad,worse");
    await user.keyboard("{Enter}");
    expect(await notifications().findByText("bad, worse aren't domains, like example.com")).toBeInTheDocument();
  });

  it("edits the provider and keeps the stored secret", async () => {
    signedIn();
    serveSecurity(security({ sso: sso(), sso_default_role: MEMBER_ROLE }));
    on("PUT", "orgs/:org/security/sso", security({ sso: null }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit single sign-on" });
    expect(within(dialog).getByRole("textbox", { name: "Issuer URL" })).toHaveValue("https://login.northwind.test");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    expect(await within(dialog).findByText("The app's client id. Leave the secret empty to keep the stored one.")).toBeInTheDocument();
    expect(within(dialog).getByRole("textbox", { name: "Client id" })).toHaveValue("client-123");
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await fill(user, await within(dialog).findByRole("combobox", { name: "Email domains" }), "Northwind.Example");
    await user.keyboard("{Enter}");
    expect(await within(dialog).findByText("northwind.example")).toBeInTheDocument();
    expect(notifications().queryByText(/isn't a domain/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "People without an invitation" }));
    await user.click(await screen.findByRole("option", { name: /Invite only/ }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Single sign-on saved")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/sso").map((r) => r.body)).toEqual([
      { issuer: "https://login.northwind.test", client_id: "client-123", domains: ["northwind.test", "northwind.example"], default_role: null },
    ]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit single sign-on" })).not.toBeInTheDocument());
  });

  it("closes without saving when cancelled", async () => {
    signedIn();
    serveSecurity(security());
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Connect an identity provider" }));
    const dialog = await screen.findByRole("dialog", { name: "Connect an identity provider" });
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Connect an identity provider" })).not.toBeInTheDocument());
    expect(requests("PUT", "orgs/:org/security/sso")).toHaveLength(0);
  });

  it("disconnects the identity provider once confirmed", async () => {
    signedIn();
    serveSecurity(security({ sso: sso() }));
    on("DELETE", "orgs/:org/security/sso", undefined);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Disconnect" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Disconnect the identity provider?" });
    expect(within(confirm).getByText("Members sign in with a password or a sign-in link again.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Disconnect" }));
    expect(await notifications().findByText("Identity provider disconnected")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/security/sso")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("warns that disconnecting turns the requirement off too, and says why it failed", async () => {
    signedIn();
    serveSecurity(security({ sso: sso(), require_sso: true }));
    on("DELETE", "orgs/:org/security/sso", apiError(409, "conflict", "The provider is still in use."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Disconnect" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Disconnect the identity provider?" });
    expect(
      within(confirm).getByText("Single sign-on is required: it's turned off too, and members sign in with a password or a sign-in link again."),
    ).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Disconnect" }));
    expect((await notifications().findAllByText("The provider is still in use.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });
});

describe("security requirements", () => {
  it("needs a connected provider before single sign-on can be required", async () => {
    signedIn();
    serveSecurity(security());
    const { user } = await renderApp(PATH);
    await hoverReason(user, await screen.findByRole("switch", { name: "Require single sign-on" }), "Connect an identity provider first");
  });

  it("asks before requiring single sign-on of someone who signed in with a password", async () => {
    signedIn();
    serveSecurity(security({ sso: sso() }));
    on("PUT", "orgs/:org/security/require-sso", security({ sso: sso(), require_sso: true }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require single sign-on" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Require single sign-on?" });
    await user.click(within(confirm).getByRole("button", { name: "Require single sign-on" }));
    expect(await notifications().findByText("Single sign-on is required")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/require-sso").map((r) => r.body)).toEqual([{ enabled: true }]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("requires single sign-on straight away for someone who signed in through it", async () => {
    const session = whoAmI();
    signedIn({ session: { ...session, session: { ...session.session, method: "oidc" } } });
    serveSecurity(security({ sso: sso() }));
    on("PUT", "orgs/:org/security/require-sso", apiError(409, "conflict", "Members are still signed in with passwords."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require single sign-on" }));
    expect((await notifications().findAllByText("Members are still signed in with passwords.")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/require-sso").map((r) => r.body)).toEqual([{ enabled: true }]);
  });

  it("stops requiring single sign-on", async () => {
    signedIn();
    serveSecurity(security({ sso: sso(), require_sso: true }));
    on("PUT", "orgs/:org/security/require-sso", security({ sso: sso(), require_sso: false }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require single sign-on" }));
    expect(await notifications().findByText("Single sign-on is no longer required")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/require-sso").map((r) => r.body)).toEqual([{ enabled: false }]);
  });

  it("asks before requiring two-factor of someone who doesn't use it", async () => {
    signedIn();
    serveSecurity(security());
    on("PUT", "orgs/:org/security/require-2fa", security({ require_2fa: true }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require two-factor authentication" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Require two-factor authentication?" });
    await user.click(within(confirm).getByRole("button", { name: "Require two-factor authentication" }));
    expect(await notifications().findByText("Two-factor authentication is required")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/require-2fa").map((r) => r.body)).toEqual([{ enabled: true }]);
  });

  it("closes the two-factor confirmation when it's dismissed or refused", async () => {
    signedIn();
    serveSecurity(security());
    on("PUT", "orgs/:org/security/require-2fa", apiError(409, "conflict", "Members without two-factor are signed in."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require two-factor authentication" }));
    let confirm = await screen.findByRole("alertdialog", { name: "Require two-factor authentication?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await user.click(screen.getByRole("switch", { name: "Require two-factor authentication" }));
    confirm = await screen.findByRole("alertdialog", { name: "Require two-factor authentication?" });
    await user.click(within(confirm).getByRole("button", { name: "Require two-factor authentication" }));
    expect((await notifications().findAllByText("Members without two-factor are signed in.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("changes two-factor straight away for someone who uses it", async () => {
    signedIn({ session: whoAmI({ totp_enabled: true }) });
    serveSecurity(security({ require_2fa: true }));
    on("PUT", "orgs/:org/security/require-2fa", security({ require_2fa: false }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require two-factor authentication" }));
    expect(await notifications().findByText("Two-factor authentication is no longer required")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/require-2fa").map((r) => r.body)).toEqual([{ enabled: false }]);
  });

  it("asks before requiring single sign-on and closes when dismissed", async () => {
    signedIn();
    serveSecurity(security({ sso: sso() }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Require single sign-on" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Require single sign-on?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("PUT", "orgs/:org/security/require-sso")).toHaveLength(0);
  });
});

describe("maximum key lifetime", () => {
  it("sets a limit of 90 days when No limit is turned off", async () => {
    signedIn();
    serveSecurity(security());
    on("PUT", "orgs/:org/security/key-policy", security({ max_key_lifetime_days: 90 }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "No limit" }));
    expect(await notifications().findByText("Key lifetime saved")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/key-policy").map((r) => r.body)).toEqual([{ max_key_lifetime_days: 90 }]);
  });

  it("changes the limit in whole days", async () => {
    signedIn();
    serveSecurity(security({ max_key_lifetime_days: 30 }));
    on("PUT", "orgs/:org/security/key-policy", security({ max_key_lifetime_days: 60 }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("New keys expire within 30 days at most.")).toBeInTheDocument();
    const days = screen.getByRole("textbox", { name: "Maximum key lifetime" });
    expect(days).toHaveValue("30");
    const save = () => screen.getByRole("button", { name: "Save" });
    await hoverReason(user, save(), "No change to save");
    await user.clear(days);
    await hoverReason(user, save(), "Enter a number of days");
    await user.type(days, "0");
    await hoverReason(user, save(), "Enter whole days, at least 1");
    await user.clear(days);
    await user.type(days, "60");
    await user.click(save());
    expect(await notifications().findByText("Key lifetime saved")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/key-policy").map((r) => r.body)).toEqual([{ max_key_lifetime_days: 60 }]);
  });

  it("removes the limit", async () => {
    signedIn();
    serveSecurity(security({ max_key_lifetime_days: 1 }));
    on("PUT", "orgs/:org/security/key-policy", security({ max_key_lifetime_days: null }));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("New keys expire within 1 day at most.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "No limit" }));
    expect(await notifications().findByText("Key lifetime saved")).toBeInTheDocument();
    expect(requests("PUT", "orgs/:org/security/key-policy").map((r) => r.body)).toEqual([{ max_key_lifetime_days: null }]);
  });
});
