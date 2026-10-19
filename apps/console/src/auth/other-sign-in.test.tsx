import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, membership, signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { signedInReply } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const ssoSession = { ...whoAmI().session, method: "oidc", sso_org: "org_contoso" };
const dialog = () => screen.findByRole("dialog", { name: "Sign in another way" });
const methods = { password: true, magic_link: true, sso: true, sso_required: false, sign_up: false, provider: null };

afterEach(() => {
  vi.restoreAllMocks();
});

describe("a session from another organization's single sign-on", () => {
  it("asks for a password before it opens the organization", async () => {
    signedIn({ session: whoAmI({ session: ssoSession, memberships: [membership({ access: "needs_other_sign_in" })] }) });
    on("GET", "me", me({ has_password: true }));
    on("POST", "auth/methods", methods);
    on("POST", "auth/password", signedInReply());
    const { user } = await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByText("Sign in another way to open Northwind Labs")).toBeInTheDocument();
    expect(screen.getByText("Your SSO sign-in opens only its own organization; sign in with your password or an email link.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sign in another way" }));
    const box = await dialog();
    expect(within(box).getByText("You signed in with SSO; this needs your password or an email link.")).toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: "Continue with SSO instead" })).not.toBeInTheDocument();
    await user.type(await within(box).findByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(requests("POST", "auth/password")).toHaveLength(1));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in another way" })).not.toBeInTheDocument());
    await waitFor(() => expect(requests("GET", "auth/session").length).toBeGreaterThan(1));
  });

  it("stays on the page when the person cancels", async () => {
    signedIn({ session: whoAmI({ session: ssoSession, memberships: [membership({ access: "needs_other_sign_in" })] }) });
    on("GET", "me", me({ has_password: true }));
    on("POST", "auth/methods", methods);
    const { user } = await renderApp(`/orgs/${ORG}`);
    await user.click(await screen.findByRole("button", { name: "Sign in another way" }));
    const box = await dialog();
    await user.click(within(box).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in another way" })).not.toBeInTheDocument());
    expect(screen.getByText("Sign in another way to open Northwind Labs")).toBeInTheDocument();
    expect(requests("GET", "auth/session")).toHaveLength(1);
  });
});

describe("an account change from a single sign-on session", () => {
  it("repeats the change after a sign-in with the password", async () => {
    signedIn({ session: whoAmI({ session: ssoSession }) });
    on("GET", "me", me({ has_password: true }));
    on("POST", "auth/methods", methods);
    on("POST", "auth/password", signedInReply());
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    let calls = 0;
    on("PUT", "me/notifications", ({ body }: { body: unknown }) =>
      ++calls === 1 ? apiError(403, "other_sign_in_required", "You signed in with SSO; sign in with your password or a sign-in link to do this.") : body,
    );
    const { user } = await renderApp("/settings/account/notifications");
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    const box = await dialog();
    await user.type(await within(box).findByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(requests("PUT", "me/notifications")).toHaveLength(2));
  });

  it("drops the change when the person doesn't sign in again", async () => {
    signedIn({ session: whoAmI({ session: ssoSession }) });
    on("GET", "me", me({ has_password: true }));
    on("POST", "auth/methods", methods);
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", apiError(403, "other_sign_in_required", "You signed in with SSO; sign in with your password or a sign-in link to do this."));
    const { user } = await renderApp("/settings/account/notifications");
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    const box = await dialog();
    await user.click(within(box).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in another way" })).not.toBeInTheDocument());
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
  });

  it("offers no way when the account has neither a password nor sign-in links", async () => {
    signedIn({ session: whoAmI({ session: ssoSession }) });
    on("GET", "me", me({ has_password: false }));
    on("POST", "auth/methods", { ...methods, password: false, magic_link: false });
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", apiError(403, "other_sign_in_required", "You signed in with SSO; sign in with your password or a sign-in link to do this."));
    const { user } = await renderApp("/settings/account/notifications");
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    const box = await dialog();
    expect(await within(box).findByText("No sign-in method this install offers works for your account; ask an instance admin.")).toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: "Continue with SSO" })).not.toBeInTheDocument();
  });
});

describe("instance administration from a single sign-on session", () => {
  it("asks an instance admin to sign in another way", async () => {
    signedIn({ session: whoAmI({ session: ssoSession }) });
    on("GET", "me", me({ has_password: true }));
    on("POST", "auth/methods", methods);
    const { user } = await renderApp("/settings/instance/system");
    expect(await screen.findByText("Sign in another way to manage the instance")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sign in another way" }));
    const box = await dialog();
    on("POST", "auth/password", signedInReply());
    on("GET", "auth/session", whoAmI());
    await user.type(await within(box).findByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(screen.queryByText("Sign in another way to manage the instance")).not.toBeInTheDocument());
  });
});
