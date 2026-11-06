import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ApiError } from "~/api/errors";
import { queryClient } from "~/query";
import { signedIn, signedOut } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { methods } from "~/test/fixtures/public";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { sessionQuery } from "./session-queries";
import { notificationSettings } from "~/test/fixtures/notifications";

const PROFILE = "/settings/account/profile";
const NOTIFICATIONS = "/settings/account/notifications";
const RECENT = JSON.stringify([{ kind: "key", id: "key_web", label: "checkout-web", href: "/" }]);

async function openProfile() {
  signedIn();
  on("GET", "me", me());
  on("GET", "auth/methods", methods());
  const rendered = await renderApp(PROFILE);
  await screen.findByRole("textbox", { name: "Name" });
  await screen.findByRole("navigation", { name: "Settings" });
  return rendered;
}

const nextOf = () => new URLSearchParams(window.location.search).get("next");

describe("a session that ends while the console is open", () => {
  it("sends the person to sign in, and back to the page, when a page can't load", async () => {
    const { user } = await openProfile();
    window.localStorage.setItem("kaeso:recent", RECENT);
    on("GET", "me/notifications", () => {
      signedOut();
      return apiError(401, "authentication", "Sign in to continue.");
    });

    await user.click(within(screen.getByRole("navigation", { name: "Settings" })).getByRole("link", { name: "Notifications" }));
    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(nextOf()).toBe(NOTIFICATIONS);
    expect(window.localStorage.getItem("kaeso:recent")).toBeNull();
    expect(await screen.findByRole("textbox", { name: "Email" })).toBeInTheDocument();
  });

  it("sends the person to sign in when the session check finds it ended", async () => {
    await openProfile();
    signedOut();
    queryClient
      .getQueryCache()
      .find({ queryKey: sessionQuery.queryKey })
      ?.setState({ dataUpdatedAt: Date.now() - 60_000 });
    window.dispatchEvent(new Event("visibilitychange"));

    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(nextOf()).toBe(PROFILE);
  });

  it("checks again before opening a page while the last session check said signed out", async () => {
    const { user } = await openProfile();
    on("GET", "me/notifications", notificationSettings());
    const ended = { status: "error" as const, error: new ApiError({ type: "authentication", status: 401, message: "Sign in to continue." }) };
    queryClient.getQueryCache().find({ queryKey: sessionQuery.queryKey })?.setState(ended);
    const checks = requests("GET", "auth/session").length;

    await user.click(within(screen.getByRole("navigation", { name: "Settings" })).getByRole("link", { name: "Notifications" }));
    expect(await screen.findByRole("combobox", { name: "Budget alerts" })).toHaveTextContent("Now");
    expect(requests("GET", "auth/session").length).toBeGreaterThan(checks);
    expect(window.location.pathname).toBe(NOTIFICATIONS);

    signedOut();
    queryClient.getQueryCache().find({ queryKey: sessionQuery.queryKey })?.setState(ended);
    await user.click(within(screen.getByRole("navigation", { name: "Settings" })).getByRole("link", { name: "Profile" }));
    await waitFor(() => expect(window.location.pathname).toBe("/sign-in"));
    expect(nextOf()).toBe(PROFILE);
  });
});
