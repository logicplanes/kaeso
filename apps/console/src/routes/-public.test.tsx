import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, signedIn, signedOut } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("the sign-in address", () => {
  it("keeps a next page on this site through the notice", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    const { router } = await renderApp(`/sign-in?done=password-changed&next=/orgs/${ORG}/projects`);
    expect(await (await notifications()).findByText("Password changed; sign in with the new one")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.search).toEqual({ next: `/orgs/${ORG}/projects` }));
  });

  it("drops a next page on another site", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    const { router } = await renderApp("/sign-in?done=password-changed&next=//elsewhere.test/steal");
    expect(await (await notifications()).findByText("Password changed; sign in with the new one")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("confirms a changed password once and takes the notice out of the address", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    const { router } = await renderApp("/sign-in?done=password-changed");
    expect(await (await notifications()).findByText("Password changed; sign in with the new one")).toBeInTheDocument();
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("shows why single sign-on failed from the address fragment", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    await renderApp("/sign-in#sso_error=no_access");
    expect((await (await notifications()).findAllByText("You have no access to this organization yet; ask an admin to invite you.")).length).toBeGreaterThan(0);
    expect(window.location.hash).toBe("");
  });

  it("shows its own words, never text from the address", async () => {
    signedOut();
    on("GET", "auth/methods", methods());
    await renderApp("/sign-in#sso_error=Call%20555-0100%20to%20restore%20your%20account.");
    expect((await (await notifications()).findAllByText("Single sign-on didn't work; try again.")).length).toBeGreaterThan(0);
    expect(screen.queryByText(/555-0100/)).not.toBeInTheDocument();
  });
});

describe("the public pages", () => {
  it.each([
    ["/sign-up", "Create an account"],
    ["/setup", "This page needs the setup link"],
    ["/invite", "This page needs the invitation link"],
    ["/auth/magic-link", "This page needs the sign-in link"],
    ["/auth/password-reset", "Reset your password"],
    ["/auth/signup", "This page needs the sign-up link"],
    ["/me/email/confirm", "This page needs the confirmation link"],
  ])("open %s without a session", async (path, title) => {
    signedOut();
    on("GET", "auth/methods", methods({ sign_up: true }));
    await renderApp(path);
    expect(await screen.findByRole("heading", { name: title, level: 1 })).toBeInTheDocument();
  });
});

describe("the sign-in link", () => {
  const account = { email: "ada@example.com", expires_at: "2026-10-19T12:15:00Z" };

  it("signs in and opens the page the person came for", async () => {
    signedOut();
    on("POST", "auth/magic-link/lookup", account);
    on("POST", "auth/magic-link/verify", () => {
      signedIn();
      return noContent();
    });
    const { router, user } = await renderApp("/auth/magic-link?next=/settings/account/appearance#token=tok_1");
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/settings/account/appearance"));
    expect(requests("POST", "auth/magic-link/verify").map((r) => r.body)).toEqual([{ token: "tok_1" }]);
    expect(await screen.findByRole("heading", { name: "Appearance", level: 1 })).toBeInTheDocument();
  });

  it("ignores a next page on another site and goes to the start page", async () => {
    signedOut();
    on("POST", "auth/magic-link/lookup", account);
    on("POST", "auth/magic-link/verify", () => {
      signedIn();
      return noContent();
    });
    const { router, user } = await renderApp("/auth/magic-link?next=//elsewhere.test/steal#token=tok_2");
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`/orgs/${ORG}`));
    expect(window.location.host).toBe("localhost:3000");
  });
});
