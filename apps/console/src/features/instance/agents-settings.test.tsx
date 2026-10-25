import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { signedIn, user as account, whoAmI } from "~/test/fixtures";
import { agents } from "~/test/fixtures/agents";
import { instanceSettings, system } from "~/test/fixtures/instance-account";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

describe("the instance's agent policy", () => {
  it("lets an instance admin cap and turn off agents", async () => {
    signedIn({ session: whoAmI({ user: account({ is_instance_admin: true }) }) });
    let current = { allowed: true, max: "configure" };
    on("GET", "instance/system", system());
    on("GET", "instance/settings", instanceSettings());
    on("GET", "me/agents/catalogue", agents);
    on("GET", "instance/settings/agents", () => current);
    on("PUT", "instance/settings/agents", ({ body }: { body: typeof current }) => (current = body));
    const { user } = await renderApp("/settings/instance/general");
    expect(await screen.findByRole("switch", { name: "Allow agents" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Configure" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Read" }));
    await waitFor(() => expect(requests("PUT", "instance/settings/agents").map((r) => r.body)).toEqual([{ allowed: true, max: "read" }]));
    await user.click(screen.getByRole("switch", { name: "Allow agents" }));
    await waitFor(() => expect(requests("PUT", "instance/settings/agents").at(-1)?.body).toEqual({ allowed: false, max: "read" }));
  });
});
