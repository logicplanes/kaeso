import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { ORG, signedIn, withRights } from "~/test/fixtures";
import { hoverReason, logSettings, notifications } from "~/test/fixtures/org";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/logging`;
const POLICY = "orgs/:org/response-cache";

describe("organization response cache", () => {
  it("forbids and allows the response cache for every project", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    let current = { allowed: true, available: true };
    on("GET", POLICY, () => current);
    on("PATCH", POLICY, ({ body }: { body: { allowed: boolean } }) => (current = { allowed: body.allowed, available: true }));
    const { user } = await renderApp(PATH);

    const toggle = await screen.findByRole("switch", { name: "Allow the response cache" });
    await waitFor(() => expect(toggle).toBeChecked());
    expect(screen.getByText("Each project turns it on in its settings.")).toBeInTheDocument();
    await user.click(toggle);
    expect(await notifications().findByText("Response cache turned off")).toBeInTheDocument();
    expect(await screen.findByText("No project can use it, and stored replies are cleared.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Allow the response cache" }));
    expect(await notifications().findByText("Response cache allowed")).toBeInTheDocument();
    expect(requests("PATCH", POLICY).map((r) => r.body)).toEqual([{ allowed: false }, { allowed: true }]);
  });

  it("can't change without the right to edit the organization", async () => {
    signedIn();
    withRights(["org.view"], { role: "auditor" });
    on("GET", "orgs/:org/logs/settings", logSettings());
    on("GET", POLICY, { allowed: true, available: true });
    await renderApp(PATH);
    const toggle = await screen.findByRole("switch", { name: "Allow the response cache" });
    expect(toggle).toHaveAttribute("aria-disabled", "true");
  });

  it("says when the install doesn't offer it", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    on("GET", POLICY, { allowed: true, available: false });
    const { user } = await renderApp(PATH);
    const toggle = await screen.findByRole("switch", { name: "Allow the response cache" });
    expect(await screen.findByText("This install doesn't offer it.")).toBeInTheDocument();
    expect(toggle).not.toBeChecked();
    await hoverReason(user, toggle, "Off on this install");
  });
});
