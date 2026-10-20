import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ORG, signedIn, signedOut } from "~/test/fixtures";
import { emailSent, methods, signedInReply, ssoStart, totpRequired } from "~/test/fixtures/public";
import { apiError, json, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

async function atMethods(path: string, offered = methods()) {
  signedOut();
  on("GET", "auth/methods", methods({ sign_up: offered.sign_up }));
  on("POST", "auth/methods", offered);
  const view = await renderApp(path);
  await view.user.type(await screen.findByRole("textbox", { name: "Email" }), "  ada@example.com ");
  await view.user.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByRole("link", { name: "Use another email" });
  return view;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the email step", () => {
  it("labels every field with visible text, not a placeholder", async () => {
    const { user } = await atMethods("/sign-in");
    const password = screen.getByLabelText("Password");
    expect(password).not.toHaveAttribute("placeholder");
    expect(screen.getByText("Password", { selector: "label" })).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "Use another email" }));
    const email = await screen.findByRole("textbox", { name: "Email" });
    expect(email).not.toHaveAttribute("placeholder");
    expect(screen.getByText("Email", { selector: "label" })).toBeInTheDocument();
  });

  it("asks the gateway which methods the address has", async () => {
    await atMethods("/sign-in");
    expect(requests("POST", "auth/methods").map((r) => r.body)).toEqual([{ email: "ada@example.com" }]);
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Email me a sign-in link" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Forgot your password?" })).toHaveAttribute("href", "/auth/password-reset");
    expect(screen.queryByRole("button", { name: "Continue with SSO" })).not.toBeInTheDocument();
  });

  it("offers to create an account when the install allows it", async () => {
    signedOut();
    on("GET", "auth/methods", methods({ sign_up: true }));
    await renderApp("/sign-in");
    expect(await screen.findByRole("link", { name: "Create an account" })).toHaveAttribute("href", "/sign-up");
  });

  it("keeps account creation out of sight on an install by invitation", async () => {
    signedOut();
    on("GET", "auth/methods", methods({ sign_up: false }));
    await renderApp("/sign-in");
    await screen.findByRole("textbox", { name: "Email" });
    await waitFor(() => expect(requests("GET", "auth/methods")).toHaveLength(1));
    expect(screen.queryByRole("link", { name: "Create an account" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
  });

  it("says when the gateway can't be reached", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    on("POST", "auth/methods", () => {
      throw new TypeError("Failed to fetch");
    });
    const { user } = await renderApp("/sign-in");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@example.com");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await (await notifications()).findByText("Can't reach the gateway. Try again.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveValue("ada@example.com");
  });

  it("still asks for the address when the install's methods can't load", async () => {
    signedOut();
    on("GET", "auth/methods", () => {
      throw new TypeError("Failed to fetch");
    });
    await renderApp("/sign-in");
    expect(await screen.findByRole("textbox", { name: "Email" })).toBeInTheDocument();
  });

  it("asks to try again when the gateway's answer can't be read", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    on("POST", "auth/methods", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp("/sign-in");
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@example.com{Enter}");
    expect(await (await notifications()).findByText("Can't reach the gateway. Try again.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveValue("ada@example.com");
  });

  it("goes back to the email field with the address it used", async () => {
    const { user } = await atMethods("/sign-in");
    expect(screen.getByText("ada@example.com")).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "Use another email" }));
    expect(await screen.findByRole("textbox", { name: "Email" })).toHaveValue("ada@example.com");
  });
});

