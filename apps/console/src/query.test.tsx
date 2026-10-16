import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { api, data } from "~/api/client";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { sessionItem } from "~/test/fixtures/instance-account";
import { memberUser } from "~/test/fixtures/project";
import { apiError, json, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { queryClient } from "./query";

const notifications = () => within(screen.getByRole("region", { name: "Notifications" }));

describe("data that fails to load", () => {
  it("shows the API's message with a way to try again", async () => {
    signedIn();
    let refused = true;
    on("GET", "me/sessions", () => (refused ? apiError(409, "conflict", "Sessions are being rotated; try again.") : { data: [sessionItem()] }));
    const { user } = await renderApp("/settings/account/sessions");
    expect((await notifications().findAllByText("Sessions are being rotated; try again.")).length).toBeGreaterThan(0);
    refused = false;
    await user.click(await notifications().findByRole("button", { name: "Try again", hidden: true }));
    await waitFor(() => expect(requests("GET", "me/sessions")).toHaveLength(2));
  });

  it("names the missing right and who can give it", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/playground/models", apiError(403, "permission_denied", "Forbidden.", { right: "playground.use" }));
    on("GET", "orgs/:org/projects/:project/rights/:right/grantors", page([memberUser({ name: "Lin Wei" })]));
    await renderApp(`/orgs/${ORG}/projects/${PROJECT}/playground`);
    expect((await notifications().findAllByText(/^You need .+; ask Lin Wei for it\.$/)).length).toBeGreaterThan(0);
    expect(notifications().queryByText("Forbidden.")).not.toBeInTheDocument();
  });

  it("stays quiet about something that doesn't exist", async () => {
    signedIn();
    on("GET", "me/sessions", apiError(404, "not_found", "No sessions here."));
    await renderApp("/settings/account/sessions");
    await waitFor(() => expect(requests("GET", "me/sessions")).toHaveLength(1));
    await screen.findByRole("heading", { name: "Sessions", level: 1 });
    expect(notifications().queryByText("No sessions here.")).not.toBeInTheDocument();
  });
});

describe("asking again", () => {
  const fetchOnce = () => queryClient.fetchQuery({ queryKey: ["me", "retry"], queryFn: async () => data(await api.GET("/platform/v1/me")) });

  it("asks twice more after the gateway fails", async () => {
    on("GET", "me", json({ error: { type: "internal", message: "Try again." } }, { status: 500 }));
    await expect(fetchOnce()).rejects.toThrow("Try again.");
    expect(requests("GET", "me")).toHaveLength(3);
  });

  it("takes a refusal as the answer", async () => {
    on("GET", "me", apiError(422, "invalid_request", "No."));
    await expect(fetchOnce()).rejects.toThrow("No.");
    expect(requests("GET", "me")).toHaveLength(1);
  });
});
