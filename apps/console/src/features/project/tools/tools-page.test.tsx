import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PortalGrantView, ProjectMcpServerView, ProjectToolView } from "~/api/generated/types";
import { ALL_ORG_RIGHTS, ALL_PROJECT_RIGHTS, signedIn, withRights } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { PORTAL_GRANTS, PROJECT_SERVERS, PROJECT_TOOLS, SERVERS, mcpServer, obj, portalGrant, projectServer, projectTool } from "~/test/fixtures/mcp";
import { me, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const post = projectTool({
  name: "create_post",
  title: "Create a post",
  annotations: obj({ destructiveHint: true, openWorldHint: true }),
  enabled: false,
  offered: false,
  calls_7d: 0,
  last_call_at: null,
});
const getMe = projectTool();
const changed = projectTool({ name: "search", state: "changed", offered: false, description: "", annotations: null, calls_7d: null, last_call_at: null });

function tools({
  list = [post, getMe],
  servers = [projectServer()],
  grants = [],
}: { list?: ProjectToolView[]; servers?: ProjectMcpServerView[]; grants?: PortalGrantView[] } = {}) {
  projectData({ members: [me()] });
  on("GET", PROJECT_TOOLS, page(list));
  on("GET", PROJECT_SERVERS, page(servers));
  on("GET", PORTAL_GRANTS, page(grants));
}

const list = () => screen.findByRole("grid", { name: "Tools" });

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Project › Tools", () => {
  it("lists each server's tools with their switch, hints, state and calls", async () => {
    signedIn();
    tools({ list: [post, getMe, changed, projectTool({ name: "old", state: "removed" })] });
    on("PATCH", PROJECT_TOOLS, page([]));
    const { user } = await renderApp(projectPath("tools"));
    const grid = await list();
    expect(within(grid).getByText("X")).toBeInTheDocument();
    expect(within(grid).getByText("Connected")).toBeInTheDocument();
    expect(within(grid).getByText("1 of 2 on")).toBeInTheDocument();
    const row = (name: string) => within(grid).getByRole("row", { name: new RegExp(name) });
    expect(row("x__create_post")).toHaveTextContent("Destructive");
    expect(row("x__create_post")).toHaveTextContent("Open world");
    expect(row("x__create_post")).toHaveTextContent("Off");
    expect(row("x__create_post")).toHaveTextContent("0 calls");
    expect(row("x__get_me")).toHaveTextContent("Read-only");
    expect(row("x__get_me")).toHaveTextContent("3 calls");
    expect(row("x__search")).toHaveTextContent("Changed");
    expect(within(grid).queryByText("x__old")).not.toBeInTheDocument();
    expect(screen.getByText("1 tool waits for review")).toBeInTheDocument();
    await hoverReason(user, within(row("x__search")).getByRole("switch"), "Waits for review");
    await user.click(within(row("x__create_post")).getByRole("switch", { name: "Offer x__create_post" }));
    expect(await notifications().findByText("x__create_post is on")).toBeInTheDocument();
    expect(requests("PATCH", PROJECT_TOOLS)[0]?.body).toEqual({ tools: [{ id: "tool_create_post", enabled: true }] });
    await user.click(within(row("x__get_me")).getByRole("switch"));
    expect(await notifications().findByText("x__get_me is off")).toBeInTheDocument();
  });

  it("turns every tool of a server on or off at once, leaving held tools alone", async () => {
    signedIn();
    tools({ list: [post, getMe, changed] });
    on("PATCH", PROJECT_TOOLS, page([]));
    const { user } = await renderApp(projectPath("tools"));
    const grid = await list();
    await user.click(within(grid).getByRole("button", { name: "Turn all on" }));
    expect(await notifications().findByText("1 tool of X on")).toBeInTheDocument();
    expect(requests("PATCH", PROJECT_TOOLS)[0]?.body).toEqual({ tools: [{ id: "tool_create_post", enabled: true }] });
    await user.click(within(grid).getByRole("button", { name: "Turn all off" }));
    expect(await notifications().findByText("1 tool of X off")).toBeInTheDocument();
    expect(requests("PATCH", PROJECT_TOOLS)[1]?.body).toEqual({ tools: [{ id: "tool_get_me", enabled: false }] });
  });

  it("offers no bulk switch to someone who may not manage tools", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "tools.manage") });
    tools();
    await renderApp(projectPath("tools"));
    const grid = await list();
    expect(within(grid).queryByRole("button", { name: /Turn all/ })).toBeNull();
  });

  it("searches and filters tools by server, state and hint", async () => {
    signedIn();
    tools({ list: [post, getMe, changed] });
    const { user } = await renderApp(projectPath("tools?server=mcp_x&state=on&hint=read_only"));
    const grid = await list();
    expect(within(grid).getByText("x__get_me")).toBeInTheDocument();
    expect(within(grid).queryByText("x__create_post")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove the State filter" }));
    await waitFor(() => expect(window.location.search).not.toContain("state="));
    await user.click(screen.getByRole("button", { name: "Remove the Hint filter" }));
    await user.click(screen.getByRole("button", { name: "Remove the Server filter" }));
    await waitFor(() => expect(window.location.search).toBe(""));
    await user.type(screen.getByRole("searchbox", { name: "Search tools" }), "nothing like it");
    expect(await screen.findByText("No tools match")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(await within(await list()).findByText("x__create_post")).toBeInTheDocument();
    await user.type(screen.getByRole("searchbox", { name: "Search tools" }), "nothing like it");
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(await within(await list()).findByText("x__create_post")).toBeInTheDocument();
  });

  it("filters through the chips", async () => {
    signedIn();
    tools({ list: [post, getMe, changed] });
    const { user } = await renderApp(projectPath("tools"));
    await list();
    for (const [chip, option, param] of [
      ["Server", "X", "server=mcp_x"],
      ["State", "Changed", "state=changed"],
      ["Hint", "Destructive", "hint=destructive"],
    ] as const) {
      await user.click(within(screen.getByRole("group", { name: `Filter: ${chip}` })).getByRole("button", { name: "any" }));
      await user.click(await screen.findByRole("menuitemradio", { name: option }));
      await waitFor(() => expect(window.location.search).toContain(param));
    }
    expect(await screen.findByText("No tools match")).toBeInTheDocument();
  });

  it("explains a project without tools", async () => {
    signedIn();
    tools({ list: [], servers: [] });
    const { user } = await renderApp(projectPath("tools"));
    expect(await screen.findByText("No tools yet")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Use a server" }));
    await waitFor(() => expect(window.location.search).toContain("tab=servers"));
    expect(await screen.findByText("No servers yet")).toBeInTheDocument();
  });

  it("says when tools or servers couldn't load", async () => {
    signedIn();
    projectData({ members: [me()] });
    on("GET", PROJECT_TOOLS, apiError(403, "permission_denied", "You can't see these tools."));
    on("GET", PROJECT_SERVERS, apiError(403, "permission_denied", "You can't see these servers."));
    on("GET", PORTAL_GRANTS, page([]));
    const { user } = await renderApp(projectPath("tools"));
    expect(await screen.findByText("The tools couldn't load")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /^Servers/ }));
    expect(await screen.findByText("The servers couldn't load")).toBeInTheDocument();
  });

  it("names the right that switching tools needs", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "tools.manage") });
    tools();
    const { user } = await renderApp(projectPath("tools"));
    const grid = await list();
    await hoverReason(user, within(within(grid).getByRole("row", { name: /x__get_me/ })).getByRole("switch"), /Manage tools/);
  });

  it("opens a tool with its details, schema and calls", async () => {
    signedIn();
    tools({ list: [post, getMe, changed] });
    const { user } = await renderApp(projectPath("tools"));
    await user.click(within(await list()).getByText("x__create_post"));
    const sheet = await screen.findByRole("dialog", { name: "x__create_post" });
    expect(within(sheet).getByText("create_post")).toBeInTheDocument();
    expect(within(sheet).getByText("Destructive")).toBeInTheDocument();
    expect(within(sheet).getByText("Calls in 7 days")).toBeInTheDocument();
    expect(within(sheet).getByText("Never")).toBeInTheDocument();
    expect(within(sheet).getByText("The create_post tool.")).toBeInTheDocument();
    expect(within(sheet).getByText("Input schema")).toBeInTheDocument();
    expect(within(sheet).getByText("Nobody: the portal doesn't offer it now.")).toBeInTheDocument();
    expect(within(sheet).getByRole("link", { name: "See its calls" })).toHaveAttribute("href", expect.stringContaining("call_tool=x__create_post"));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(window.location.search).not.toContain("tool="));
    await user.click(within(await list()).getByText("x__get_me"));
    const second = await screen.findByRole("dialog", { name: "x__get_me" });
    expect(within(second).getByText("People and keys with Use tools whose tool access includes it.")).toBeInTheDocument();
  });

  it("sends people who may review to a changed tool's server", async () => {
    signedIn();
    tools({ list: [changed] });
    await renderApp(projectPath("tools?tool=tool_search"));
    const sheet = await screen.findByRole("dialog", { name: "x__search" });
    expect(within(sheet).getByText("X changed this tool")).toBeInTheDocument();
    expect(within(sheet).getByRole("link", { name: "Review the change" })).toHaveAttribute("href", expect.stringContaining("/mcp-servers?server=mcp_x"));
  });

  it("explains a changed tool to people who can't review it", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "mcp_servers.view" && r !== "mcp_servers.manage"));
    tools({ list: [changed] });
    await renderApp(projectPath("tools?tool=tool_search"));
    const sheet = await screen.findByRole("dialog", { name: "x__search" });
    expect(within(sheet).getByText("The portal offers it again once an admin approves the change.")).toBeInTheDocument();
    expect(within(sheet).queryByRole("link", { name: "Review the change" })).not.toBeInTheDocument();
  });

  it("says when an opened tool is gone", async () => {
    signedIn();
    tools();
    await renderApp(projectPath("tools?tool=tool_gone"));
    expect(await screen.findByText("This tool isn't in the project anymore.")).toBeInTheDocument();
  });
});

