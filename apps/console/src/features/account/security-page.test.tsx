import { afterEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { emailSent, signedInReply } from "~/test/fixtures/public";
import { apiError, noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { cachedText } from "~/test/cache";

const PATH = "/settings/account/security";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const CODES = ["aaaa-bbbb", "cccc-dddd"];
const SETUP = { secret: "JBSWY3DPEHPK3PXP", otpauth_uri: "otpauth://totp/Kaeso:owner@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Kaeso" };

afterEach(() => {
  vi.restoreAllMocks();
});

describe("password", () => {
  it("changes the password with the current one", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/password", { status: "changed" });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Changing it signs out your other sessions.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Change password" }));
    const dialog = await screen.findByRole("dialog", { name: "Change password" });
    const save = within(dialog).getByRole("button", { name: "Change password" });
    expect(save).toBeDisabled();
    await user.type(within(dialog).getByLabelText("New password"), "correct horse battery");
    expect(within(dialog).getByLabelText("New password")).toHaveAccessibleDescription("Long enough.");
    expect(dialog.querySelector('input[autocomplete="username"]')).toHaveValue(me().email);
    expect(within(dialog).getByLabelText("New password")).not.toHaveAttribute("data-1p-ignore");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "me/password")).toHaveLength(0);
    await user.type(within(dialog).getByLabelText("Current password"), "old password");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await (await notifications()).findByText("Password changed; other sessions signed out")).toBeInTheDocument();
    expect(requests("POST", "me/password").map((r) => r.body)).toEqual([{ new_password: "correct horse battery", current_password: "old password" }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Change password" })).not.toBeInTheDocument());
  });

  it("sets a first password without asking for a current one", async () => {
    signedIn();
    on("GET", "me", me({ has_password: false }));
    on("POST", "me/password", { status: "changed" });
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Not set; you sign in with a link or SSO.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Set password" }));
    const dialog = await screen.findByRole("dialog", { name: "Set a password" });
    expect(within(dialog).queryByLabelText("Current password")).not.toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("New password"), "correct horse battery");
    await user.click(within(dialog).getByRole("button", { name: "Set password" }));
    expect(await (await notifications()).findByText("Password set; other sessions signed out")).toBeInTheDocument();
    expect(requests("POST", "me/password").map((r) => r.body)).toEqual([{ new_password: "correct horse battery" }]);
  });

  it("sends a link to set a first password from a single sign-on session, which can't set one itself", async () => {
    signedIn({ session: whoAmI({ session: { ...whoAmI().session, method: "oidc" } }) });
    on("GET", "me", me({ has_password: false }));
    on("GET", "me/sign-in-again", { password: false, magic_link: false, set_password: true, sso: [] });
    on("POST", "auth/password-reset", emailSent("If an account exists for this address, we sent it a password reset link."));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Not set; set one through a link sent to your email.")).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Email me a link" }));
    expect(await (await notifications()).findByText("Link sent to owner@example.com")).toBeInTheDocument();
    expect(requests("POST", "auth/password-reset").map((r) => r.body)).toEqual([{ email: "owner@example.com" }]);
    expect(screen.queryByRole("button", { name: "Set password" })).not.toBeInTheDocument();
  });

  it("sets a first password from a single sign-on session of the organization that manages the account", async () => {
    const managed = whoAmI({ managed_by: { org: "org_acme", name: "Acme" } });
    signedIn({ session: { ...managed, session: { ...managed.session, method: "oidc", sso_org: "org_acme" } } });
    on("GET", "me", me({ has_password: false }));
    await renderApp(PATH);
    expect(await screen.findByText("Not set; you sign in with a link or SSO.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Set password" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Email me a link" })).not.toBeInTheDocument();
  });

  it("keeps the dialog open when the current password is wrong", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/password", apiError(401, "invalid_request", "Your current password is wrong.", { param: "current_password" }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Change password" }));
    const dialog = await screen.findByRole("dialog", { name: "Change password" });
    await user.type(within(dialog).getByLabelText("Current password"), "nope");
    await user.type(within(dialog).getByLabelText("New password"), "correct horse battery");
    await user.click(within(dialog).getByRole("button", { name: "Change password" }));
    expect((await (await notifications()).findAllByText("Your current password is wrong.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Change password" })).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Current password")).toHaveAccessibleDescription("Your current password is wrong.");
    expect(within(dialog).getByLabelText("New password")).not.toHaveAttribute("aria-invalid");
  });

  it("closes the dialog with Escape or Cancel and forgets what was typed", async () => {
    signedIn();
    on("GET", "me", me());
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Change password" }));
    const dialog = await screen.findByRole("dialog", { name: "Change password" });
    await user.type(within(dialog).getByLabelText("Current password"), "old password");
    await user.type(within(dialog).getByLabelText("New password"), "correct horse battery");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Change password" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Change password" }));
    const again = await screen.findByRole("dialog", { name: "Change password" });
    expect(within(again).getByLabelText("Current password")).toHaveValue("");
    expect(within(again).getByLabelText("New password")).toHaveValue("");
    await user.type(within(again).getByLabelText("Current password"), "old password");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Change password" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Change password" }));
    const third = await screen.findByRole("dialog", { name: "Change password" });
    expect(within(third).getByLabelText("Current password")).toHaveValue("");
  });

  it("says when the security settings couldn't load", async () => {
    signedIn();
    on("GET", "me", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("Your security settings couldn't load")).toBeInTheDocument();
  });
});

