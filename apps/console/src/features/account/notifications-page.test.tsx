import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/notifications";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("notification settings", () => {
  it("shows which alert emails are on", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: true, key_expiry: false });
    await renderApp(PATH);
    expect(await screen.findByRole("switch", { name: "Budget alerts" })).toBeChecked();
    expect(screen.getByRole("switch", { name: "Expiring keys" })).not.toBeChecked();
    expect(screen.getByText("To your account's address; security emails are always sent.")).toBeInTheDocument();
  });

  it("saves each switch with the other one kept", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: true, key_expiry: false });
    on("PUT", "me/notifications", ({ body }: { body: unknown }) => body);
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Expiring keys" }));
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Budget alerts" }));
    await waitFor(() => expect(requests("PUT", "me/notifications")).toHaveLength(2));
    expect(requests("PUT", "me/notifications").map((r) => r.body)).toEqual([
      { budget_alerts: true, key_expiry: true },
      { budget_alerts: false, key_expiry: true },
    ]);
  });

  it("shows the change while it saves and keeps it for the next switch", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: true, key_expiry: false });
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    on("PUT", "me/notifications", async ({ body }: { body: unknown }) => {
      await held;
      return body;
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Expiring keys" }));
    await waitFor(() => expect(screen.getByRole("switch", { name: "Budget alerts" })).toHaveAttribute("aria-disabled", "true"));
    expect(screen.getByRole("switch", { name: "Expiring keys" })).toBeChecked();
    release();
    expect(await (await notifications()).findByText("Notifications saved")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("switch", { name: "Budget alerts" })).not.toHaveAttribute("aria-disabled"));
    await user.click(screen.getByRole("switch", { name: "Budget alerts" }));
    await waitFor(() => expect(requests("PUT", "me/notifications")).toHaveLength(2));
    expect(requests("PUT", "me/notifications")[1]?.body).toEqual({ budget_alerts: false, key_expiry: true });
  });

  it("says when the notification settings couldn't load", async () => {
    signedIn();
    on("GET", "me/notifications", apiError(429, "rate_limited", "Too many requests; try again in a minute."));
    await renderApp(PATH);
    expect(await screen.findByText("Your notification settings couldn't load")).toBeInTheDocument();
  });

  it("shows why saving was refused", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", apiError(422, "invalid_request", "Notifications can't be changed right now."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    expect((await (await notifications()).findAllByText("Notifications can't be changed right now.")).length).toBeGreaterThan(0);
  });
});
