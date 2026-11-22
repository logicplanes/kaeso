import { describe, expect, it } from "vitest";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, signedIn } from "~/test/fixtures";
import { AGENT, agentView, agents, grantable } from "~/test/fixtures/agents";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { hoverReason } from "~/test/fixtures/org";

function agentApi(view = agentView()) {
  on("GET", "me/agents/:agent", view);
  on("GET", "me/agents", { data: [view], has_more: false, next_cursor: null });
  on("GET", "me/agents/catalogue", agents);
  on("GET", "me/agents/grantable", grantable());
  on(
    "GET",
    "me/agents/:agent/activity",
    page([
      {
        id: "act_1",
        at: "2026-10-15T11:00:00Z",
        tool: "get_orgs_org_usage",
        org_id: ORG,
        project_id: null,
        target: null,
        status: 200,
        result: "ok",
        ip: "203.0.113.7",
      },
      {
        id: "act_2",
        at: "2026-10-15T10:00:00Z",
        tool: "post_orgs_org_projects",
        org_id: ORG,
        project_id: null,
        target: null,
        status: 403,
        result: "insufficient_rights",
        ip: null,
      },
    ]),
  );
}

describe("the agents tab", () => {
  it("lists agents with their model, summary and dates", async () => {
    signedIn();
    agentApi();
    await renderApp("/settings/account/agents");
    expect(await screen.findByText("Release helper")).toBeInTheDocument();
    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(await screen.findByText(/^Manages .* in Northwind\.$/)).toBeInTheDocument();
    expect(screen.getByText(/^Added .* · used .+$/)).toBeInTheDocument();
    expect(screen.getByText(`${window.location.origin}/mcp`)).toBeInTheDocument();
  });

  it("shows how to add this instance's MCP server to Claude, Cursor and GitHub Copilot", async () => {
    signedIn();
    agentApi();
    const { user } = await renderApp("/settings/account/agents");
    const url = `${window.location.origin}/mcp`;
    await user.click(await screen.findByRole("button", { name: "Add to Claude" }));
    let dialog = await screen.findByRole("dialog", { name: "Add Kaeso to Claude" });
    const connector = new URL(within(dialog).getByRole("link", { name: "Add to Claude" }).getAttribute("href") ?? "");
    expect(`${connector.origin}${connector.pathname}`).toBe("https://claude.ai/customize/connectors");
    expect(connector.searchParams.get("modal")).toBe("add-custom-connector");
    expect(connector.searchParams.get("connectorName")).toBe("Kaeso");
    expect(connector.searchParams.get("connectorUrl")).toBe(url);
    expect(within(dialog).getByText("In Claude Code, connect the server, then run /mcp to sign in.")).toBeInTheDocument();
    await waitFor(() => expect(dialog).toHaveTextContent(`claude mcp add --transport http --scope user kaeso ${url}`));
    expect(
      within(dialog).getByText("Or install the plugin, which adds Kaeso's skill: it teaches your agent the tools and can connect the server too."),
    ).toBeInTheDocument();
    await waitFor(() => expect(dialog).toHaveTextContent("/plugin install kaeso --marketplace logicplanes/kaeso"));
    expect(within(dialog).queryByRole("tab")).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    await user.click(screen.getByRole("button", { name: "Add to Cursor" }));
    dialog = await screen.findByRole("dialog", { name: "Add Kaeso to Cursor" });
    const cursor = new URL(within(dialog).getByRole("link", { name: "Add to Cursor" }).getAttribute("href") ?? "");
    expect(cursor.protocol).toBe("cursor:");
    expect(cursor.searchParams.get("name")).toBe("kaeso");
    expect(JSON.parse(atob(cursor.searchParams.get("config") ?? ""))).toEqual({ url });
    await user.click(within(dialog).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    await user.click(screen.getByRole("button", { name: "Add to GitHub Copilot" }));
    dialog = await screen.findByRole("dialog", { name: "Add Kaeso to GitHub Copilot" });
    const vscode = within(dialog).getByRole("link", { name: "Add to VS Code" }).getAttribute("href") ?? "";
    expect(JSON.parse(decodeURIComponent(vscode.slice("vscode:mcp/install?".length)))).toEqual({ name: "kaeso", type: "http", url });
    expect(within(dialog).getByRole("link", { name: "Read the documentation" })).toHaveAttribute("href", "https://docs.kaeso.app/api/mcp");
  });

  it("names an agent's model as unknown until its first call, not the agent itself", async () => {
    signedIn();
    agentApi(agentView({ model: "unknown" }));
    const { user } = await renderApp("/settings/account/agents");
    expect(await screen.findByText("Unknown model")).toBeInTheDocument();
    await user.click(screen.getByText("Release helper"));
    expect(await screen.findByText(/^Unknown model · Added .* · used .+$/)).toBeInTheDocument();
    const save = screen.getByRole("button", { name: "Save" });
    await hoverReason(user, save, "No change to save");
    await user.clear(screen.getByLabelText("Name"));
    await hoverReason(user, save, "Enter a name");
  });

  it("says how to connect one when there are none", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents", { data: [], has_more: false, next_cursor: null });
    await renderApp("/settings/account/agents");
    expect(await screen.findByText("No agents yet")).toBeInTheDocument();
  });

  it("renames an agent and shows its tool calls", async () => {
    signedIn();
    agentApi();
    on("PATCH", "me/agents/:agent", agentView({ name: "Ops" }));
    const { user } = await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText("get_orgs_org_usage")).toBeInTheDocument();
    expect(screen.getByText(/^Missing a right · /)).toBeInTheDocument();
    const name = screen.getByLabelText("Name");
    await user.clear(name);
    await user.type(name, "Ops");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", "me/agents/:agent").map((r) => r.body)).toEqual([{ name: "Ops" }]));
  });

  it("keeps the name in a settings row with its Save beside it, like the profile's name", async () => {
    signedIn();
    agentApi();
    on("PATCH", "me/agents/:agent", agentView({ name: "Ops" }));
    const { user } = await renderApp(`/settings/account/agents/${AGENT}`);
    const name = await screen.findByLabelText("Name");
    expect(screen.getByText("Shown in your agent list and the audit log.")).toBeInTheDocument();
    expect(name.closest("[data-row-control]")).toContainElement(screen.getByRole("button", { name: "Save" }));
    await user.clear(name);
    await user.type(name, "Ops");
    fireEvent.keyDown(name, { key: "Enter", isComposing: true });
    expect(requests("PATCH", "me/agents/:agent"), "an Enter that confirms an input method's word doesn't save").toHaveLength(0);
    await user.type(name, "{Enter}");
    await waitFor(() => expect(requests("PATCH", "me/agents/:agent").map((r) => r.body)).toEqual([{ name: "Ops" }]));
  });

  it("adds the right an agent asked for once the person saves", async () => {
    signedIn();
    agentApi();
    on("PUT", "me/agents/:agent/rights", agentView());
    const { user } = await renderApp(`/settings/account/agents/${AGENT}?add=limits.manage&org=${ORG}&project=${PROJECT}`);
    expect(await screen.findByText("The agent asks for Manage limits")).toBeInTheDocument();
    const dialog = await screen.findByRole("dialog", { name: "Customize Release helper" });
    expect(within(dialog).getByRole("group", { name: "Rights in Northwind" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await user.click(screen.getByRole("button", { name: "Save rights" }));
    await waitFor(() => expect(requests("PUT", "me/agents/:agent/rights")).toHaveLength(1));
    const [sent] = requests("PUT", "me/agents/:agent/rights").map(
      (r) => r.body as { customized: boolean; orgs: { org: string; projects: { rights: string[] } }[] },
    );
    expect(sent?.customized).toBe(true);
    expect(sent?.orgs.find((o) => o.org === ORG)?.projects.rights).toEqual(expect.arrayContaining(["limits.manage", "limits.view"]));
  });

  it("opens the rights editor from Customize", async () => {
    signedIn();
    agentApi();
    const { user } = await renderApp(`/settings/account/agents/${AGENT}`);
    await user.click(await screen.findByRole("button", { name: "Customize" }));
    expect(await screen.findByRole("dialog", { name: "Customize Release helper" })).toBeInTheDocument();
  });

  it("revokes an agent after asking", async () => {
    signedIn();
    agentApi();
    on("DELETE", "me/agents/:agent", noContent);
    const { user } = await renderApp(`/settings/account/agents/${AGENT}`);
    await user.click(await screen.findByRole("button", { name: "Revoke" }));
    expect(await screen.findByText("Revoke Release helper?")).toBeInTheDocument();
    await user.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "Revoke" }));
    await waitFor(() => expect(requests("DELETE", "me/agents/:agent")).toHaveLength(1));
    await waitFor(() => expect(window.location.pathname).toBe("/settings/account/agents"));
  });

  it("says when the agents couldn't load", async () => {
    signedIn();
    agentApi(agentView({ last_used_at: null }));
    on("GET", "me/agents", apiError(500, "internal", "Something went wrong in the gateway; report request req_1 to your admin."));
    await renderApp("/settings/account/agents");
    expect(await screen.findByText("Your agents couldn't load")).toBeInTheDocument();
  });

  it("says when an agent was never used", async () => {
    signedIn();
    agentApi(agentView({ last_used_at: null }));
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText(/^Claude · Added .* · used never$/)).toBeInTheDocument();
  });

  it("says when an agent is gone", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent", apiError(404, "not_found", "No such agent, or you cannot see it."));
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText("This agent was revoked or doesn't exist.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Back to agents" })).toHaveAttribute("href", "/settings/account/agents");
  });

  it("says when an agent couldn't load", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent", apiError(503, "unavailable", "Kaeso cannot reach its database right now; try again shortly."));
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText("This agent couldn't load")).toBeInTheDocument();
  });

  it("asks before saving sensitive rights and can discard changes", async () => {
    signedIn();
    agentApi();
    on("PUT", "me/agents/:agent/rights", agentView());
    on("PATCH", "me/agents/:agent", agentView({ name: "Ops" }));
    const { user } = await renderApp(`/settings/account/agents/${AGENT}`);
    await user.click(await screen.findByRole("radio", { name: "Full access" }));
    await user.click(screen.getByRole("button", { name: "Discard" }));
    expect(screen.getByRole("radio", { name: "Builder" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Full access" }));
    await user.click(screen.getByRole("button", { name: "Save rights" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/^It would get .*Delete organization/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(requests("PUT", "me/agents/:agent/rights")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Save rights" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Save rights" }));
    await waitFor(() => expect(requests("PUT", "me/agents/:agent/rights")).toHaveLength(1));
    const name = screen.getByLabelText("Name");
    await user.clear(name);
    await user.type(name, "Ops");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PATCH", "me/agents/:agent").map((r) => r.body)).toEqual([{ name: "Ops" }]));
  });

  it("says when the person lacks the right an agent asked for", async () => {
    signedIn();
    agentApi();
    const { user } = await renderApp(`/settings/account/agents/${AGENT}?add=limits.manage&org=${ORG}&project=${OTHER_PROJECT}`);
    expect(await screen.findByText("The agent asks for Manage limits, which you don't hold")).toBeInTheDocument();
    expect(screen.getByText("Ask an admin for it first; an agent never gets more than you hold.")).toBeInTheDocument();
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const save = screen.getByRole("button", { name: "Save rights" });
    expect(save).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, save, "No change to save");
    await hoverReason(user, screen.getByRole("button", { name: "Discard" }), "No change to discard");
  });

  it("pages through the activity", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent/activity", ({ query }: { query: URLSearchParams }) =>
      query.get("after")
        ? page([
            {
              id: "act_9",
              at: "2026-10-01T11:00:00Z",
              tool: "get_me",
              org_id: null,
              project_id: null,
              target: "usr_ada",
              status: 404,
              result: "not_found",
              ip: null,
            },
          ])
        : page(
            [
              {
                id: "act_1",
                at: "2026-10-15T11:00:00Z",
                tool: "get_orgs_org_usage",
                org_id: ORG,
                project_id: null,
                target: null,
                status: 200,
                result: "ok",
                ip: "203.0.113.7",
              },
            ],
            "act_1",
          ),
    );
    const { user } = await renderApp(`/settings/account/agents/${AGENT}`);
    await user.click(await screen.findByRole("button", { name: "Load more" }));
    expect(await screen.findByText(/^not_found · /)).toBeInTheDocument();
  });

  it("names the organization and project each call acted in", async () => {
    signedIn();
    agentApi();
    on(
      "GET",
      "me/agents/:agent/activity",
      page([
        {
          id: "act_p",
          at: "2026-10-15T11:00:00Z",
          tool: "get_orgs_org_projects_project_usage",
          org_id: ORG,
          project_id: PROJECT,
          target: null,
          status: 200,
          result: "ok",
          ip: null,
        },
        {
          id: "act_o",
          at: "2026-10-15T11:00:00Z",
          tool: "get_orgs_org_usage",
          org_id: ORG,
          project_id: null,
          target: null,
          status: 200,
          result: "ok",
          ip: null,
        },
        {
          id: "act_g",
          at: "2026-10-15T11:00:00Z",
          tool: "delete_orgs_org_credentials_credential",
          org_id: "org_gone",
          project_id: "prj_gone",
          target: null,
          status: 200,
          result: "ok",
          ip: null,
        },
        { id: "act_m", at: "2026-10-15T11:00:00Z", tool: "get_me", org_id: null, project_id: null, target: null, status: 200, result: "ok", ip: null },
      ]),
    );
    await renderApp(`/settings/account/agents/${AGENT}`);
    const title = async (tool: string) => (await screen.findByText(tool)).parentElement;
    expect(await title("get_orgs_org_projects_project_usage")).toHaveTextContent(/^get_orgs_org_projects_project_usageNorthwind › Checkout$/);
    expect(await title("get_orgs_org_usage")).toHaveTextContent(/^get_orgs_org_usageNorthwind$/);
    expect(await title("delete_orgs_org_credentials_credential")).toHaveTextContent(/^delete_orgs_org_credentials_credentialorg_gone › prj_gone$/);
    expect(await title("get_me")).toHaveTextContent(/^get_me$/);
  });

  it("names an organization the user can no longer open to an agent from their memberships", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/grantable", { ...grantable(), orgs: grantable().orgs.filter((o) => o.id !== ORG) });
    on(
      "GET",
      "me/agents/:agent/activity",
      page([
        {
          id: "act_l",
          at: "2026-10-15T11:00:00Z",
          tool: "get_orgs_org_projects_project_usage",
          org_id: ORG,
          project_id: PROJECT,
          target: null,
          status: 200,
          result: "ok",
          ip: null,
        },
      ]),
    );
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect((await screen.findByText("get_orgs_org_projects_project_usage")).parentElement).toHaveTextContent(/Northwind Labs › proj_\w+$/);
  });

  it("says when there are no tool calls yet, at a section's size rather than a page's", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent/activity", page([]));
    await renderApp(`/settings/account/agents/${AGENT}`);
    const empty = (await screen.findByRole("heading", { name: "No tool calls yet" })).parentElement?.parentElement;
    expect(empty).toHaveClass("py-48");
    expect(empty).not.toHaveClass("py-80");
  });

  it("says when the activity couldn't load", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent/activity", apiError(503, "unavailable", "Kaeso cannot reach its database right now; try again shortly."));
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText("The activity couldn't load")).toBeInTheDocument();
  });
});
