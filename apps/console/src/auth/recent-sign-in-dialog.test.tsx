import { afterEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { signedIn, whoAmI } from "~/test/fixtures";
import { emailSent, signedInReply, ssoStart, totpRequired } from "~/test/fixtures/public";
import { apiError, json, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { announceConfirmed } from "./confirm-channel";
import { requestRecentSignIn } from "./recent-sign-in-store";

const PATH = "/settings/account/notifications";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const dialog = () => screen.findByRole("dialog", { name: "Sign in again to continue" });

type Ways = {
  password: boolean;
  magic_link: boolean;
  set_password: boolean;
  sso: { org_id: string; org_name: string; provider_name: string | null }[];
};
const ACME_ID = { org_id: "org_01hxacme", org_name: "Acme", provider_name: "Acme ID" };

function needsRecentSignIn(options: { session?: Parameters<typeof whoAmI>[0]; ways?: Partial<Ways> } = {}) {
  signedIn(options.session ? { session: whoAmI(options.session) } : {});
  on("GET", "me/sign-in-again", { password: true, magic_link: true, set_password: false, sso: [], ...options.ways });
  on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
  let calls = 0;
  on("PUT", "me/notifications", ({ body }: { body: unknown }) =>
    ++calls === 1 ? apiError(403, "recent_sign_in_required", "Sign in again to do this.") : body,
  );
}

function popupWindow() {
  const popup = { location: { href: "" }, focus: vi.fn(), close: vi.fn() };
  const open = vi.spyOn(window, "open").mockReturnValue(popup as unknown as Window);
  return { popup, open };
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
    await user.type(await within(box).findByLabelText("Password"), "correct horse battery");
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
    expect(await within(box).findByText("Enter the 6-digit code from your authenticator app.")).toBeInTheDocument();
    expect(requests("POST", "auth/password")).toHaveLength(1);
    expect(within(box).getByRole("button", { name: "Verify" })).toBeDisabled();
    await user.type(within(box).getByRole("textbox"), "123456");
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([{ challenge: "chl_42", code: "123456" }]);
  });

  it("checks one code at a time, and keeps the recovery link waiting meanwhile", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", json(totpRequired(), { status: 202 }));
    let respond: (reply: unknown) => void = () => undefined;
    on("POST", "auth/totp", () => new Promise((resolve) => (respond = resolve)));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await user.type(await within(box).findByRole("textbox"), "123456");
    await waitFor(() => expect(requests("POST", "auth/totp")).toHaveLength(1));
    expect(within(box).getByRole("button", { name: "Use a recovery code" })).toBeDisabled();
    await user.click(within(box).getByRole("button", { name: "Verify" }));
    await user.type(within(box).getByRole("textbox"), "{Backspace}7");
    expect(requests("POST", "auth/totp")).toHaveLength(1);
    respond(signedInReply());
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("POST", "auth/totp")).toHaveLength(1);
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

  it("asks for the password again once the challenge has taken its last code", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", json(totpRequired({ attempts: 1 }), { status: 202 }));
    on("POST", "auth/totp", apiError(401, "authentication", "Too many wrong codes for this sign-in; start signing in again."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await user.type(await within(box).findByRole("textbox"), "000000");
    expect((await (await notifications()).findAllByText("Too many wrong codes for this sign-in; start signing in again.")).length).toBeGreaterThan(0);
    expect(await within(box).findByLabelText("Password")).toHaveValue("");
  });

  it("asks for a recovery code when wrong codes locked the authenticator app", async () => {
    needsRecentSignIn();
    on("POST", "auth/password", json(totpRequired(), { status: 202 }));
    on("POST", "auth/totp", apiError(403, "permission_denied", "Too many wrong codes in a row; sign in with a recovery code or reset your password."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.type(within(box).getByLabelText("Password"), "correct horse battery");
    await user.click(within(box).getByRole("button", { name: "Sign in" }));
    await user.type(await within(box).findByRole("textbox"), "000000");
    expect(await within(box).findByText("Too many wrong codes locked your authenticator app; enter a recovery code.")).toBeInTheDocument();
    expect(within(box).getByLabelText("Recovery code")).toHaveValue("");
    expect(within(box).queryByRole("button", { name: "Use a recovery code" })).not.toBeInTheDocument();
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
    needsRecentSignIn({ ways: { sso: [ACME_ID] } });
    on("POST", "me/sign-in-again/sso", ssoStart("https://idp.example.com/authorize?state=again"));
    const { popup } = popupWindow();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Continue with Acme ID instead" }));
    await waitFor(() => expect(popup.location.href).toBe("https://idp.example.com/authorize?state=again"));
    expect(requests("POST", "me/sign-in-again/sso").map((r) => r.body)).toEqual([{ org: ACME_ID.org_id, next: "/signed-in-again" }]);
  });
});

describe("signing in again without a password", () => {
  it("emails a sign-in link", async () => {
    needsRecentSignIn({ ways: { password: false } });
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

  it("confirms at the identity provider in a window of its own and repeats the change, keeping the page", async () => {
    needsRecentSignIn({ ways: { password: false, sso: [ACME_ID] } });
    on("POST", "me/sign-in-again/sso", ssoStart("https://idp.example.com/authorize?state=again"));
    const { popup, open } = popupWindow();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(
      await within(box).findByText("Sign in again at Acme ID as owner@example.com, in a window that opens; this page stays as it is."),
    ).toBeInTheDocument();
    await user.click(within(box).getByRole("button", { name: "Continue with Acme ID" }));
    expect(open).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(popup.location.href).toBe("https://idp.example.com/authorize?state=again"));
    expect(await within(box).findByText("Finish signing in at Acme ID in the window that opened; the action continues by itself.")).toBeInTheDocument();
    act(() => announceConfirmed(null));
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    expect(requests("PUT", "me/notifications")).toHaveLength(2);
  });

  it("says why the identity provider didn't confirm, and lets the person try again", async () => {
    needsRecentSignIn({ ways: { password: false, sso: [ACME_ID] } });
    on("POST", "me/sign-in-again/sso", ssoStart("https://idp.example.com/authorize?state=again"));
    popupWindow();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Continue with Acme ID" }));
    await within(box).findByRole("button", { name: "Open the window again" });
    act(() => announceConfirmed("other_account"));
    expect((await screen.findAllByText("You signed in at the identity provider as someone else; sign in there as yourself.")).length).toBeGreaterThan(0);
    expect(await within(box).findByRole("button", { name: "Continue with Acme ID" })).toBeInTheDocument();
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
  });

  it("names a provider that can't confirm a recent sign-in, and offers only the other ways afterwards", async () => {
    needsRecentSignIn({ ways: { password: false, sso: [ACME_ID] } });
    on("POST", "me/sign-in-again/sso", ssoStart("https://idp.example.com/authorize?state=again"));
    popupWindow();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Continue with Acme ID" }));
    await within(box).findByRole("button", { name: "Open the window again" });
    on("GET", "me/sign-in-again", { password: false, magic_link: true, set_password: false, sso: [] });
    act(() => announceConfirmed("no_auth_time"));
    expect((await screen.findAllByText("Acme ID can't confirm a recent sign-in; use your password or an email link.")).length).toBeGreaterThan(0);
    expect(await within(box).findByRole("button", { name: "Email me a sign-in link" })).toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: "Continue with Acme ID" })).not.toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: "Continue with Acme ID instead" })).not.toBeInTheDocument();
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
  });

  it("offers to set a password when the only provider can't confirm and no link is sent", async () => {
    needsRecentSignIn({ ways: { password: false, magic_link: false, set_password: true, sso: [] } });
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByRole("button", { name: "Email me a link to set a password" })).toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: /Continue with/ })).not.toBeInTheDocument();
  });

  it("goes to the identity provider in this tab when windows are blocked", async () => {
    needsRecentSignIn({ ways: { password: false, sso: [ACME_ID] } });
    on("POST", "me/sign-in-again/sso", ssoStart("https://idp.example.com/authorize?state=again"));
    vi.spyOn(window, "open").mockReturnValue(null);
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Continue with Acme ID" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.example.com/authorize?state=again"));
    expect(requests("POST", "me/sign-in-again/sso").map((r) => r.body)).toEqual([{ org: ACME_ID.org_id, next: PATH }]);
  });

  it("says when the gateway's answer can't be read", async () => {
    needsRecentSignIn({ ways: { password: false } });
    on("POST", "auth/magic-link", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Email me a sign-in link" }));
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
  });
});

