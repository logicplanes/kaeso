import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import { signedOut } from "~/test/fixtures";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const LINK = "/unsubscribe#token=usr_1.org_members.tag";
const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));
const lookup = { kind: "org_members", name: "Member changes", email: "owner@example.com", delivery: "daily" };

describe("unsubscribing from an email", () => {
  it("asks first, then turns that one email off", async () => {
    signedOut();
    on("GET", "notifications/unsubscribe", lookup);
    on("POST", "notifications/unsubscribe", { ...lookup, delivery: "off" });
    const { user } = await renderApp(LINK);
    expect(await screen.findByRole("heading", { name: "Unsubscribe from “Member changes”?", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("Kaeso stops sending these to owner@example.com; security emails still arrive.")).toBeInTheDocument();
    expect(window.location.hash).toBe("");
    expect(requests("GET", "notifications/unsubscribe").map((r) => r.query.get("token"))).toEqual(["usr_1.org_members.tag"]);
    expect(requests("POST", "notifications/unsubscribe")).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "Unsubscribe" }));
    expect(await screen.findByRole("heading", { name: "You're unsubscribed", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("Kaeso no longer sends “Member changes” to owner@example.com.")).toBeInTheDocument();
    expect(requests("POST", "notifications/unsubscribe").map((r) => r.query.get("token"))).toEqual(["usr_1.org_members.tag"]);
    expect(screen.getByRole("link", { name: "Choose other emails" })).toHaveAttribute("href", "/settings/account/notifications");
  });

  it("says so at once when the email is already off", async () => {
    signedOut();
    on("GET", "notifications/unsubscribe", { ...lookup, kind: "gossip", name: "Gossip", delivery: "off" });
    await renderApp(LINK);
    expect(await screen.findByRole("heading", { name: "You're unsubscribed", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("Kaeso no longer sends “Gossip” to owner@example.com.")).toBeInTheDocument();
  });

  it("shows why turning it off failed and keeps the question", async () => {
    signedOut();
    on("GET", "notifications/unsubscribe", lookup);
    on("POST", "notifications/unsubscribe", apiError(503, "unavailable", "Kaeso can't reach its database right now; try again shortly."));
    const { user } = await renderApp(LINK);
    await user.click(await screen.findByRole("button", { name: "Unsubscribe" }));
    expect((await (await notifications()).findAllByText("Kaeso can't reach its database right now; try again shortly.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("heading", { name: "Unsubscribe from “Member changes”?", level: 1 })).toBeInTheDocument();
  });

  it("says when the link is invalid", async () => {
    signedOut();
    on("GET", "notifications/unsubscribe", apiError(404, "not_found", "This unsubscribe link is invalid; choose your emails under Account › Notifications."));
    await renderApp(LINK);
    expect(await screen.findByRole("heading", { name: "This unsubscribe link is invalid", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open notification settings" })).toHaveAttribute("href", "/settings/account/notifications");
  });
});