describe("turning on two-factor authentication", () => {
  it("shows the setup key, confirms the code and hands out recovery codes", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", SETUP);
    on("POST", "me/totp/confirm", { recovery_codes: CODES });
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:codes");
    const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Off; signing in needs only the password or a sign-in link.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    expect(await within(dialog).findByText("JBSW Y3DP EHPK 3PXP")).toBeInTheDocument();
    const app = within(dialog).getByRole("link", { name: "Open in an authenticator app on this device" });
    expect(app).toHaveAttribute("href", SETUP.otpauth_uri);
    expect(app).not.toHaveAttribute("target");
    expect(within(dialog).getByRole("button", { name: "Turn on" })).toBeDisabled();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(requests("POST", "me/totp/confirm")).toHaveLength(0);
    await user.type(within(dialog).getAllByLabelText("Code from the app")[0] as HTMLElement, "123456");
    const codes = await screen.findByRole("dialog", { name: "Store your recovery codes" });
    expect(codes).toBe(dialog);
    expect(within(codes).getByRole("checkbox", { name: "I stored these codes" })).toHaveFocus();
    expect(requests("POST", "me/totp/confirm").map((r) => r.body)).toEqual([{ code: "123456" }]);
    await waitFor(() => expect(requests("GET", "me").length).toBeGreaterThan(1));
    expect(requests("POST", "me/totp")).toHaveLength(1);
    expect(within(codes).getByRole("group", { name: "Recovery codes code" })).toHaveTextContent(/aaaa-bbbb\s*cccc-dddd/);
    await user.click(within(codes).getByRole("button", { name: "Download .txt" }));
    expect(click).toHaveBeenCalledTimes(1);
    expect(createObjectURL).toHaveBeenCalledTimes(1);
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:codes");
    const done = within(codes).getByRole("button", { name: "Finish" });
    expect(done).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: "Store your recovery codes" })).toBeInTheDocument();
    await user.click(within(codes).getByRole("checkbox", { name: "I stored these codes" }));
    await user.click(within(codes).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Store your recovery codes" })).not.toBeInTheDocument());
    await waitFor(() => expect(cachedText()).not.toMatch(/aaaa-bbbb|JBSWY3DPEHPK3PXP/));
  });

  it("confirms one code at a time, even when the code is typed again while the first is checked", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", SETUP);
    let respond: (body: unknown) => void = () => undefined;
    on("POST", "me/totp/confirm", () => new Promise((resolve) => (respond = resolve)));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    await within(dialog).findByText("JBSW Y3DP EHPK 3PXP");
    const field = within(dialog).getAllByLabelText("Code from the app")[0] as HTMLElement;
    await user.type(field, "123456");
    await waitFor(() => expect(requests("POST", "me/totp/confirm")).toHaveLength(1));
    await user.type(field, "{Backspace}7");
    await user.click(within(dialog).getByRole("button", { name: "Turn on" }));
    expect(requests("POST", "me/totp/confirm")).toHaveLength(1);
    respond({ recovery_codes: CODES });
    expect(await screen.findByRole("dialog", { name: "Store your recovery codes" })).toBe(dialog);
    expect(requests("POST", "me/totp/confirm")).toHaveLength(1);
  });

  it("confirms with the Turn on button", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", SETUP);
    on("POST", "me/totp/confirm", apiError(422, "invalid_request", "That code didn't work.", { param: "code" }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    await within(dialog).findByText("JBSW Y3DP EHPK 3PXP");
    const field = within(dialog).getAllByLabelText("Code from the app")[0] as HTMLElement;
    await user.type(field, "000000");
    expect((await (await notifications()).findAllByText("That code didn't work.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Turn on" })).toBeDisabled());
    expect(screen.queryByRole("dialog", { name: "Store your recovery codes" })).not.toBeInTheDocument();
  });

  async function atSetupAfterSkippedSignIn() {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", SETUP);
    let calls = 0;
    on("POST", "me/totp/confirm", () => (++calls === 1 ? apiError(403, "recent_sign_in_required", "Sign in again to do this.") : { recovery_codes: CODES }));
    const view = await renderApp(PATH);
    await view.user.click(await screen.findByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    await within(dialog).findByText("JBSW Y3DP EHPK 3PXP");
    await view.user.type(within(dialog).getAllByLabelText("Code from the app")[0] as HTMLElement, "123456");
    const again = await screen.findByRole("dialog", { name: "Sign in again to continue" });
    await view.user.click(within(again).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Sign in again to continue" })).not.toBeInTheDocument());
    return { ...view, dialog };
  }

  it("keeps the code to turn it on with the button after skipping the new sign-in", async () => {
    const { user, dialog } = await atSetupAfterSkippedSignIn();
    const turnOn = within(dialog).getByRole("button", { name: "Turn on" });
    await waitFor(() => expect(turnOn).not.toHaveAttribute("aria-busy"));
    await user.click(turnOn);
    expect(await screen.findByRole("dialog", { name: "Store your recovery codes" })).toBeInTheDocument();
    expect(requests("POST", "me/totp/confirm").map((r) => r.body)).toEqual([{ code: "123456" }, { code: "123456" }]);
  });

  it("keeps the code to turn it on from the keyboard after skipping the new sign-in", async () => {
    const { user, dialog } = await atSetupAfterSkippedSignIn();
    (within(dialog).getAllByLabelText("Code from the app")[0] as HTMLElement).focus();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(await screen.findByRole("dialog", { name: "Store your recovery codes" })).toBeInTheDocument();
    expect(requests("POST", "me/totp/confirm").map((r) => r.body)).toEqual([{ code: "123456" }, { code: "123456" }]);
  });

  it("closes the setup with Escape", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", SETUP);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    await within(dialog).findByText("JBSW Y3DP EHPK 3PXP");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Set up two-factor authentication" })).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Set up" })).toBeInTheDocument();
  });

  it("closes the setup when the gateway refuses to start it", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", apiError(409, "conflict", "Two-factor authentication is already on."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    expect((await (await notifications()).findAllByText("Two-factor authentication is already on.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Set up two-factor authentication" })).not.toBeInTheDocument());
    expect(requests("POST", "me/totp")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Set up" }));
    await waitFor(() => expect(requests("POST", "me/totp")).toHaveLength(2));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Set up two-factor authentication" })).not.toBeInTheDocument());
    expect(requests("POST", "me/totp")).toHaveLength(2);
  });

  it("starts a new setup with an empty code after one was cancelled half-way", async () => {
    signedIn();
    on("GET", "me", me());
    const second = { secret: "KRSXG5CTMVRXEZLU", otpauth_uri: "otpauth://totp/Kaeso:owner@example.com?secret=KRSXG5CTMVRXEZLU&issuer=Kaeso" };
    let setups = 0;
    on("POST", "me/totp", () => (++setups === 1 ? SETUP : second));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    await within(dialog).findByText("JBSW Y3DP EHPK 3PXP");
    await user.type(within(dialog).getAllByLabelText("Code from the app")[0] as HTMLElement, "123");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Set up two-factor authentication" })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Set up" }));
    const again = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    expect(await within(again).findByText("KRSX G5CT MVRX EZLU")).toBeInTheDocument();
    expect(screen.queryByText("JBSW Y3DP EHPK 3PXP")).not.toBeInTheDocument();
    expect((within(again).getAllByLabelText("Code from the app")[0] as HTMLInputElement).value).toBe("");
    expect(within(again).getByRole("button", { name: "Turn on" })).toBeDisabled();
    expect(requests("POST", "me/totp/confirm")).toHaveLength(0);
  });

  it("cancels the setup", async () => {
    signedIn();
    on("GET", "me", me());
    on("POST", "me/totp", SETUP);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Set up two-factor authentication" })).not.toBeInTheDocument());
    expect(requests("POST", "me/totp/confirm")).toHaveLength(0);
  });
});