describe("choosing how to sign in again", () => {
  it("asks no password while password sign-in is off and emails a link instead", async () => {
    needsRecentSignIn({ ways: { password: false } });
    on("POST", "auth/magic-link", emailSent("A sign-in link is on its way to owner@example.com."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText(/We send a sign-in link to owner@example.com/)).toBeInTheDocument();
    expect(within(box).queryByLabelText("Password")).not.toBeInTheDocument();
  });

  it("offers only single sign-on when the organization requires it", async () => {
    needsRecentSignIn({ ways: { password: false, magic_link: false, sso: [ACME_ID] } });
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByRole("button", { name: "Continue with Acme ID" })).toBeInTheDocument();
    expect(within(box).queryByLabelText("Password")).not.toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: "Email me a sign-in link instead" })).not.toBeInTheDocument();
  });

  it("offers a sign-in link next to the password", async () => {
    needsRecentSignIn();
    on("POST", "auth/magic-link", emailSent("A sign-in link is on its way to owner@example.com."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Email me a sign-in link instead" }));
    await waitFor(() => expect(requests("POST", "auth/magic-link")).toHaveLength(1));
  });

  it("says so when no method works for the account", async () => {
    needsRecentSignIn({ ways: { password: false, magic_link: false } });
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText("No sign-in method this install offers works for your account; ask an instance admin.")).toBeInTheDocument();
    expect(within(box).queryByRole("button", { name: "Sign in" })).not.toBeInTheDocument();
  });
});

