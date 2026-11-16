import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { ORG, signedIn, withRights } from "~/test/fixtures";
import { logSettings, notifications } from "~/test/fixtures/org";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/logging`;
const POLICY = "orgs/:org/stored-responses";
const NAME = "Allow responses stored at the provider";

describe("organization stored responses", () => {
  it("allows and forbids responses kept at providers", async () => {
    signedIn();
    on("GET", "orgs/:org/logs/settings", logSettings());
    let current = { allowed: false };
    on("GET", POLICY, () => current);
    on("PATCH", POLICY, ({ body }: { body: { allowed: boolean } }) => (current = { allowed: body.allowed }));
    const { user } = await renderApp(PATH);

    const toggle = await screen.findByRole("switch", { name: NAME });
    await waitFor(() => expect(screen.getByText("Apps must send each conversation whole.")).toBeInTheDocument());
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    expect(await notifications().findByText("Stored responses allowed")).toBeInTheDocument();
    expect(await screen.findByText("Each project turns it on in its settings.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: NAME }));
    expect(await notifications().findByText("Stored responses turned off")).toBeInTheDocument();
    expect(requests("PATCH", POLICY).map((r) => r.body)).toEqual([{ allowed: true }, { allowed: false }]);
  });

  it("can't change without the right to edit the organization", async () => {
    signedIn();
    withRights(["org.view"], { role: "auditor" });
    on("GET", "orgs/:org/logs/settings", logSettings());
    on("GET", POLICY, { allowed: true });
    await renderApp(PATH);
    const toggle = await screen.findByRole("switch", { name: NAME });
    await waitFor(() => expect(toggle).toBeChecked());
    expect(toggle).toHaveAttribute("aria-disabled", "true");
  });

  it("shows the switch where request logs are off too", async () => {
    signedIn();
    on("GET", POLICY, { allowed: false });
    await renderApp(PATH);
    expect(await screen.findByRole("switch", { name: NAME })).toBeInTheDocument();
  });
});
