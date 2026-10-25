import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, OTHER_PROJECT, PROJECT, signedIn } from "~/test/fixtures";
import { AGENT, agentView, agents, grantable } from "~/test/fixtures/agents";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

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
    expect(screen.getByText(/\/mcp$/)).toBeInTheDocument();
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

  it("adds the right an agent asked for once the person saves", async () => {
    signedIn();
    agentApi();
    on("PUT", "me/agents/:agent/rights", agentView());
    const { user } = await renderApp(`/settings/account/agents/${AGENT}?add=limits.manage&org=${ORG}&project=${PROJECT}`);
    expect(await screen.findByText("The agent asks for Manage limits")).toBeInTheDocument();
    const dialog = await screen.findByRole("dialog", { name: "Customize Release helper" });
    expect(within(dialog).getByRole("group", { name: "Rights in Northwind" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await user.click(screen.getByRole("button", { name: "Save rights" }));
    await waitFor(() => expect(requests("PUT", "me/agents/:agent/rights")).toHaveLength(1));
    const [sent] = requests("PUT", "me/agents/:agent/rights").map(
      (r) => r.body as { customized: boolean; orgs: { org: string; projects: { rights: string[] } }[] },
    );
    expect(sent?.customized).toBe(true);
    expect(sent?.orgs.find((o) => o.org === ORG)?.projects.rights).toEqual(expect.arrayContaining(["limits.manage", "limits.view"]));
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
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: "Save rights" })).toBeDisabled();
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

  it("says when there are no tool calls yet", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent/activity", page([]));
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText("No tool calls yet")).toBeInTheDocument();
  });

  it("says when the activity couldn't load", async () => {
    signedIn();
    agentApi();
    on("GET", "me/agents/:agent/activity", apiError(503, "unavailable", "Kaeso cannot reach its database right now; try again shortly."));
    await renderApp(`/settings/account/agents/${AGENT}`);
    expect(await screen.findByText("The activity couldn't load")).toBeInTheDocument();
  });
});
