import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, signedIn, signedOut, user as account, whoAmI } from "~/test/fixtures";
import { accepted, invite, methods } from "~/test/fixtures/public";
import { apiError, noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "~/query";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const PENDING = "kaeso:invite";

function lookup(info = invite()) {
  on("POST", "auth/invites/lookup", info);
}

function acceptSigningIn(org: string | null = ORG) {
  on("POST", "auth/invites/accept", () => {
    signedIn();
    return accepted(org);
  });
}

describe("opening an invitation", () => {
  it("asks for the invitation link when there's none", async () => {
    signedOut();
    await renderApp("/invite");
    expect(await screen.findByText("This page needs the invitation link")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to sign in" })).toHaveAttribute("href", "/sign-in");
    expect(requests("POST", "auth/invites/lookup")).toHaveLength(0);
  });

  it("looks the invitation up and keeps the token only in memory", async () => {
    signedOut();
    lookup();
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Join Northwind Labs")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(window.sessionStorage.getItem(PENDING)).toBeNull();
    expect(requests("POST", "auth/invites/lookup").map((r) => r.body)).toEqual([{ token: "inv_tok" }]);
  });

  it("uses the token kept from earlier in the visit", async () => {
    signedOut();
    lookup();
    window.sessionStorage.setItem(PENDING, "inv_kept");
    await renderApp("/invite");
    expect(await screen.findByText("Join Northwind Labs")).toBeInTheDocument();
    expect(requests("POST", "auth/invites/lookup").map((r) => r.body)).toEqual([{ token: "inv_kept" }]);
  });

  it("shows a spinner while looking the invitation up", async () => {
    signedOut();
    on("POST", "auth/invites/lookup", () => new Promise(() => undefined));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Opening the invitation…")).toBeInTheDocument();
  });

  it("says when the invitation can't be used and forgets it", async () => {
    signedOut();
    on("POST", "auth/invites/lookup", apiError(404, "not_found", "No such invitation."));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("This invitation can't be used")).toBeInTheDocument();
    expect(screen.getByText("It expired, was revoked or was used; ask whoever invited you for a new one.")).toBeInTheDocument();
    expect(window.sessionStorage.getItem(PENDING)).toBeNull();
  });

  it("keeps the invitation when the gateway can't be reached, and tries again", async () => {
    signedOut();
    on("POST", "auth/invites/lookup", () => {
      throw new TypeError("Failed to fetch");
    });
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("The invitation couldn't be opened")).toBeInTheDocument();
    expect(screen.queryByText("This invitation can't be used")).not.toBeInTheDocument();
    expect(window.sessionStorage.getItem(PENDING)).toBeNull();
    lookup();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Join Northwind Labs")).toBeInTheDocument();
    expect(requests("POST", "auth/invites/lookup").map((r) => r.body)).toEqual([{ token: "inv_tok" }, { token: "inv_tok" }]);
  });

  it("shows the gateway's message when the invitation can't be looked up for another reason", async () => {
    signedOut();
    on("POST", "auth/invites/lookup", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("The invitation couldn't be opened")).toBeInTheDocument();
    expect(screen.getByText("Too many requests; try again in a minute.", { selector: "p" })).toBeInTheDocument();
    expect(window.sessionStorage.getItem(PENDING)).toBeNull();
  });
});