describe("starting two-factor setup after a while", () => {
  it("asks to sign in again, then shows the setup key", async () => {
    signedIn();
    on("GET", "me", me());
    on("GET", "me/sign-in-again", { password: true, magic_link: true, sso: [] });
    on("POST", "auth/password", signedInReply());
    let calls = 0;
    on("POST", "me/totp", () => (++calls === 1 ? apiError(403, "recent_sign_in_required", "Sign in again to do this.") : SETUP));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const again = await screen.findByRole("dialog", { name: "Sign in again to continue" });
    await user.type(await within(again).findByLabelText("Password"), "correct horse battery");
    await user.click(within(again).getByRole("button", { name: "Sign in" }));
    const setup = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    expect(await within(setup).findByText("JBSW Y3DP EHPK 3PXP")).toBeInTheDocument();
    expect(requests("POST", "me/totp")).toHaveLength(2);
  });

  it("asks a single sign-on session to sign in another way, then shows the setup key", async () => {
    signedIn({ session: whoAmI({ session: { ...whoAmI().session, method: "oidc", sso_org: "org_contoso" } }) });
    on("GET", "me", me({ has_password: true }));
    on("GET", "me/sign-in-again", { password: true, magic_link: true, set_password: false, sso: [] });
    on("POST", "auth/password", signedInReply());
    let calls = 0;
    on("POST", "me/totp", () =>
      ++calls === 1 ? apiError(403, "other_sign_in_required", "You signed in with SSO; sign in with your password or a sign-in link to do this.") : SETUP,
    );
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Set up" }));
    const again = await screen.findByRole("dialog", { name: "Sign in another way" });
    await user.type(await within(again).findByLabelText("Password"), "correct horse battery");
    await user.click(within(again).getByRole("button", { name: "Sign in" }));
    const setup = await screen.findByRole("dialog", { name: "Set up two-factor authentication" });
    expect(await within(setup).findByText("JBSW Y3DP EHPK 3PXP")).toBeInTheDocument();
    expect(requests("POST", "me/totp")).toHaveLength(2);
  });
});

