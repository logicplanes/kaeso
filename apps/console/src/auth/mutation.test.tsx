import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn, signedOut } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/notifications";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("a change that fails", () => {
  it("sends someone whose session ended to sign in and back", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", () => {
      signedOut();
      on("GET", "auth/methods", methods());
      return apiError(401, "authentication", "Sign in to continue.");
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(new URLSearchParams(window.location.search).get("next")).toBe(PATH);
    expect(requests("PUT", "me/notifications")).toHaveLength(1);
  });

  it("says the console hit an unexpected error when the answer can't be read", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    expect((await (await notifications()).findAllByText("The change wasn't made; reload the page and try again.")).length).toBeGreaterThan(0);
  });
});
