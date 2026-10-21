import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { sessionItem } from "~/test/fixtures/instance-account";
import { apiError, noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/sessions";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const FIREFOX = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0";

function sessions(
  list = [sessionItem(), sessionItem({ id: "ses_laptop", current: false, user_agent: FIREFOX, ip: null, method: "magic_link", second_factor: true })],
) {
  on("GET", "me/sessions", { data: list });
}

describe("sessions", () => {
  it("lists this device first with its sign-in method", async () => {
    signedIn();
    sessions([
      sessionItem({ id: "ses_old", current: false, user_agent: FIREFOX, last_used_at: "2026-10-01T12:00:00Z" }),
      sessionItem({ id: "ses_laptop", current: false, user_agent: null, ip: null, second_factor: true, last_used_at: "2026-10-14T12:00:00Z" }),
      sessionItem(),
    ]);
    await renderApp(PATH);
    expect(await screen.findByText("This device")).toBeInTheDocument();
    const titles = screen.getAllByText(/^(Safari on macOS|Firefox on Windows|Unknown device)$/).map((n) => n.firstChild?.textContent);
    expect(titles).toEqual(["Safari on macOS", "Unknown device", "Firefox on Windows"]);
    expect(screen.getAllByText(/^203\.0\.113\.7 · signed in .* · used /)).toHaveLength(2);
    expect(screen.getByText(/^Unknown address · signed in /)).toBeInTheDocument();
    expect(screen.getByText("Two-factor")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "End session" })).toHaveLength(2);
  });

  it("orders the other sessions by when they were last used, whatever the precision of the times, and names API clients", async () => {
    signedIn();
    sessions([
      sessionItem({ id: "ses_a", current: false, user_agent: "node", last_used_at: "2026-10-20T04:55:42.6Z", signed_in_at: "2026-10-01T08:00:00Z" }),
      sessionItem({ id: "ses_b", current: false, user_agent: FIREFOX, last_used_at: "2026-10-20T04:55:42.629199Z", signed_in_at: "2026-10-02T08:00:00Z" }),
      sessionItem({ id: "ses_c", current: false, user_agent: "curl/8.7.1", last_used_at: "2026-10-20T04:55:43Z", signed_in_at: "2026-09-30T08:00:00Z" }),
      sessionItem(),
    ]);
    await renderApp(PATH);
    await screen.findByText("This device");
    const titles = screen
      .getAllByText(/^(Safari on macOS|Firefox on Windows|Node\.js \(API client\)|curl \(API client\))$/)
      .map((n) => n.firstChild?.textContent);
    expect(titles).toEqual(["Safari on macOS", "curl (API client)", "Firefox on Windows", "Node.js (API client)"]);
  });

  it("puts the later sign-in first when two sessions were last used at the same moment", async () => {
    signedIn();
    sessions([
      sessionItem({ id: "ses_early", current: false, user_agent: "curl/8.7.1", last_used_at: "2026-10-20T04:55:43Z", signed_in_at: "2026-09-01T08:00:00Z" }),
      sessionItem({ id: "ses_late", current: false, user_agent: FIREFOX, last_used_at: "2026-10-20T04:55:43Z", signed_in_at: "2026-10-02T08:00:00Z" }),
      sessionItem(),
    ]);
    await renderApp(PATH);
    await screen.findByText("This device");
    const titles = screen.getAllByText(/^(Safari on macOS|Firefox on Windows|curl \(API client\))$/).map((n) => n.firstChild?.textContent);
    expect(titles).toEqual(["Safari on macOS", "Firefox on Windows", "curl (API client)"]);
  });

  it("says when the sessions couldn't load", async () => {
    signedIn();
    on("GET", "me/sessions", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("Your sessions couldn't load")).toBeInTheDocument();
  });

  it("can't sign out everywhere else with only this session", async () => {
    signedIn();
    sessions([sessionItem()]);
    await renderApp(PATH);
    await screen.findByText("This device");
    expect(screen.getByRole("button", { name: "Sign out everywhere" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "End session" })).not.toBeInTheDocument();
  });

  it("ends another session after confirming", async () => {
    signedIn();
    sessions();
    on("DELETE", "me/sessions/:session", noContent);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "End session" }));
    const confirm = await screen.findByRole("alertdialog", { name: "End this session?" });
    expect(within(confirm).getByText("Firefox on Windows is signed out at once.")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "End session" }));
    expect(await (await notifications()).findByText("Session ended")).toBeInTheDocument();
    expect(requests("DELETE", "me/sessions/:session").map((r) => r.params.session)).toEqual(["ses_laptop"]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("keeps the session when ending it is refused", async () => {
    signedIn();
    sessions();
    on("DELETE", "me/sessions/:session", apiError(404, "not_found", "That session already ended."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "End session" }));
    const confirm = await screen.findByRole("alertdialog", { name: "End this session?" });
    await user.click(within(confirm).getByRole("button", { name: "End session" }));
    expect((await (await notifications()).findAllByText("That session already ended.")).length).toBeGreaterThan(0);
  });

  it("closes the confirmation without ending the session", async () => {
    signedIn();
    sessions();
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "End session" }));
    const confirm = await screen.findByRole("alertdialog", { name: "End this session?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(requests("DELETE", "me/sessions/:session")).toHaveLength(0);
  });

  it("signs out everywhere else", async () => {
    signedIn();
    sessions();
    on("POST", "me/sessions/revoke-all", { revoked: 1 });
    const { user } = await renderApp(PATH);
    await screen.findByText("This device");
    await user.click(screen.getByRole("button", { name: "Sign out everywhere" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Sign out everywhere else?" });
    await user.click(within(confirm).getByRole("button", { name: "Sign out everywhere" }));
    expect(await (await notifications()).findByText("Signed out of 1 other session")).toBeInTheDocument();
    expect(requests("POST", "me/sessions/revoke-all")).toHaveLength(1);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });
});
