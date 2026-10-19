import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, PROJECT, signedIn, signedOut, whoAmI } from "~/test/fixtures";
import { signedInReply, totpRequired } from "~/test/fixtures/public";
import { apiError, json, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

const LOOKUP = "auth/magic-link/lookup";
const VERIFY = "auth/magic-link/verify";

function linkFor(email = "ada@example.com") {
  on("POST", LOOKUP, { email, expires_at: new Date(Date.now() + 15 * 60_000).toISOString() });
}

describe("signing in with an emailed link", () => {
  it("asks for the link when the token is missing", async () => {
    await renderApp("/auth/magic-link");
    expect(await screen.findByText("This page needs the sign-in link")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to sign in" })).toHaveAttribute("href", "/sign-in");
    expect(requests("POST", LOOKUP)).toHaveLength(0);
    expect(requests("POST", VERIFY)).toHaveLength(0);
  });

  it("names the account and signs in only when asked, then opens the page the person came for", async () => {
    signedOut();
    linkFor();
    on("POST", VERIFY, () => {
      signedIn();
      return signedInReply();
    });
    const next = `/orgs/${ORG}/projects/${PROJECT}/keys`;
    const { user } = await renderApp(`/auth/magic-link?next=${encodeURIComponent(next)}#token=mgl_1`);
    expect(await screen.findByRole("heading", { name: "Sign in as ada@example.com", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("Continue only if you asked for this link yourself.")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(requests("POST", LOOKUP).map((r) => r.body)).toEqual([{ token: "mgl_1" }]);
    expect(requests("POST", VERIFY)).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(window.location.pathname).toBe(next));
    expect(requests("POST", VERIFY).map((r) => r.body)).toEqual([{ token: "mgl_1" }]);
  });

  it("says which session a different account replaces, and can keep it", async () => {
    signedIn({ session: whoAmI() });
    linkFor();
    await renderApp("/auth/magic-link#token=mgl_1");
    expect(
      await screen.findByText("You're signed in as owner@example.com in this browser; signing in as ada@example.com replaces that here."),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Stay signed in as owner@example.com" })).toHaveAttribute("href", "/");
    expect(requests("POST", VERIFY)).toHaveLength(0);
  });

  it("asks for the authenticator code when the account has two-factor authentication", async () => {
    signedOut();
    linkFor();
    on("POST", VERIFY, json(totpRequired(), { status: 202 }));
    on("POST", "auth/totp", () => {
      signedIn();
      return signedInReply();
    });
    const { user } = await renderApp("/auth/magic-link#token=mgl_1");
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    expect(await screen.findByText("Two-step verification")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Code from your authenticator app"), "123456");
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect(requests("POST", "auth/totp").map((r) => r.body)).toEqual([{ challenge: "chl_42", code: "123456" }]);
  });

  it("goes home when the next address leaves the console", async () => {
    signedOut();
    linkFor();
    on("POST", VERIFY, () => {
      signedIn();
      return signedInReply();
    });
    const { user } = await renderApp("/auth/magic-link?next=//elsewhere.test#token=mgl_1");
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("offers a new link when this one is used up", async () => {
    signedOut();
    on("POST", LOOKUP, apiError(401, "authentication", "This sign-in link has expired."));
    await renderApp("/auth/magic-link#token=mgl_1");
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect(screen.getByText("A sign-in link works once, for 15 minutes.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Request a new link" })).toHaveAttribute("href", "/sign-in");
    expect((await (await notifications()).findAllByText("This sign-in link has expired.")).length).toBeGreaterThan(0);
    expect(requests("POST", VERIFY)).toHaveLength(0);
  });

  it("offers a new link when the link is used up before the person signs in", async () => {
    signedOut();
    linkFor();
    on("POST", VERIFY, apiError(401, "authentication", "This sign-in link has expired."));
    const { user } = await renderApp("/auth/magic-link#token=mgl_1");
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
  });

  it("says when the gateway can't be reached", async () => {
    signedOut();
    on("POST", LOOKUP, () => {
      throw new TypeError("Failed to fetch");
    });
    await renderApp("/auth/magic-link#token=mgl_1");
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(screen.getByText("This link has expired or was already used.")).toBeInTheDocument();
  });
});