describe("with two-factor authentication on", () => {
  it("turns it off with a code from the app", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true, recovery_codes_left: 7 }));
    on("DELETE", "me/totp", noContent);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("On, so organizations that require it accept your sign-ins.")).toBeInTheDocument();
    expect(screen.getByText("7 of 10 left; each works for one sign-in.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Turn off" }));
    const confirm = await screen.findByRole("dialog", { name: "Turn off two-factor authentication?" });
    const turnOff = within(confirm).getByRole("button", { name: "Turn off" });
    expect(turnOff).toBeDisabled();
    await user.type(within(confirm).getByLabelText("Code from the app"), "123456");
    await user.click(turnOff);
    expect(await (await notifications()).findByText("Two-factor authentication turned off")).toBeInTheDocument();
    expect(requests("DELETE", "me/totp").map((r) => r.body)).toEqual([{ code: "123456" }]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Turn off two-factor authentication?" })).not.toBeInTheDocument());
  });

  it("turns it off with a recovery code instead", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on("DELETE", "me/totp", noContent);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Turn off" }));
    const confirm = await screen.findByRole("dialog", { name: "Turn off two-factor authentication?" });
    await user.click(within(confirm).getByRole("link", { name: "Use a recovery code" }));
    await user.type(within(confirm).getByRole("textbox", { name: "Recovery code" }), " abcd-efgh ");
    await user.click(within(confirm).getByRole("button", { name: "Turn off" }));
    await waitFor(() => expect(requests("DELETE", "me/totp").map((r) => r.body)).toEqual([{ recovery_code: "abcd-efgh" }]));
  });

  it("keeps the dialog open with a wrong code, and says why it's refused", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on("DELETE", "me/totp", apiError(409, "conflict", "Northwind Labs requires two-factor authentication."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Turn off" }));
    const confirm = await screen.findByRole("dialog", { name: "Turn off two-factor authentication?" });
    const field = within(confirm).getByLabelText("Code from the app");
    await user.type(field, "123456");
    await user.click(within(confirm).getByRole("button", { name: "Turn off" }));
    expect((await (await notifications()).findAllByText("Northwind Labs requires two-factor authentication.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(field).toHaveValue(""));
    expect(screen.getByRole("dialog", { name: "Turn off two-factor authentication?" })).toBeInTheDocument();
  });

  it("says in the person's language when codes from the app are locked", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on(
      "DELETE",
      "me/totp",
      apiError(400, "invalid_request", "Codes from your app are locked after too many wrong ones; use a recovery code.", { param: "code" }),
    );
    const { user } = await renderApp(PATH, { locale: "de" });
    await user.click(await screen.findByRole("button", { name: "Ausschalten" }));
    const confirm = await screen.findByRole("dialog", { name: "Zwei-Faktor-Authentifizierung ausschalten?" });
    await user.type(within(confirm).getByLabelText("Code aus der App"), "123456");
    await user.click(within(confirm).getByRole("button", { name: "Ausschalten" }));
    expect(
      (await screen.findAllByText("Codes aus Ihrer App sind nach zu vielen falschen gesperrt; einen Wiederherstellungscode verwenden.")).length,
    ).toBeGreaterThan(0);
  });

  async function regenerateWith(user: UserEvent, code = "123456") {
    await user.click(await screen.findByRole("button", { name: "Regenerate" }));
    const confirm = await screen.findByRole("dialog", { name: "Regenerate recovery codes?" });
    const regenerate = within(confirm).getByRole("button", { name: "Regenerate" });
    expect(regenerate).toBeDisabled();
    await user.type(within(confirm).getByLabelText("Code from the app"), code);
    await user.click(regenerate);
    return confirm;
  }

  it("regenerates the recovery codes with a code from the app", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on("POST", "me/recovery-codes", { recovery_codes: ["eeee-ffff"] });
    const { user } = await renderApp(PATH);
    await regenerateWith(user);
    const codes = await screen.findByRole("dialog", { name: "Store your recovery codes" });
    expect(within(codes).getByRole("group", { name: "Recovery codes code" })).toHaveTextContent("eeee-ffff");
    expect(requests("POST", "me/recovery-codes").map((r) => r.body)).toEqual([{ code: "123456" }]);
    await user.click(within(codes).getByRole("checkbox", { name: "I stored these codes" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Store your recovery codes" })).not.toBeInTheDocument());
    await waitFor(() => expect(cachedText()).not.toContain("eeee-ffff"));
  });

  it("regenerates the recovery codes with one of the current ones", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on("POST", "me/recovery-codes", { recovery_codes: ["eeee-ffff"] });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Regenerate" }));
    const confirm = await screen.findByRole("dialog", { name: "Regenerate recovery codes?" });
    await user.click(within(confirm).getByRole("link", { name: "Use a recovery code" }));
    await user.type(within(confirm).getByRole("textbox", { name: "Recovery code" }), " abcd-efgh ");
    await user.click(within(confirm).getByRole("button", { name: "Regenerate" }));
    expect(await screen.findByRole("dialog", { name: "Store your recovery codes" })).toBeInTheDocument();
    expect(requests("POST", "me/recovery-codes").map((r) => r.body)).toEqual([{ recovery_code: "abcd-efgh" }]);
  });

  it("keeps the dialog open with a wrong code, and says why", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    on("POST", "me/recovery-codes", apiError(400, "invalid_request", "That code isn't valid or was already used.", { param: "code" }));
    const { user } = await renderApp(PATH);
    const confirm = await regenerateWith(user);
    expect((await within(confirm).findAllByText("That code isn't valid or was already used.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("dialog", { name: "Regenerate recovery codes?" })).toBe(confirm);
    expect(screen.queryByRole("dialog", { name: "Store your recovery codes" })).not.toBeInTheDocument();
  });

  it("keeps the dialog open while new codes are made, so two regenerations can't race", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    let respond: (body: unknown) => void = () => undefined;
    on("POST", "me/recovery-codes", () => new Promise((resolve) => (respond = resolve)));
    const { user } = await renderApp(PATH);
    const confirm = await regenerateWith(user);
    await waitFor(() => expect(requests("POST", "me/recovery-codes")).toHaveLength(1));
    expect(within(confirm).getByRole("button", { name: "Cancel" })).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: "Regenerate recovery codes?" })).toBe(confirm);
    await user.click(within(confirm).getByRole("button", { name: "Regenerate" }));
    expect(requests("POST", "me/recovery-codes")).toHaveLength(1);
    respond({ recovery_codes: ["eeee-ffff"] });
    const codes = await screen.findByRole("dialog", { name: "Store your recovery codes" });
    expect(within(codes).getByRole("group", { name: "Recovery codes code" })).toHaveTextContent("eeee-ffff");
  });

  it("shows only the new recovery codes when they are regenerated again", async () => {
    signedIn();
    on("GET", "me", me({ totp_enabled: true }));
    let rounds = 0;
    on("POST", "me/recovery-codes", () => ({ recovery_codes: ++rounds === 1 ? ["eeee-ffff"] : ["gggg-hhhh"] }));
    const { user } = await renderApp(PATH);
    for (const [shown, gone] of [
      ["eeee-ffff", null],
      ["gggg-hhhh", "eeee-ffff"],
    ] as const) {
      await regenerateWith(user);
      const codes = await screen.findByRole("dialog", { name: "Store your recovery codes" });
      expect(within(codes).getByRole("group", { name: "Recovery codes code" })).toHaveTextContent(shown);
      if (gone) expect(screen.queryByText(gone, { exact: false })).not.toBeInTheDocument();
      expect(within(codes).getByRole("checkbox", { name: "I stored these codes" })).not.toBeChecked();
      await user.click(within(codes).getByRole("checkbox", { name: "I stored these codes" }));
      await user.click(within(codes).getByRole("button", { name: "Finish" }));
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Store your recovery codes" })).not.toBeInTheDocument());
      expect(screen.queryByText(shown, { exact: false })).not.toBeInTheDocument();
    }
  });
});
