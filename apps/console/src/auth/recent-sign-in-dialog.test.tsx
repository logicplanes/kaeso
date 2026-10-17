import { afterEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { emailSent, signedInReply, ssoStart, totpRequired } from "~/test/fixtures/public";
import { apiError, json, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { requestRecentSignIn } from "./recent-sign-in-store";

const PATH = "/settings/account/notifications";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const dialog = () => screen.findByRole("dialog", { name: "Sign in again to continue" });

function needsRecentSignIn(options: { session?: Parameters<typeof whoAmI>[0]; hasPassword?: boolean } = {}) {
  signedIn(options.session ? { session: whoAmI(options.session) } : {});
  on("GET", "me", me({ has_password: options.hasPassword ?? true }));
  on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
  let calls = 0;
  on("PUT", "me/notifications", ({ body }: { body: unknown }) =>
    ++calls === 1 ? apiError(403, "recent_sign_in_required", "Sign in again to do this.") : body,
  );
}

async function toggle(user: Awaited<ReturnType<typeof renderApp>>["user"]) {
  await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
  return dialog();
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("signing in again with a password", () => {
  it("repeats the change once the password is accepted", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", signedInReply());
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(within(box).getByText("This action needs a sign-in from the last 15 minutes.")).toBeInTheDocument();
    expect(within(box).getByRole("button", { name: "Sign in" })).toBeDisabled();
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("POST", "auth/password").map((r) => r.body)).toEqual([{ email: "owner@example.com", password: "correct horse battery" }]);
    expect(requests("PUT", "me/notifications")).toHaveLength(2);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
  });

  it("keeps the dialog open when the password is wrong", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", apiError(401, "authentication", "That email and password don't match."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "wrong one");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    expect((await (await notifications()).findAllByText("That email and password don't match.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Sign in again to continue" })).toBeInTheDocument();
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
  });

  it("drops the change quietly when cancelled", async () => {
    needsRecentSignIn();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(within(box).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
    expect(screen.queryByText("Sign in again to do this.")).not.toBeInTheDocument();
  });

  it("closes with Escape", async () => {
    needsRecentSignIn();
    const { user } = await renderApp(PATH);
    await toggle(user);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
  });

  it("asks for the authenticator code next", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", json(totpRequired(), { status: 202 }));
    on("POST", "auth/totp", signedInReply());
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery{Enter}");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    expect(await within(box).findByText("Enter the 6-digit code from your authenticator app.")).toBeInTheDocument();
    expect(within(box).getByRole("button", { name: "Verify" })).toBeDisabled();
    await user.type(within(box).getByRole("textbox"), "123456");
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([{ challenge: "chl_42", code: "123456" }]);
  });

  it("tries the same code again with Verify", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", json(totpRequired(), { status: 202 }));
    let attempts = 0;
    on("POST", "auth/totp", () => (++attempts === 1 ? apiError(401, "authentication", "That code didn't work.") : signedInReply()));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await user.type(await within(box).findByRole("textbox"), "123456");
    expect((await (await notifications()).findAllByText("That code didn't work.")).length).toBeGreaterThan(0);
    await user.click(within(box).getByRole("button", { name: "Verify" }));
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([
      { challenge: "chl_42", code: "123456" },
      { challenge: "chl_42", code: "123456" },
    ]);
  });

  it("asks once when two changes need it at the same time", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", signedInReply());
    const { user } = await renderApp(PATH);
    await screen.findByRole("switch", { name: "Budget alerts" });
    const [first, second] = await act(async () => [requestRecentSignIn(), requestRecentSignIn()]);
    const box = await dialog();
    expect(screen.getAllByRole("dialog", { name: "Sign in again to continue" })).toHaveLength(1);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    expect(await Promise.all([first, second])).toEqual([true, true]);
    expect(requests("POST", "auth/password")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
  });

  it("takes a recovery code instead", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", json(totpRequired(), { status: 202 }));
    on("POST", "auth/totp", signedInReply());
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    expect(await within(box).findByRole("textbox", { name: "Verification code" })).toHaveFocus();
    await user.click(within(box).getByRole("button", { name: "Use a recovery code" }));
    expect(within(box).getByLabelText("Recovery code")).toHaveFocus();
    const verify = within(box).getByRole("button", { name: "Verify" });
    expect(verify).toBeDisabled();
    await user.type(within(box).getByLabelText("Recovery code"), " abcd-efgh ");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([{ challenge: "chl_42", recovery_code: "abcd-efgh" }]);
  });

  it("offers single sign-on next to the password", async () => {
    needsRecentSignIn({ session: { session: { ...whoAmI().session, method: "oidc" } } });
    on("POST", "auth/oidc/start", ssoStart("https://idp.example.com/authorize?state=again"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(within(box).getByRole("button", { name: "Continue with SSO instead" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.example.com/authorize?state=again"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ email: "owner@example.com", next: PATH }]);
  });
});

describe("signing in again without a password", () => {
  it("emails a sign-in link", async () => {
    needsRecentSignIn({ hasPassword: false });
    on("POST", "auth/magic-link", emailSent("A sign-in link is on its way to owner@example.com."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText("We send a sign-in link to owner@example.com. Open it, then repeat the action.")).toBeInTheDocument();
    await user.click(within(box).getByRole("button", { name: "Email me a sign-in link" }));
    expect(await within(box).findByText("A sign-in link is on its way to owner@example.com.")).toBeInTheDocument();
    expect(requests("POST", "auth/magic-link").map((r) => r.body)).toEqual([{ email: "owner@example.com" }]);
    const closes = within(box).getAllByRole("button", { name: "Close" });
    await user.click(closes[closes.length - 1] as HTMLElement);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
  });

  it("sends someone who signed in with single sign-on back to the identity provider", async () => {
    needsRecentSignIn({ hasPassword: false, session: { session: { ...whoAmI().session, method: "oidc" } } });
    on("POST", "auth/oidc/start", ssoStart("https://idp.example.com/authorize?state=again"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText("Sign in again at your identity provider as owner@example.com. You come back to this page.")).toBeInTheDocument();
    await user.click(within(box).getByRole("button", { name: "Continue with SSO" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.example.com/authorize?state=again"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ email: "owner@example.com", next: PATH }]);
  });

  it("sends an account without an email address to the identity provider", async () => {
    needsRecentSignIn({ hasPassword: false, session: { user: { ...whoAmI().user, email: null }, session: { ...whoAmI().session, method: "oidc" } } });
    on("POST", "auth/oidc/start", ssoStart("https://idp.example.com/authorize?state=again"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Continue with SSO" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.example.com/authorize?state=again"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ email: "", next: PATH }]);
  });

  it("says when the gateway's answer can't be read", async () => {
    needsRecentSignIn({ hasPassword: false });
    on("POST", "auth/magic-link", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Email me a sign-in link" }));
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
  });
});