describe("when Kaeso can't tell how to sign in again", () => {
  it("says so and checks again on request instead of guessing a password", async () => {
    signedIn();
    let asked = 0;
    on("GET", "me/sign-in-again", () => (++asked === 1 ? apiError(503, "unavailable", "Try again shortly.") : { password: true, magic_link: false, sso: [] }));
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", apiError(403, "recent_sign_in_required", "Sign in again to do this."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText("Kaeso couldn't check how you can sign in again.")).toBeInTheDocument();
    expect(within(box).queryByLabelText("Password")).not.toBeInTheDocument();
    await user.click(within(box).getByRole("button", { name: "Try again" }));
    expect(await within(box).findByLabelText("Password")).toBeInTheDocument();
  });
});

describe("confirming through one of several identity providers", () => {
  const OTHER = { org_id: "org_01hxglobex", org_name: "Globex", provider_name: null };

  it("offers the session's own provider first, and the others beside it", async () => {
    needsRecentSignIn({
      session: { session: { ...whoAmI().session, method: "oidc", sso_org: OTHER.org_id } },
      ways: { password: false, sso: [ACME_ID, OTHER] },
    });
    on("POST", "me/sign-in-again/sso", ssoStart("https://idp.example.com/authorize?state=again"));
    const { popup } = popupWindow();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByRole("button", { name: "Continue with Globex" })).toBeInTheDocument();
    await user.click(within(box).getByRole("button", { name: "Continue with Acme ID instead" }));
    await waitFor(() => expect(popup.location.href).toBe("https://idp.example.com/authorize?state=again"));
    expect(requests("POST", "me/sign-in-again/sso").map((r) => (r.body as { org: string }).org)).toEqual([ACME_ID.org_id]);
    await user.click(await within(box).findByRole("button", { name: "Open the window again" }));
    await waitFor(() => expect(requests("POST", "me/sign-in-again/sso")).toHaveLength(2));
  });

  it("closes the window again when the confirmation can't start", async () => {
    needsRecentSignIn({ ways: { password: false, sso: [ACME_ID] } });
    on("POST", "me/sign-in-again/sso", apiError(404, "not_found", "SSO connection not found."));
    const { popup } = popupWindow();
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    await user.click(await within(box).findByRole("button", { name: "Continue with Acme ID" }));
    await waitFor(() => expect(popup.close).toHaveBeenCalled());
    expect(within(box).getByRole("button", { name: "Continue with Acme ID" })).toBeInTheDocument();
  });
});

describe("an account without an email address", () => {
  it("is asked to sign in again as itself at its identity provider", async () => {
    needsRecentSignIn({ session: { user: { ...whoAmI().user, email: null } }, ways: { password: false, magic_link: false, sso: [ACME_ID] } });
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText("Sign in again at Acme ID as yourself, in a window that opens; this page stays as it is.")).toBeInTheDocument();
  });
});

describe("an account without a password", () => {
  it("is sent a link to set one when nothing else confirms", async () => {
    needsRecentSignIn({ ways: { password: false, magic_link: false, set_password: true } });
    on("POST", "auth/password-reset", emailSent("If an account exists for this address, we sent it a password reset link."));
    const { user } = await renderApp(PATH);
    const box = await toggle(user);
    expect(await within(box).findByText("Your account has no password yet. We send a link to owner@example.com to set one.")).toBeInTheDocument();
    await user.click(within(box).getByRole("button", { name: "Email me a link to set a password" }));
    expect(
      await within(box).findByText("We sent a link to owner@example.com. Set a password there, sign in with it, then repeat the action."),
    ).toBeInTheDocument();
    expect(requests("POST", "auth/password-reset").map((r) => r.body)).toEqual([{ email: "owner@example.com" }]);
  });
});
