import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import { ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { memberUser } from "~/test/fixtures/public";
import { apiError, on } from "~/test/network";
import { renderApp } from "~/test/render";
import { placeOf } from "./missing-right-toast";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

describe("where a refusal happened", () => {
  it("reads the organization and project from the address", () => {
    expect(placeOf(`/orgs/${ORG}/projects/${PROJECT}/keys`)).toEqual({ org: ORG, project: PROJECT });
    expect(placeOf(`/settings/orgs/${ORG}/members`)).toEqual({ org: ORG, project: undefined });
    expect(placeOf("/settings/account/profile")).toEqual({ org: undefined, project: undefined });
  });
});

describe("a change refused for a missing right", () => {
  it("names the right and who can give it inside an organization", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ org_state: "scheduled_for_deletion" })] }) });
    on("POST", "orgs/:org/restore", apiError(403, "permission_denied", "Missing org.delete.", { right: "org.delete" }));
    on("GET", "orgs/:org/rights/:right/grantors", { data: [memberUser("Grace Hopper")] });
    const { user } = await renderApp(`/orgs/${ORG}`);
    await user.click(await screen.findByRole("button", { name: "Restore organization" }));
    expect(await (await notifications()).findByText("You need Delete organization; ask Grace Hopper for it.")).toBeInTheDocument();
  });

  it("shows the gateway's message outside an organization", async () => {
    signedIn();
    on("GET", "me/notifications", { budget_alerts: false, key_expiry: false });
    on("PUT", "me/notifications", apiError(403, "permission_denied", "Your account can't change notifications.", { right: "org.edit" }));
    const { user } = await renderApp("/settings/account/notifications");
    await user.click(await screen.findByRole("switch", { name: "Budget alerts" }));
    expect((await (await notifications()).findAllByText("Your account can't change notifications.")).length).toBeGreaterThan(0);
  });
});