describe("Project › Tools › Servers", () => {
  it("lists the servers the project uses and adds one from the organization", async () => {
    signedIn();
    tools({
      servers: [
        projectServer(),
        projectServer({ id: "mcp_docs", name: "Docs", auth_method: "none", tools: 0, tools_on: 0, calls_7d: null, last_call_at: null }),
      ],
    });
    on(
      "GET",
      SERVERS,
      page([mcpServer(), mcpServer({ id: "mcp_docs", name: "Docs" }), mcpServer({ id: "mcp_github", name: "GitHub", url: "https://api.github.example/mcp" })]),
    );
    on("PUT", PROJECT_SERVERS, ({ body }: { body: { servers: string[] } }) =>
      page(body.servers.map((id) => projectServer({ id, name: id === "mcp_github" ? "GitHub" : id }))),
    );
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    const grid = await screen.findByRole("grid", { name: "Servers" });
    const x = within(grid).getByRole("row", { name: /^X/ });
    expect(x).toHaveTextContent("mcp.x.example");
    expect(x).toHaveTextContent("Connected");
    expect(x).toHaveTextContent("4 calls in 7 days");
    expect(within(grid).getByRole("row", { name: /^Docs/ })).toHaveTextContent("No sign-in");
    await user.click(screen.getByRole("button", { name: "Use a server" }));
    const pick = await screen.findByRole("combobox", { name: "Server" });
    await user.click(pick);
    expect(screen.queryByRole("option", { name: /^X/ })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("option", { name: /GitHub/ }));
    expect(await notifications().findByText("GitHub is in use; turn its tools on")).toBeInTheDocument();
    expect(requests("PUT", PROJECT_SERVERS)[0]?.body).toEqual({ servers: ["mcp_x", "mcp_docs", "mcp_github"] });
    await user.click(within(screen.getByRole("grid", { name: "Servers" })).getByText("Docs"));
    await waitFor(() => expect(window.location.search).not.toContain("tab="));
  });

  it("sorts the servers by name, calls, last use or tools on", async () => {
    signedIn();
    tools({
      servers: [
        projectServer({ id: "mcp_b", name: "Bravo", tools: 4, tools_on: 1, calls_7d: 9, last_call_at: "2026-10-14T11:00:00Z" }),
        projectServer({ id: "mcp_a", name: "Alpha", tools: 3, tools_on: 1, calls_7d: 9, last_call_at: "2026-10-14T11:00:00Z" }),
        projectServer({ id: "mcp_c", name: "Charlie", tools: 0, tools_on: 0, calls_7d: null, last_call_at: null }),
        projectServer({ id: "mcp_d", name: "Delta", tools: 3, tools_on: 1, calls_7d: 0, last_call_at: "2026-10-01T11:00:00Z" }),
      ],
    });
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    const names = () =>
      within(screen.getByRole("grid", { name: "Servers" }))
        .getAllByRole("row")
        .map((r) => r.textContent?.match(/^(Alpha|Bravo|Charlie|Delta)/)?.[1]);
    await screen.findByRole("grid", { name: "Servers" });
    expect(names()).toEqual(["Alpha", "Bravo", "Charlie", "Delta"]);
    expect(
      within(screen.getByRole("grid", { name: "Servers" })).getByRole("row", { name: /^Alpha/ }).querySelector('[role="meter"]'),
    ).toHaveAttribute("aria-valuenow", "33");
    const sort = screen.getByRole("combobox", { name: "Sort" });
    await user.click(sort);
    await user.click(await screen.findByRole("option", { name: "Calls in 7 days" }));
    await waitFor(() => expect(window.location.search).toContain("sort=calls"));
    expect(names()).toEqual(["Alpha", "Bravo", "Delta", "Charlie"]);
    await user.click(sort);
    await user.click(await screen.findByRole("option", { name: "Tools on" }));
    await waitFor(() => expect(names()).toEqual(["Bravo", "Alpha", "Delta", "Charlie"]));
    await user.click(sort);
    await user.click(await screen.findByRole("option", { name: "Last used" }));
    await waitFor(() => expect(window.location.search).toContain("sort=used"));
    expect(names()).toEqual(["Alpha", "Bravo", "Delta", "Charlie"]);
    await user.click(sort);
    await user.click(await screen.findByRole("option", { name: "Name" }));
    await waitFor(() => expect(window.location.search).not.toContain("sort="));
  });

  it("stops using a server", async () => {
    signedIn();
    tools({ servers: [projectServer(), projectServer({ id: "mcp_docs", name: "Docs" })] });
    on("PUT", PROJECT_SERVERS, page([]));
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    await user.click(await screen.findByRole("button", { name: "Actions for X" }));
    await user.click(await screen.findByRole("menuitem", { name: "Stop using" }));
    const alert = await screen.findByRole("alertdialog", { name: "Stop using X?" });
    await user.click(within(alert).getByRole("button", { name: "Stop using" }));
    expect(await notifications().findByText("X isn't used anymore")).toBeInTheDocument();
    expect(requests("PUT", PROJECT_SERVERS)[0]?.body).toEqual({ servers: ["mcp_docs"] });
  });

  it("closes the confirmation when stopping fails", async () => {
    signedIn();
    tools();
    on("PUT", PROJECT_SERVERS, apiError(409, "conflict", "The project is archived; its tools cannot change."));
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    await user.click(await screen.findByRole("button", { name: "Actions for X" }));
    await user.click(await screen.findByRole("menuitem", { name: "Stop using" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Stop using" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("names the rights that using a server needs", async () => {
    withRights(
      ALL_ORG_RIGHTS.filter((r) => r !== "mcp_servers.view" && r !== "mcp_servers.manage"),
      { projectRights: ALL_PROJECT_RIGHTS },
    );
    tools({ servers: [] });
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    await hoverReason(user, await screen.findByRole("button", { name: "Use a server" }), /View MCP servers/);
  });

  it("names the project right first", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "tools.manage") });
    tools();
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    await hoverReason(user, await screen.findByRole("button", { name: "Use a server" }), /Manage tools/);
    expect(screen.queryByRole("button", { name: "Actions for X" })).not.toBeInTheDocument();
  });
});

