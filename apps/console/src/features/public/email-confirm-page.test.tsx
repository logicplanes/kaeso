import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("confirming a new email address", () => {
  it("asks for the confirmation link when the token is missing", async () => {
    await renderApp("/me/email/confirm");
    expect(await screen.findByText("This page needs the confirmation link")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Go to your profile" })).toHaveAttribute("href", "/settings/account/profile");
  });

  it("confirms the address and opens the profile", async () => {
    signedIn();
    on("GET", "me", me({ email: "ada@example.org" }));
    on("POST", "me/email/confirm", me({ email: "ada@example.org" }));
    await renderApp("/me/email/confirm#token=eml_1");
    expect(window.location.hash).toBe("");
    expect(await (await notifications()).findByText("Email address changed; your other sessions are signed out")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toBe("/settings/account/profile"));
    expect(requests("POST", "me/email/confirm").map((r) => r.body)).toEqual([{ token: "eml_1" }]);
  });

  it("shows the waiting state while confirming", async () => {
    on("POST", "me/email/confirm", () => new Promise(() => undefined));
    await renderApp("/me/email/confirm#token=eml_1");
    expect(await screen.findByText("Confirming your email address…")).toBeInTheDocument();
  });

  it("offers to change the address again when the link is used up", async () => {
    on("POST", "me/email/confirm", apiError(401, "authentication", "This confirmation link has expired."));
    await renderApp("/me/email/confirm#token=eml_1");
    expect(await screen.findByText("This link has expired or was already used.")).toBeInTheDocument();
    expect(screen.getByText("A confirmation link works once; change the address again for a new one.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Change the address again" })).toHaveAttribute("href", "/settings/account/profile");
    expect((await (await notifications()).findAllByText("This confirmation link has expired.")).length).toBeGreaterThan(0);
  });
});