describe("signing in with a password", () => {
  it("signs in and opens the page the person came for", async () => {
    const { user } = await atMethods(`/sign-in?next=/orgs/${ORG}`);
    on("POST", "auth/password", () => {
      signedIn();
      return signedInReply();
    });
    await user.type(screen.getByLabelText("Password"), "correct horse battery");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/password").map((r) => r.body)).toEqual([{ email: "ada@example.com", password: "correct horse battery" }]);
  });

  it("ignores a next address on another site", async () => {
    const { user } = await atMethods("/sign-in?next=//elsewhere.test/steal");
    on("POST", "auth/password", () => {
      signedIn();
      return signedInReply();
    });
    await user.type(screen.getByLabelText("Password"), "correct horse battery");
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(window.location.host).toBe("localhost:3000");
  });

  it("shows the gateway's refusal under the password and selects it to retype", async () => {
    const { user } = await atMethods("/sign-in");
    on("POST", "auth/password", apiError(401, "authentication", "That email and password don't match."));
    const field = screen.getByLabelText("Password");
    await user.type(field, "wrong one");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(field).toHaveAttribute("aria-invalid", "true"));
    expect(field).toHaveAccessibleDescription("That email and password don't match.");
    expect(screen.getByText("That email and password don't match.")).toBeVisible();
    expect(within(await screen.findByRole("region", { name: "Notifications" })).queryByText("That email and password don't match.")).not.toBeInTheDocument();
    await waitFor(() => expect(field).toHaveFocus());
    expect(window.location.pathname).toBe("/sign-in");
    await user.type(field, "x");
    expect(field).not.toHaveAttribute("aria-invalid");
    expect(screen.queryByText("That email and password don't match.")).not.toBeInTheDocument();
  });

  it("keeps a failure that isn't about the password in a notification", async () => {
    const { user } = await atMethods("/sign-in");
    on("POST", "auth/password", apiError(503, "unavailable", "The gateway is starting; try again in a moment."));
    const field = screen.getByLabelText("Password");
    await user.type(field, "correct horse battery{Enter}");
    expect(await (await notifications()).findByText("The gateway is starting; try again in a moment.")).toBeInTheDocument();
    expect(field).not.toHaveAttribute("aria-invalid");
  });

  it("carries the address into the password reset", async () => {
    const { user } = await atMethods("/sign-in");
    await user.click(screen.getByRole("link", { name: "Forgot your password?" }));
    expect(await screen.findByRole("heading", { name: "Reset your password", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Email" })).toHaveValue("ada@example.com");
  });

  it("underlines every link on the page alike", async () => {
    await atMethods("/sign-in", methods({ sign_up: true }));
    const links = screen.getAllByRole("link");
    expect(links.length).toBeGreaterThan(2);
    for (const link of links) expect(link).toHaveClass("underline");
  });
});

describe("a signed-in visit", () => {
  it("opens the page it was asked for instead of the form", async () => {
    signedIn();
    await renderApp("/sign-in?next=/settings/account/appearance");
    await waitFor(() => expect(window.location.pathname).toBe("/settings/account/appearance"));
    expect(screen.queryByRole("textbox", { name: "Email" })).not.toBeInTheDocument();
  });

  it("opens the console without a next page, and never another site", async () => {
    signedIn();
    await renderApp("/sign-in?next=//elsewhere.test/steal");
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(window.location.host).toBe("localhost:3000");
  });
});

describe("two-step verification", () => {
  async function atCode() {
    const view = await atMethods("/sign-in");
    on("POST", "auth/password", json(totpRequired(), { status: 202 }));
    await view.user.type(screen.getByLabelText("Password"), "correct horse battery");
    await view.user.click(screen.getByRole("button", { name: "Sign in" }));
    await screen.findByText("Two-step verification");
    return view;
  }

  it("verifies the six digits as soon as they're typed", async () => {
    const { user } = await atCode();
    expect(screen.getByText(/The challenge expires in 5 min\./)).toBeInTheDocument();
    on("POST", "auth/totp", () => {
      signedIn();
      return signedInReply();
    });
    await user.type(screen.getByLabelText("Code from your authenticator app"), "123456");
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([{ challenge: "chl_42", code: "123456" }]);
  });

  it("clears a wrong code for another try", async () => {
    const { user } = await atCode();
    on("POST", "auth/totp", apiError(401, "authentication", "That code didn't work."));
    const field = screen.getByLabelText("Code from your authenticator app");
    await user.type(field, "000000");
    expect(await (await notifications()).findByText("That code didn't work.")).toBeInTheDocument();
    await waitFor(() => expect(field).toHaveValue(""));
    expect(screen.getByRole("button", { name: "Verify" })).toBeDisabled();
  });

  it("takes a recovery code instead, and can switch back", async () => {
    const { user } = await atCode();
    await user.click(screen.getByRole("link", { name: "Use a recovery code" }));
    expect(screen.getByText("Enter one of your recovery codes; each works once.")).toBeInTheDocument();
    on("POST", "auth/totp", () => {
      signedIn();
      return signedInReply();
    });
    const verify = screen.getByRole("button", { name: "Verify" });
    expect(verify).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Recovery code" }), " abcd-efgh ");
    await user.click(verify);
    await waitFor(() => expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([{ challenge: "chl_42", recovery_code: "abcd-efgh" }]));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("goes back to the authenticator app", async () => {
    const { user } = await atCode();
    await user.click(screen.getByRole("link", { name: "Use a recovery code" }));
    await user.click(screen.getByRole("link", { name: "Use the authenticator app" }));
    expect(screen.getByLabelText("Code from your authenticator app")).toBeInTheDocument();
  });
});

describe("other ways in", () => {
  it("emails a sign-in link", async () => {
    const { user } = await atMethods("/sign-in");
    on("POST", "auth/magic-link", emailSent("Sent."));
    await user.click(screen.getByRole("button", { name: "Email me a sign-in link" }));
    expect(await screen.findByText("Check your email")).toBeInTheDocument();
    expect(screen.getByText("If an account exists for ada@example.com, a sign-in link is on its way.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Back to sign in" })).toHaveFocus();
    expect(requests("POST", "auth/magic-link").map((r) => r.body)).toEqual([{ email: "ada@example.com" }]);
    await user.click(screen.getByRole("button", { name: "Back to sign in" }));
    expect(await screen.findByRole("textbox", { name: "Email" })).toBeInTheDocument();
  });

  it("sends the person to their identity provider", async () => {
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await atMethods("/sign-in", methods({ sso: true, password: false, magic_link: false }));
    expect(screen.getByRole("button", { name: "Continue with SSO" })).toHaveFocus();
    on("POST", "auth/oidc/start", ssoStart("https://idp.example.com/authorize?state=s1"));
    await user.click(screen.getByRole("button", { name: "Continue with SSO" }));
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://idp.example.com/authorize?state=s1"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ email: "ada@example.com" }]);
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
  });

  it("puts the focus on the sign-in link when it's the only way in", async () => {
    await atMethods("/sign-in", methods({ sso: false, password: false, magic_link: true }));
    expect(screen.getByRole("button", { name: "Email me a sign-in link" })).toHaveFocus();
  });

  it("offers single sign-on next to the password", async () => {
    await atMethods("/sign-in", methods({ sso: true, password: true, magic_link: false }));
    expect(screen.getByRole("button", { name: "Continue with SSO" })).toBeInTheDocument();
    expect(screen.getByLabelText("Password")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeInTheDocument();
  });

  it("asks for an account once while the request is on its way", async () => {
    const { user } = await atMethods("/sign-in", methods({ password: false, magic_link: false, sign_up: true }));
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    on("POST", "auth/signup", async () => {
      await held;
      return emailSent("We sent a link to ada@example.com to finish your account.");
    });
    const create = screen.getByRole("link", { name: "Create an account" });
    await user.click(create);
    await waitFor(() => expect(create).toHaveAttribute("aria-disabled", "true"));
    await user.click(create);
    release();
    expect(await screen.findByText("We sent a link to ada@example.com to finish your account.")).toBeInTheDocument();
    expect(requests("POST", "auth/signup")).toHaveLength(1);
  });

  it("creates an account for a new address", async () => {
    const { user } = await atMethods("/sign-in", methods({ password: false, magic_link: false, sign_up: true }));
    on("POST", "auth/signup", emailSent("We sent a link to ada@example.com to finish your account."));
    await user.click(screen.getByRole("link", { name: "Create an account" }));
    expect(await screen.findByText("We sent a link to ada@example.com to finish your account.")).toBeInTheDocument();
    expect(requests("POST", "auth/signup").map((r) => r.body)).toEqual([{ email: "ada@example.com" }]);
  });

  it("says when there's no way in for the address", async () => {
    await atMethods("/sign-in", methods({ password: false, magic_link: false, sso: false, sign_up: false }));
    expect(screen.getByText("There's no way to sign in with ada@example.com on this install. Ask an administrator.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Sign in" })).not.toBeInTheDocument();
  });
});

describe("arriving at sign-in", () => {
  it("shows why single sign-on failed and drops it from the address", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    await renderApp("/sign-in#sso_error=provider_refused");
    expect(await (await notifications()).findByText("The identity provider refused the sign-in.")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
  });

  it("confirms a changed password and keeps where the person was going", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    await renderApp(`/sign-in?done=password-changed&next=/orgs/${ORG}`);
    expect(await (await notifications()).findByText("Password changed; sign in with the new one")).toBeInTheDocument();
    await waitFor(() => expect(window.location.search).toBe(`?next=${encodeURIComponent(`/orgs/${ORG}`)}`));
  });

  it("drops an unknown notice", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    await renderApp("/sign-in?done=hacked");
    await screen.findByRole("textbox", { name: "Email" });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(within(await screen.findByRole("region", { name: "Notifications" })).queryByText(/./)).not.toBeInTheDocument();
  });
});

describe("single sign-on without an address", () => {
  const NORTHWIND_ID = { org: ORG, name: "Northwind ID", auto_redirect: false };
  const ssoOnly = (provider: typeof NORTHWIND_ID | null = NORTHWIND_ID) => methods({ password: false, magic_link: false, sso: true, provider });

  it("offers the install's only provider by name and sends the person on with where they were going", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly());
    on("POST", "auth/oidc/start", ssoStart("https://id.northwind.test/authorize?state=s1"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    const { user } = await renderApp(`/sign-in?next=/orgs/${ORG}/projects`);
    const button = await screen.findByRole("button", { name: "Continue with Northwind ID" });
    expect(button).toHaveFocus();
    expect(screen.queryByRole("textbox", { name: "Email" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Sign in with your email instead" })).not.toBeInTheDocument();
    expect(requests("POST", "auth/oidc/start")).toHaveLength(0);
    await user.click(button);
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://id.northwind.test/authorize?state=s1"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ org: ORG, next: `/orgs/${ORG}/projects` }]);
  });

  it("says SSO on the button of a provider without a name", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly({ ...NORTHWIND_ID, name: null as unknown as string }));
    await renderApp("/sign-in");
    expect(await screen.findByRole("button", { name: "Continue with SSO" })).toBeInTheDocument();
  });

  it("starts at the provider on its own when the provider asks for it", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly({ ...NORTHWIND_ID, auto_redirect: true }));
    on("POST", "auth/oidc/start", ssoStart("https://id.northwind.test/authorize?state=s2"));
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    await renderApp("/sign-in");
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://id.northwind.test/authorize?state=s2"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ org: ORG }]);
  });

  it("waits for a click after signing out", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly({ ...NORTHWIND_ID, auto_redirect: true }));
    await renderApp("/sign-in?done=signed-out");
    expect(await (await notifications()).findByText("You're signed out")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue with Northwind ID" })).toBeInTheDocument();
    await waitFor(() => expect(window.location.search).toBe(""));
    expect(requests("POST", "auth/oidc/start")).toHaveLength(0);
  });

  it("keeps the organization of a sign-in link when it drops the notice", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly({ ...NORTHWIND_ID, auto_redirect: true }));
    await renderApp(`/sign-in?done=signed-out&org=${ORG}`);
    expect(await (await notifications()).findByText("You're signed out")).toBeInTheDocument();
    await waitFor(() => expect(window.location.search).toBe(`?org=${ORG}`));
    expect(requests("POST", "auth/oidc/start")).toHaveLength(0);
  });

  it("says why the provider couldn't start, and lets the person try again", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly());
    on("POST", "auth/oidc/start", apiError(503, "unavailable", "The identity provider could not be reached."));
    const { user } = await renderApp("/sign-in");
    await user.click(await screen.findByRole("button", { name: "Continue with Northwind ID" }));
    expect(await (await notifications()).findByText("The identity provider could not be reached.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue with Northwind ID" })).toBeEnabled();
  });

  it("waits for a click after the provider refused, and says why in the person's language", async () => {
    signedOut();
    on("GET", "auth/methods", ssoOnly({ ...NORTHWIND_ID, auto_redirect: true }));
    await renderApp("/sign-in#sso_error=provider_refused", { locale: "de" });
    expect(
      await within(await screen.findByRole("region", { name: "Benachrichtigungen" })).findByText("Der Identitätsanbieter hat die Anmeldung abgelehnt."),
    ).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Weiter mit Northwind ID" })).toBeInTheDocument();
    expect(requests("POST", "auth/oidc/start")).toHaveLength(0);
  });

  it("offers an organization's provider from its sign-in link, and the address too", async () => {
    signedOut();
    on("GET", "auth/methods", (r: { query: URLSearchParams }) => (r.query.get("org") === ORG ? methods({ sso: true, provider: NORTHWIND_ID }) : methods()));
    const { user } = await renderApp(`/sign-in?org=${ORG}`);
    expect(await screen.findByRole("button", { name: "Continue with Northwind ID" })).toBeInTheDocument();
    expect(requests("GET", "auth/methods").map((r) => r.query.get("org"))).toContain(ORG);
    await user.click(screen.getByRole("link", { name: "Sign in with your email instead" }));
    expect(await screen.findByRole("textbox", { name: "Email" })).toBeInTheDocument();
  });

  it("ignores an organization that isn't one", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    await renderApp("/sign-in?org=../../steal");
    await screen.findByRole("textbox", { name: "Email" });
    expect(requests("GET", "auth/methods").map((r) => r.query.get("org"))).toEqual([null]);
  });

  it("names the provider of the address, and goes straight there when it's the only way in", async () => {
    const assign = vi.spyOn(window.location, "assign").mockImplementation(() => undefined);
    await atMethods("/sign-in", methods({ sso: true, provider: NORTHWIND_ID }));
    expect(screen.getByRole("button", { name: "Continue with Northwind ID" })).toBeInTheDocument();

    signedOut();
    on("GET", "auth/methods", methods());
    on("POST", "auth/methods", ssoOnly({ ...NORTHWIND_ID, auto_redirect: true }));
    on("POST", "auth/oidc/start", ssoStart("https://id.northwind.test/authorize?state=s3"));
    const { user } = await renderApp(`/sign-in?next=/orgs/${ORG}`);
    await user.type(await screen.findByRole("textbox", { name: "Email" }), "ada@northwind.test{Enter}");
    await waitFor(() => expect(assign).toHaveBeenCalledWith("https://id.northwind.test/authorize?state=s3"));
    expect(requests("POST", "auth/oidc/start").map((r) => r.body)).toEqual([{ email: "ada@northwind.test", next: `/orgs/${ORG}` }]);
  });
});