describe("Project › Tools › Connect", () => {
  it("shows the portal address, how to add it to each client, and connected clients", async () => {
    signedIn();
    tools({
      grants: [
        portalGrant(),
        portalGrant({ id: "pgr_cursor", client_name: "Cursor", redirect_uri: "http://127.0.0.1:5173/callback", last_used_at: "2026-10-15T11:30:00Z" }),
      ],
    });
    on("DELETE", `${PORTAL_GRANTS}/:grant`, noContent);
    const { user } = await renderApp(projectPath("tools"));
    await user.click(await screen.findByRole("button", { name: "Connect a client" }));
    expect(await screen.findByText(`${window.location.origin}/mcp/proj_checkout`)).toBeInTheDocument();
    for (const [client, text] of [
      ["Claude", "In Claude, open Settings › Connectors, choose Add custom connector and paste the portal address."],
      ["ChatGPT", "In ChatGPT, open Settings › Apps & Connectors, create a connector and paste the portal address."],
      ["Cursor", "Add to Cursor"],
      ["GitHub Copilot", "Add to VS Code"],
    ] as const) {
      await user.click(screen.getByRole("button", { name: `Add to ${client}` }));
      const dialog = await screen.findByRole("dialog", { name: `Add this portal to ${client}` });
      expect(within(dialog).getByText(text)).toBeInTheDocument();
      await user.click(within(dialog).getByRole("button", { name: "Finish" }));
      await waitFor(() => expect(screen.queryByRole("dialog", { name: `Add this portal to ${client}` })).not.toBeInTheDocument());
    }
    expect(screen.getByRole("link", { name: "Create an API key" })).toHaveAttribute("href", expect.stringContaining("/keys?create=true"));
    const clients = screen.getByRole("grid", { name: "Connected clients" });
    expect(within(clients).getByRole("row", { name: /^Claude/ })).toHaveTextContent("receives sign-ins at claude.ai");
    expect(within(clients).getByRole("row", { name: /^Cursor/ })).toHaveTextContent("127.0.0.1:5173");
    await user.click(within(within(clients).getByRole("row", { name: /^Claude/ })).getByRole("button", { name: "Disconnect" }));
    const alert = await screen.findByRole("alertdialog", { name: "Disconnect Claude?" });
    await user.click(within(alert).getByRole("button", { name: "Disconnect" }));
    expect(await notifications().findByText("Claude disconnected")).toBeInTheDocument();
    expect(requests("DELETE", `${PORTAL_GRANTS}/:grant`)[0]?.params.grant).toBe("pgr_claude");
  });

  it("closes the confirmation when disconnecting fails, and names the right it needs", async () => {
    signedIn();
    tools({ grants: [portalGrant()] });
    on("DELETE", `${PORTAL_GRANTS}/:grant`, apiError(404, "not_found", "No such portal client, or you cannot see it."));
    const { user } = await renderApp(projectPath("tools?tab=connect"));
    await user.click(await screen.findByRole("button", { name: "Disconnect" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Disconnect" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("disables disconnecting without the right to manage tools", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "tools.manage") });
    tools({ grants: [portalGrant()] });
    const { user } = await renderApp(projectPath("tools?tab=connect"));
    await hoverReason(user, await screen.findByRole("button", { name: "Disconnect" }), /Manage tools/);
  });

  it("explains a portal without clients", async () => {
    signedIn();
    tools();
    await renderApp(projectPath("tools?tab=connect"));
    expect(await screen.findByText("No clients connected yet")).toBeInTheDocument();
  });

  it("switches between the tabs", async () => {
    signedIn();
    tools();
    const { user } = await renderApp(projectPath("tools"));
    await user.click(await screen.findByRole("tab", { name: /^Connect/ }));
    await waitFor(() => expect(window.location.search).toContain("tab=connect"));
    await user.click(screen.getByRole("tab", { name: /^Tools/ }));
    await waitFor(() => expect(window.location.search).toBe(""));
  });

  it("opens the connect tab with the create shortcut", async () => {
    signedIn();
    tools();
    const { user } = await renderApp(projectPath("tools?tab=servers"));
    await screen.findByRole("grid", { name: "Servers" });
    await user.keyboard("c");
    await waitFor(() => expect(window.location.search).toContain("tab=connect"));
    expect(screen.queryByRole("button", { name: "Connect a client" })).not.toBeInTheDocument();
  });
});
