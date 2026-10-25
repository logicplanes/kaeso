import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, membership, signedIn, whoAmI } from "~/test/fixtures";
import { agents, orgAgent } from "~/test/fixtures/agents";
import { security } from "~/test/fixtures/org";
import { noContent, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/settings/orgs/${ORG}/security`;

function agentPolicy(allowed = true, max = "full") {
  let current = { allowed, max };
  on("GET", "orgs/:org/security", security());
  on("GET", "me/agents/catalogue", agents);
  on("GET", "orgs/:org/security/agents", () => current);
  on("PUT", "orgs/:org/security/agents", ({ body }: { body: typeof current }) => (current = body));
  on("GET", "orgs/:org/agents", { data: [orgAgent()], has_more: false, next_cursor: null });
}

describe("agents in an organization", () => {
  it("shows the policy and narrows it", async () => {
    signedIn();
    agentPolicy();
    const { user } = await renderApp(PATH);
    expect(await screen.findByRole("switch", { name: "Allow members' agents" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Observer" }));
    await waitFor(() => expect(requests("PUT", "orgs/:org/security/agents").map((r) => r.body)).toEqual([{ allowed: true, max: "observer" }]));
    await user.click(screen.getByRole("switch", { name: "Allow members' agents" }));
    await waitFor(() => expect(requests("PUT", "orgs/:org/security/agents").at(-1)?.body).toEqual({ allowed: false, max: "observer" }));
  });

  it("lists members' agents and closes the organization to one", async () => {
    signedIn();
    agentPolicy();
    on("DELETE", "orgs/:org/agents/:agent", noContent);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Release helper of Ada Okafor")).toBeInTheDocument();
    expect(screen.getByText("Codex · used never")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close access" }));
    expect(await screen.findByText("Close this organization to Release helper?")).toBeInTheDocument();
    await user.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Close access" }));
    await waitFor(() => expect(requests("DELETE", "orgs/:org/agents/:agent")).toHaveLength(1));
  });

  it("shows the policy to members who can't change it", async () => {
    signedIn({ session: whoAmI({ memberships: [membership({ rights: ["org.view", "members.view"] })] }) });
    agentPolicy();
    on("GET", "orgs/:org/agents", { data: [orgAgent({ last_used_at: "2026-10-15T11:00:00Z" })], has_more: false, next_cursor: null });
    await renderApp(PATH);
    expect(await screen.findByRole("switch", { name: "Allow members' agents" })).toHaveAttribute("aria-disabled", "true");
    expect(await screen.findByText(/^Codex · used /)).not.toHaveTextContent("never");
  });
});
