import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn, signedOut } from "~/test/fixtures";
import { methods } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { notificationSettings, testEmailSent } from "~/test/fixtures/notifications";

const PATH = "/settings/account/notifications";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("a change that fails", () => {
  it("sends someone whose session ended to sign in and back", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    on("POST", "me/notifications/test", () => {
      signedOut();
      on("GET", "auth/methods", methods());
      return apiError(401, "authentication", "Sign in to continue.");
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Send a test email" }));
    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(new URLSearchParams(window.location.search).get("next")).toBe(PATH);
    expect(requests("POST", "me/notifications/test")).toHaveLength(1);
  });

  it("says the console hit an unexpected error when the answer can't be read", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    on("POST", "me/notifications/test", () => new Response("{", { status: 200, headers: { "content-type": "application/json" } }));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Send a test email" }));
    expect((await (await notifications()).findAllByText("The change wasn't made; reload the page and try again.")).length).toBeGreaterThan(0);
  });

  it("takes its error away once the same change goes through", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    let calls = 0;
    on("POST", "me/notifications/test", () =>
      ++calls === 1 ? apiError(503, "unavailable", "Kaeso can't reach its database right now; try again shortly.") : testEmailSent,
    );
    const { user } = await renderApp(PATH);
    const toggle = await screen.findByRole("button", { name: "Send a test email" });
    await user.click(toggle);
    const region = await notifications();
    expect((await region.findAllByText("Kaeso can't reach its database right now; try again shortly.")).length).toBeGreaterThan(0);
    await user.click(toggle);
    await waitFor(() => expect(calls).toBe(2));
    await waitFor(() => expect(region.queryByText("Kaeso can't reach its database right now; try again shortly.")).not.toBeInTheDocument());
  });

  it("shows one error, not one per attempt, when the same change keeps failing", async () => {
    signedIn();
    on("GET", "me/notifications", notificationSettings());
    on("POST", "me/notifications/test", apiError(503, "unavailable", "Kaeso can't reach its database right now; try again shortly."));
    const { user } = await renderApp(PATH);
    const toggle = await screen.findByRole("button", { name: "Send a test email" });
    await user.click(toggle);
    const region = await notifications();
    await region.findAllByText("Kaeso can't reach its database right now; try again shortly.");
    await user.click(toggle);
    await waitFor(() => expect(requests("POST", "me/notifications/test")).toHaveLength(2));
    await new Promise((r) => setTimeout(r, 300));
    const shown = region.getAllByText("Kaeso can't reach its database right now; try again shortly.").filter((el) => el.closest(".k-toast"));
    expect(shown).toHaveLength(1);
  });
});
