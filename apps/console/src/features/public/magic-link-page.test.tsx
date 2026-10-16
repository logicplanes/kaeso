import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { signedInReply } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("signing in with an emailed link", () => {
  it("asks for the link when the token is missing", async () => {
    await renderApp("/auth/magic-link");
    expect(await screen.findByText("This page needs the sign-in link")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to sign in" })).toHaveAttribute("href", "/sign-in");
    expect(requests("POST", "auth/magic-link/verify")).toHaveLength(0);
  });

  it("verifies the token once and opens the page the person came for", async () => {
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    on("POST", "auth/magic-link/verify", async () => {
      await held;
      signedIn();
      return signedInReply();
    });
    const next = `/orgs/${ORG}/projects/${PROJECT}/keys`;
    await renderApp(`/auth/magic-link?next=${encodeURIComponent(next)}#token=mgl_1`);
    expect(await screen.findByText("Signing you in…")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    release();
    await waitFor(() => expect(window.location.pathname).toBe(next));
    expect(requests("POST", "auth/magic-link/verify").map((r) => r.body)).toEqual([{ token: "mgl_1" }]);
  });

  it("goes home when the next address leaves the console", async () => {
    on("POST", "auth/magic-link/verify", () => {
      signedIn();
      return signedInReply();
    });
    await renderApp("/auth/magic-link?next=//elsewhere.test#token=mgl_1");
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
  });

  it("offers a new link when this one is used up", async () => {
    on("POST", "auth/magic-link/verify", apiError(401, "authentication", "This sign-in link has expired."));
    await renderApp("/auth/magic-link#token=mgl_1");
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect(screen.getByText("A sign-in link works once, for 15 minutes.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Request a new link" })).toHaveAttribute("href", "/sign-in");
    expect((await (await notifications()).findAllByText("This sign-in link has expired.")).length).toBeGreaterThan(0);
  });

  it("says when the gateway can't be reached", async () => {
    on("POST", "auth/magic-link/verify", () => {
      throw new TypeError("Failed to fetch");
    });
    await renderApp("/auth/magic-link#token=mgl_1");
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(screen.getByText("This link has expired or was already used.")).toBeInTheDocument();
  });

  it("asks to try again when the gateway's answer can't be read", async () => {
    on("POST", "auth/magic-link/verify", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    await renderApp("/auth/magic-link#token=mgl_1");
    expect((await (await notifications()).findAllByText("Can't reach the gateway. Try again.")).length).toBeGreaterThan(0);
    expect(screen.getByText("This link has expired or was already used.")).toBeInTheDocument();
  });
});