describe("accepting while signed in", () => {
  it("accepts with the invited account and opens the organization", async () => {
    signedIn({ session: whoAmI({ user: account({ email: "Grace@Example.com", name: "Grace Hopper" }) }) });
    lookup();
    acceptSigningIn();
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Accept to join Northwind Labs as Developer.")).toBeInTheDocument();
    expect(screen.getByText("The invitation for grace@example.com expires in 3 days.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Accept invitation" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/invites/accept").map((r) => r.body)).toEqual([{ token: "inv_tok" }]);
    expect(window.sessionStorage.getItem(PENDING)).toBeNull();
    await waitFor(() => expect(queryClient.getQueryCache().findAll({ queryKey: ["invite-lookup"] })).toEqual([]));
  });

  it("goes home after accepting an invitation to the install", async () => {
    signedIn({ session: whoAmI({ user: account({ email: "grace@example.com" }) }) });
    lookup(invite({ org: null, role: null }));
    acceptSigningIn(null);
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Join Kaeso")).toBeInTheDocument();
    expect(screen.getByText("Accept to join this Kaeso install.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Accept invitation" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("names only the organization when the invitation carries no role", async () => {
    signedIn({ session: whoAmI({ user: account({ email: "grace@example.com" }) }) });
    lookup(invite({ role: null }));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Accept to join Northwind Labs.")).toBeInTheDocument();
  });

  it("asks someone signed in as another person to sign out first", async () => {
    signedIn();
    lookup();
    on("POST", "auth/logout", () => {
      signedOut();
      return noContent();
    });
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("This invitation is for grace@example.com; you're signed in as owner@example.com.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Accept invitation" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sign out to accept" }));
    expect(await screen.findByText("Create your account to join Northwind Labs as Developer.")).toBeInTheDocument();
    expect(requests("POST", "auth/logout")).toHaveLength(1);
  });

  it("falls back to the name of someone signed in without an email", async () => {
    signedIn({ session: whoAmI({ user: account({ email: null }) }) });
    lookup();
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("This invitation is for grace@example.com; you're signed in as Ada Okafor.")).toBeInTheDocument();
  });

  it("says why signing out failed", async () => {
    signedIn();
    lookup();
    on("POST", "auth/logout", apiError(500, "internal", "Signing out failed; try again."));
    const { user } = await renderApp("/invite#token=inv_tok");
    await user.click(await screen.findByRole("button", { name: "Sign out to accept" }));
    expect((await (await notifications()).findAllByText("Signing out failed; try again.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Sign out to accept" })).toBeInTheDocument();
  });

  it("says when the gateway can't be reached to sign out", async () => {
    signedIn();
    lookup();
    on("POST", "auth/logout", () => {
      throw new TypeError("Failed to fetch");
    });
    const { user } = await renderApp("/invite#token=inv_tok");
    await user.click(await screen.findByRole("button", { name: "Sign out to accept" }));
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
  });

  it("asks to try again when the answer to signing out can't be read", async () => {
    signedIn();
    lookup();
    on("POST", "auth/logout", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp("/invite#token=inv_tok");
    await user.click(await screen.findByRole("button", { name: "Sign out to accept" }));
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Sign out to accept" })).toBeInTheDocument();
  });
});

describe("accepting while signed out", () => {
  it("sends someone with an account to sign in and back", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    lookup(invite({ account_exists: true }));
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("An account with grace@example.com exists. Sign in to accept and join Northwind Labs as Developer.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sign in to accept" }));
    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(new URLSearchParams(window.location.search).get("next")).toBe("/invite");
    expect(window.sessionStorage.getItem(PENDING)).toBe("inv_tok");
  });

  it("words the account sentence for an organization without a role", async () => {
    signedOut();
    lookup(invite({ account_exists: true, role: null }));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("An account with grace@example.com exists. Sign in to accept and join Northwind Labs.")).toBeInTheDocument();
  });

  it("words the account sentence for the install", async () => {
    signedOut();
    lookup(invite({ account_exists: true, org: null, role: null }));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("An account with grace@example.com exists. Sign in to accept and join this Kaeso install.")).toBeInTheDocument();
  });

  it("creates an account with a password and joins", async () => {
    signedOut();
    lookup();
    acceptSigningIn();
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByRole("textbox", { name: "Email" })).toHaveValue("grace@example.com");
    const create = screen.getByRole("button", { name: "Create account and join" });
    expect(create).toBeDisabled();
    expect(screen.getByRole("button", { name: "Continue with a sign-in link instead" })).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Name" }), " Grace Hopper {Enter}");
    expect(requests("POST", "auth/invites/accept")).toHaveLength(0);
    await user.type(screen.getByLabelText("Password"), "correct horse battery");
    await user.click(screen.getByRole("button", { name: "Create account and join" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/invites/accept").map((r) => r.body)).toEqual([{ token: "inv_tok", name: "Grace Hopper", password: "correct horse battery" }]);
  });

  it("joins without a password and keeps saying so while the organization opens", async () => {
    signedOut();
    lookup(invite({ org: null, role: null }));
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    on("POST", "auth/invites/accept", () => {
      const session = signedIn();
      on("GET", "auth/session", async () => {
        await held;
        return session;
      });
      on("POST", "auth/invites/lookup", apiError(404, "not_found", "This invitation was used."));
      return accepted(null);
    });
    const { user } = await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Create your account to join this Kaeso install.")).toBeInTheDocument();
    expect(
      screen.getByText(/Without a password, you sign in with a link by email\. The invitation for grace@example.com expires in 3 days\./),
    ).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Name" }), "Grace Hopper");
    await user.click(screen.getByRole("button", { name: "Continue with a sign-in link instead" }));
    expect(await screen.findByText("Invitation accepted")).toBeInTheDocument();
    await waitFor(() => expect(requests("POST", "auth/invites/lookup")).toHaveLength(2));
    expect(screen.getByText("Invitation accepted")).toBeInTheDocument();
    expect(screen.queryByText("This invitation can't be used")).not.toBeInTheDocument();
    release();
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/invites/accept").map((r) => r.body)).toEqual([{ token: "inv_tok", name: "Grace Hopper" }]);
  });

  it("words the create sentence for an organization without a role", async () => {
    signedOut();
    lookup(invite({ role: null }));
    await renderApp("/invite#token=inv_tok");
    expect(await screen.findByText("Create your account to join Northwind Labs.")).toBeInTheDocument();
  });

  it("shows why joining was refused", async () => {
    signedOut();
    lookup();
    on("POST", "auth/invites/accept", apiError(409, "conflict", "This invitation was revoked."));
    const { user } = await renderApp("/invite#token=inv_tok");
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "Grace Hopper");
    await user.type(screen.getByLabelText("Password"), "correct horse battery{Enter}");
    expect((await (await notifications()).findAllByText("This invitation was revoked.")).length).toBeGreaterThan(0);
    expect(window.location.pathname).toBe("/invite");
  });

  it("marks the field the gateway refused", async () => {
    signedOut();
    lookup();
    on("POST", "auth/invites/accept", apiError(422, "invalid_request", "That password appears in known breaches.", { param: "password" }));
    const { user } = await renderApp("/invite#token=inv_tok");
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "Grace Hopper");
    await user.type(screen.getByLabelText("Password"), "correct horse battery{Enter}");
    expect((await (await notifications()).findAllByText("That password appears in known breaches.")).length).toBeGreaterThan(0);
    expect(screen.getByLabelText("Password")).toHaveAccessibleDescription(/That password appears in known breaches\./);
    expect(screen.getByRole("textbox", { name: "Name" })).not.toHaveAttribute("aria-invalid");
  });

  it("shows which way of joining is on its way", async () => {
    signedOut();
    lookup();
    on("POST", "auth/invites/accept", () => new Promise(() => undefined));
    const { user } = await renderApp("/invite#token=inv_tok");
    await user.type(await screen.findByRole("textbox", { name: "Name" }), "Grace Hopper");
    await user.click(screen.getByRole("button", { name: "Continue with a sign-in link instead" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Continue with a sign-in link instead" })).toHaveAttribute("aria-busy", "true"));
    expect(screen.getByRole("button", { name: "Create account and join" })).not.toHaveAttribute("aria-busy");
    expect(screen.getByRole("button", { name: "Create account and join" })).toBeDisabled();
  });
});
