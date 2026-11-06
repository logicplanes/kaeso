import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { McpServerView, McpToolView } from "~/api/generated/types";
import { ALL_ORG_RIGHTS, ORG, OTHER_PROJECT, PROJECT, signedIn, withRights } from "~/test/fixtures";
import { hoverReason, member, notifications } from "~/test/fixtures/org";
import { SERVER, SERVERS, SERVER_TOOLS, mcpConnection, mcpServer, mcpTool, obj, probeView } from "~/test/fixtures/mcp";
import { apiError, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/orgs/${ORG}/mcp-servers`;

const changed = mcpTool({
  name: "create_post",
  title: "Create a post",
  state: "changed",
  hash: "b".repeat(64),
  long_description: true,
  annotations: obj({ destructiveHint: true }),
});

function serve(servers: McpServerView[], tools: McpToolView[] = [mcpTool(), changed]) {
  on("GET", "orgs/:org/members", page([member()]));
  on("GET", SERVERS, { data: servers });
  on(
    "GET",
    SERVER,
    ({ params }: { params: Record<string, string> }) =>
      servers.find((s) => s.id === params.server) ?? apiError(404, "not_found", "No such MCP server, or you cannot see it."),
  );
  on("GET", SERVER_TOOLS, page(tools));
}

const table = () => screen.findByRole("grid", { name: "MCP servers" });
const row = async (name: string) => (await within(await table()).findByText(name)).closest('[role="row"]') as HTMLElement;
const sheet = (name: string) => screen.findByRole("dialog", { name });

afterEach(() => {
  vi.restoreAllMocks();
});

describe("organization MCP servers", () => {
  it("lists each server with its connection, tools, projects and last check", async () => {
    signedIn();
    serve([
      mcpServer(),
      mcpServer({
        id: "mcp_github",
        name: "GitHub",
        handle: "github",
        url: "https://api.github.example/mcp",
        auth_method: "header",
        connection: mcpConnection({ kind: "header", header_name: "Authorization", account: null }),
        status: "unreachable",
        last_error: "Nothing answered.",
        projects: [],
        tools: { total: 0, changed: 0 },
      }),
      mcpServer({
        id: "mcp_docs",
        name: "Docs",
        handle: "docs",
        auth_method: "none",
        connection: null,
        status: "unchecked",
        last_checked_at: null,
        projects: ["proj_gone"],
      }),
    ]);
    const { user } = await renderApp(PATH);

    const x = await row("X");
    expect(x).toHaveTextContent("mcp.x.example");
    expect(x).toHaveTextContent("Connected as @kaesoapp");
    expect(x).toHaveTextContent("1 changed");
    expect(x).toHaveTextContent("Checkout assistant");
    const github = await row("GitHub");
    expect(github).toHaveTextContent("Sends Authorization");
    expect(github).toHaveTextContent("No project");
    expect(github).toHaveTextContent("Unreachable");
    const docs = await row("Docs");
    expect(docs).toHaveTextContent("Not checked yet");
    expect(docs).toHaveTextContent("proj_gone");
    await user.click(screen.getByRole("button", { name: "Add server" }));
    expect(await screen.findByRole("dialog", { name: "Add MCP server" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add MCP server" })).not.toBeInTheDocument());
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: "Add MCP server" })).toBeInTheDocument();
  });

  it("adds a server and opens it", async () => {
    signedIn();
    serve([]);
    on("POST", `${SERVERS}/probe`, probeView());
    on("POST", SERVERS, () => {
      serve([mcpServer({ id: "mcp_new", name: "mock-x", auth_method: "none", connection: null, projects: [] })]);
      return mcpServer({ id: "mcp_new", name: "mock-x", auth_method: "none", connection: null, projects: [] });
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("button", { name: "Add a server" }));
    const dialog = await screen.findByRole("dialog", { name: "Add MCP server" });
    await user.type(within(dialog).getByRole("textbox", { name: "Address" }), "https://mcp.x.example/mcp");
    await user.click(within(dialog).getByRole("button", { name: "Check address" }));
    await within(dialog).findByText("3 tools");
    for (let i = 0; i < 3; i++) await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(within(dialog).getByRole("button", { name: "Add server" }));
    expect(await notifications().findByText("mock-x added")).toBeInTheDocument();
    expect(await sheet("mock-x")).toBeInTheDocument();
    expect(window.location.search).toBe("?server=mcp_new");
  });

  it("explains an empty organization and offers to add a server", async () => {
    signedIn();
    serve([]);
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("No MCP servers yet")).toBeInTheDocument();
    expect(screen.getByText("Add a server so your projects' portals can offer its tools.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add server" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add a server" }));
    expect(await screen.findByRole("dialog", { name: "Add MCP server" })).toBeInTheDocument();
  });

  it("hides servers without the right to see them and disables adding without the right to manage them", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "mcp_servers.view" && r !== "mcp_servers.manage"));
    serve([mcpServer()]);
    await renderApp(PATH);
    expect(await screen.findByText("You can't see the MCP servers here")).toBeInTheDocument();
  });

  it("names the right that adding a server needs", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "mcp_servers.manage"));
    serve([mcpServer()]);
    const { user } = await renderApp(PATH);
    await row("X");
    await hoverReason(user, screen.getByRole("button", { name: "Add server" }), /Manage MCP servers/);
  });

  it("offers a retry when the servers couldn't load", async () => {
    signedIn();
    on("GET", SERVERS, apiError(403, "permission_denied", "You can't see these servers."));
    const { user } = await renderApp(PATH);
    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    serve([mcpServer()]);
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await row("X")).toBeInTheDocument();
  });

  it.each([
    ["connected", "Signed in to X"],
    ["declined", "The sign-in to X was declined."],
    ["expired", "The sign-in took too long; start it again."],
    ["failed", "The sign-in to X didn't finish; try again."],
  ])("says how a sign-in ended: %s", async (result, message) => {
    signedIn();
    serve([mcpServer()]);
    await renderApp(`${PATH}?server=mcp_x&result=${result}`);
    expect(await notifications().findByText(message)).toBeInTheDocument();
    await waitFor(() => expect(window.location.search).toBe("?server=mcp_x"));
    expect(await sheet("X")).toBeInTheDocument();
  });

  it("names the server generically when a sign-in ends for a server that's gone", async () => {
    signedIn();
    serve([]);
    await renderApp(`${PATH}?result=failed`);
    expect(await notifications().findByText("The sign-in to the server didn't finish; try again.")).toBeInTheDocument();
    await waitFor(() => expect(window.location.search).toBe(""));
  });
});

describe("an MCP server's sheet", () => {
  it("shows the server, its connection, its tools and the projects that use it", async () => {
    signedIn();
    serve([mcpServer()]);
    const { user } = await renderApp(PATH);
    await user.click(await row("X"));
    const s = await sheet("X");
    expect(await within(s).findByText("Working")).toBeInTheDocument();
    expect(within(s).getByText("1 tool waits for review")).toBeInTheDocument();
    expect(within(s).getByText("https://mcp.x.example/mcp")).toBeInTheDocument();
    expect(within(s).getByText("x-mcp 1.2.0")).toBeInTheDocument();
    expect(within(s).getByText("2026-07-28")).toBeInTheDocument();
    expect(within(s).getByText("Held for review")).toBeInTheDocument();
    expect(within(s).getByText("Ada Okafor (you), 15 Oct 2026")).toBeInTheDocument();
    expect(within(s).getByText("Connected as @kaesoapp")).toBeInTheDocument();
    expect(within(s).getByText("Never")).toBeInTheDocument();
    expect(await within(s).findByText("x__get_me")).toBeInTheDocument();
    expect(within(s).getByText("Create a post")).toBeInTheDocument();
    expect(within(s).getByText("Checkout assistant")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(window.location.search).toBe(""));
  });

  it("reviews a changed tool and approves it", async () => {
    signedIn();
    serve([mcpServer()]);
    on("GET", `${SERVER_TOOLS}/:tool/change`, {
      tool: changed,
      approved: { name: "create_post", description: "Posts to X.", inputSchema: { type: "object" }, annotations: obj({ destructiveHint: true }) },
      current: {
        name: "create_post",
        title: "Create a post",
        description: "Posts to X and follows everyone.",
        inputSchema: { type: "object", properties: { text: { type: "string" } } },
        annotations: obj({ destructiveHint: true }),
      },
    });
    on("POST", `${SERVER_TOOLS}/:tool/approve`, { ...changed, state: "active", approved_hash: changed.hash });
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Review" }));
    const review = await sheet("Review x__create_post");
    expect(await within(review).findByText("X changed this tool")).toBeInTheDocument();
    expect(within(review).getByText("Its description is longer than 2,000 characters; read all of it before you approve.")).toBeInTheDocument();
    expect(within(review).getByText("Description")).toBeInTheDocument();
    expect(within(review).getByText("Input schema")).toBeInTheDocument();
    expect(within(review).getByText("Title")).toBeInTheDocument();
    expect(within(review).queryByText("Hints")).not.toBeInTheDocument();
    expect(within(review).getByText("b".repeat(64))).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Review x__create_post" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Review" }));
    await user.click(within(await sheet("Review x__create_post")).getByRole("button", { name: "Approve change" }));
    expect(await notifications().findByText("x__create_post approved")).toBeInTheDocument();
    expect(requests("POST", `${SERVER_TOOLS}/:tool/approve`)[0]?.body).toEqual({ hash: changed.hash });
  });

  it("shows a change to a tool that was never approved", async () => {
    signedIn();
    serve([mcpServer()], [{ ...changed, long_description: false, approved_hash: null }, mcpTool({ name: "ping", title: null, description: "" })]);
    on("GET", `${SERVER_TOOLS}/:tool/change`, { tool: changed, approved: null, current: { name: "create_post", inputSchema: { type: "object" } } });
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByText("x__ping"));
    expect(screen.queryByRole("dialog", { name: /^Review/ })).not.toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Review" }));
    const review = await sheet("Review x__create_post");
    expect(await within(review).findByText("Input schema")).toBeInTheDocument();
    expect(within(review).queryByText("Description")).not.toBeInTheDocument();
    expect(within(review).getByText("—")).toBeInTheDocument();
  });

  it("keeps a changed tool off, and says when its change couldn't load", async () => {
    signedIn();
    serve([mcpServer()], [{ ...changed, long_description: false, approved_hash: null }]);
    on("GET", `${SERVER_TOOLS}/:tool/change`, apiError(404, "not_found", "No such tool, or you cannot see it."));
    on("POST", `${SERVER_TOOLS}/:tool/reject`, changed);
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByText("x__create_post"));
    const review = await sheet("Review x__create_post");
    expect(await within(review).findByText("This change couldn't load")).toBeInTheDocument();
    await user.click(within(review).getByRole("button", { name: "Keep off" }));
    expect(await notifications().findByText("x__create_post stays off")).toBeInTheDocument();
    expect(requests("POST", `${SERVER_TOOLS}/:tool/reject`)).toHaveLength(1);
  });

  it("checks a server for new tools", async () => {
    signedIn();
    serve([mcpServer()]);
    let found = { added: ["post_thread"], held: [], accepted: [], removed: 0, skipped: [], server: mcpServer() };
    on("POST", `${SERVER}/refresh`, () => found);
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Check for new tools" }));
    expect(await notifications().findByText("1 new and 0 changed tools found")).toBeInTheDocument();
    found = { ...found, added: [] };
    await user.click(within(s).getByRole("button", { name: "Server actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Check for new tools" }));
    expect(await notifications().findByText("No new or changed tools")).toBeInTheDocument();
  });

  it("renames a server, changes its handle and lets changed tools through", async () => {
    signedIn();
    serve([mcpServer()]);
    on("PATCH", SERVER, ({ body }: { body: Record<string, unknown> }) =>
      mcpServer({ ...(body as Partial<McpServerView>), name: (body.name as string) ?? "X" }),
    );
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Server actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit X" });
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save" }), "Give the server a name");
    await user.type(name, "X posts");
    const handle = within(dialog).getByRole("textbox", { name: "Handle" });
    await user.clear(handle);
    await user.type(handle, "X Posts");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Save" }), "Use 1 to 24 lowercase letters, digits and hyphens");
    await user.clear(handle);
    await user.type(handle, "x-posts");
    expect(within(dialog).getByText("Its tools are named x_posts__tool outside Kaeso.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("switch", { name: /Hold changed tools for review/ }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("X posts saved")).toBeInTheDocument();
    const sent = requests("PATCH", SERVER)[0];
    expect(sent?.body).toEqual({ name: "X posts", handle: "x-posts", change_policy: "accept" });
    expect(sent?.headers.get("if-match")).toBe('"3"');
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit X" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Server actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    let again = await screen.findByRole("dialog", { name: "Edit X" });
    await user.click(within(again).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit X" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Server actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Edit" }));
    again = await screen.findByRole("dialog", { name: "Edit X" });
    await user.type(within(again).getByRole("textbox", { name: "Name" }), "{Enter}");
    await waitFor(() => expect(requests("PATCH", SERVER)).toHaveLength(2));
    expect(requests("PATCH", SERVER)[1]?.body).toEqual({ change_policy: "hold" });
  });

  it("chooses the projects that use a server", async () => {
    signedIn();
    serve([mcpServer()]);
    on("PATCH", SERVER, ({ body }: { body: { projects: string[] } }) => mcpServer({ projects: body.projects }));
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Choose projects" }));
    let dialog = await screen.findByRole("dialog", { name: "Projects that use X" });
    await user.click(await within(dialog).findByRole("checkbox", { name: "Support triage" }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("X is in 2 projects")).toBeInTheDocument();
    expect(requests("PATCH", SERVER)[0]?.body).toEqual({ projects: [PROJECT, OTHER_PROJECT] });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Projects that use X" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Choose projects" }));
    dialog = await screen.findByRole("dialog", { name: "Projects that use X" });
    await user.click(await within(dialog).findByRole("checkbox", { name: "Checkout assistant" }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("No project uses X now")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Projects that use X" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Choose projects" }));
    dialog = await screen.findByRole("dialog", { name: "Projects that use X" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Projects that use X" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Choose projects" }));
    dialog = await screen.findByRole("dialog", { name: "Projects that use X" });
    within(dialog).getByRole("checkbox", { name: "Checkout assistant" }).focus();
    await user.keyboard("{Control>}{Enter}{/Control}");
    await waitFor(() => expect(requests("PATCH", SERVER)).toHaveLength(3));
  });

  it("deletes a server that projects use", async () => {
    signedIn();
    serve([mcpServer()]);
    on("DELETE", SERVER, noContent);
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Server actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const alert = await screen.findByRole("alertdialog", { name: "Delete X?" });
    expect(within(alert).getByText("1 project stops offering its tools, and its sign-ins are removed.")).toBeInTheDocument();
    await user.click(within(alert).getByRole("button", { name: "Delete server" }));
    expect(await notifications().findByText("X deleted")).toBeInTheDocument();
    expect(requests("DELETE", SERVER)[0]?.headers.get("if-match")).toBe('"3"');
    await waitFor(() => expect(window.location.search).toBe(""));
  });

  it("says when deleting an unused server leaves no project without tools", async () => {
    signedIn();
    serve([mcpServer({ projects: [] }), mcpServer({ id: "mcp_old", name: "Old", projects: ["proj_gone"] })]);
    on("DELETE", SERVER, apiError(412, "precondition_failed", "This MCP server was changed in the meantime; reload and try again."));
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    expect(await within(s).findByText("No project uses this server.")).toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Server actions" }));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    const alert = await screen.findByRole("alertdialog", { name: "Delete X?" });
    expect(within(alert).getByText("No project uses it; its sign-ins are removed.")).toBeInTheDocument();
    await user.click(within(alert).getByRole("button", { name: "Delete server" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await user.click(await row("Old"));
    expect(await within(await sheet("Old")).findByText("proj_gone")).toBeInTheDocument();
  });

  it("replaces the secret header of a server", async () => {
    signedIn();
    const github = mcpServer({
      id: "mcp_github",
      name: "GitHub",
      auth_method: "header",
      connection: mcpConnection({ kind: "header", header_name: "X-Api-Key", account: null, expires_at: null, last_used_at: "2026-10-15T11:59:00Z" }),
    });
    serve([github]);
    on("POST", `${SERVER}/connection`, { redirect_url: null, server: github });
    const { user } = await renderApp(`${PATH}?server=mcp_github`);
    const s = await sheet("GitHub");
    expect(await within(s).findByText("X-Api-Key: …a1b2")).toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Replace secret" }));
    const dialog = await screen.findByRole("dialog", { name: "Replace the secret of GitHub" });
    const header = within(dialog).getByRole("textbox", { name: "Header" });
    expect(header).toHaveValue("X-Api-Key");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Replace secret" }), "Paste the header's value");
    await user.clear(header);
    await hoverReason(user, within(dialog).getByRole("button", { name: "Replace secret" }), "Name the header");
    await user.type(header, "Authorization");
    await user.type(within(dialog).getByLabelText("Value"), "Bearer new-token");
    await user.click(within(dialog).getByRole("button", { name: "Replace secret" }));
    expect(await notifications().findByText("The secret of GitHub was replaced")).toBeInTheDocument();
    expect(requests("POST", `${SERVER}/connection`)[0]?.body).toEqual({ header: { name: "Authorization", value: "Bearer new-token" } });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Replace the secret of GitHub" })).not.toBeInTheDocument());
    await user.click(within(s).getByRole("button", { name: "Replace secret" }));
    const again = await screen.findByRole("dialog", { name: "Replace the secret of GitHub" });
    await user.click(within(again).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Replace the secret of GitHub" })).not.toBeInTheDocument());
  });

  it("asks for a secret header where a server has none yet", async () => {
    signedIn();
    serve([mcpServer({ id: "mcp_github", name: "GitHub", auth_method: "header", connection: null })]);
    on("POST", `${SERVER}/connection`, { redirect_url: null, server: mcpServer({ id: "mcp_github", name: "GitHub" }) });
    const { user } = await renderApp(`${PATH}?server=mcp_github`);
    const s = await sheet("GitHub");
    expect(await within(s).findByText("Not signed in")).toBeInTheDocument();
    await user.click(within(s).getByRole("button", { name: "Replace secret" }));
    const dialog = await screen.findByRole("dialog", { name: "Replace the secret of GitHub" });
    expect(within(dialog).getByRole("textbox", { name: "Header" })).toHaveValue("Authorization");
    await user.type(within(dialog).getByLabelText("Value"), "Bearer t{Enter}");
    expect(await notifications().findByText("The secret of GitHub was replaced")).toBeInTheDocument();
  });

  it("signs in to an OAuth server again in a new tab", async () => {
    signedIn();
    const tab = { opener: window as unknown, location: { href: "" }, close: vi.fn() };
    vi.spyOn(window, "open").mockReturnValue(tab as unknown as Window);
    const before = mcpServer();
    let current = before;
    serve([before]);
    on("GET", SERVER, () => current);
    on("POST", `${SERVER}/connection`, { redirect_url: "https://auth.x.example/authorize?state=s", server: before });
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Sign in again" }));
    expect(await within(s).findByText("Finish the sign-in in the new tab.")).toBeInTheDocument();
    expect(tab.location.href).toBe("https://auth.x.example/authorize?state=s");
    expect(requests("POST", `${SERVER}/connection`)[0]?.body).toEqual({ oauth: {} });
    current = mcpServer({ connection: mcpConnection({ updated_at: "2026-10-15T12:01:00Z" }) });
    await waitFor(() => expect(within(s).queryByText("Finish the sign-in in the new tab.")).not.toBeInTheDocument(), { timeout: 5000 });
  });

  it("closes the new tab when the sign-in can't start", async () => {
    signedIn();
    const tab = { opener: window as unknown, location: { href: "" }, close: vi.fn() };
    vi.spyOn(window, "open").mockReturnValue(tab as unknown as Window);
    serve([mcpServer()]);
    on("POST", `${SERVER}/connection`, apiError(502, "provider_error", "The server names no OAuth sign-in Kaeso can use."));
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Sign in again" }));
    await waitFor(() => expect(tab.close).toHaveBeenCalledTimes(1));
    on("POST", `${SERVER}/connection`, { redirect_url: null, server: mcpServer() });
    await user.click(within(s).getByRole("button", { name: "Sign in again" }));
    await waitFor(() => expect(tab.close).toHaveBeenCalledTimes(2));
  });

  it("links to the sign-in page when the browser blocks the new tab", async () => {
    signedIn();
    vi.spyOn(window, "open").mockReturnValue(null);
    serve([mcpServer({ connection: mcpConnection({ signed_out: true }), status: "signed_out" })]);
    on("POST", `${SERVER}/connection`, { redirect_url: "https://auth.x.example/authorize?state=s", server: mcpServer() });
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    expect(await within(s).findAllByText("Signed out")).not.toHaveLength(0);
    await user.click(within(s).getByRole("button", { name: "Sign in again" }));
    expect(await within(s).findByRole("link", { name: /Open the sign-in page/ })).toHaveAttribute("href", "https://auth.x.example/authorize?state=s");
  });

  it("explains servers whose tools wait for a sign-in", async () => {
    signedIn();
    serve(
      [
        mcpServer({ credential_mode: "per_user", connection: null, tools: { total: 0, changed: 0 } }),
        mcpServer({
          id: "mcp_docs",
          name: "Docs",
          auth_method: "none",
          connection: null,
          change_policy: "accept",
          server_info: null,
          protocol_version: null,
          last_checked_at: null,
          last_error: "Nothing answered.",
        }),
      ],
      [],
    );
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    let s = await sheet("X");
    expect(await within(s).findByText("Each person connects their own account under Account › Connections.")).toBeInTheDocument();
    expect(await within(s).findByText("Tools appear once someone connects an account.")).toBeInTheDocument();
    expect(within(s).queryByRole("button", { name: "Sign in again" })).not.toBeInTheDocument();
    await user.click(await row("Docs"));
    s = await sheet("Docs");
    expect(await within(s).findByText("Tools appear once Kaeso can sign in and list them.")).toBeInTheDocument();
    expect(within(s).getByText("Offered as they come")).toBeInTheDocument();
    expect(within(s).getAllByText("Nothing answered.")).not.toHaveLength(0);
  });

  it("names the right that changing a server needs, and says when its tools couldn't load", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "mcp_servers.manage"));
    serve([mcpServer()]);
    on("GET", SERVER_TOOLS, apiError(403, "permission_denied", "You can't see these tools."));
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await hoverReason(user, await within(s).findByRole("button", { name: "Server actions" }), /Manage MCP servers/);
    await hoverReason(user, within(s).getByRole("button", { name: "Sign in again" }), /Manage MCP servers/);
    expect(await within(s).findByText("No tools listed yet")).toBeInTheDocument();
    expect(within(s).queryByText("1 tool waits for review")).not.toBeInTheDocument();
  });

  it("names the right that reviewing a change needs", async () => {
    withRights(ALL_ORG_RIGHTS.filter((r) => r !== "mcp_servers.manage"));
    serve([mcpServer()]);
    on("GET", `${SERVER_TOOLS}/:tool/change`, { tool: changed, approved: null, current: { name: "create_post" } });
    const { user } = await renderApp(`${PATH}?server=mcp_x`);
    const s = await sheet("X");
    await user.click(await within(s).findByRole("button", { name: "Review" }));
    const review = await sheet("Review x__create_post");
    await hoverReason(user, within(review).getByRole("button", { name: "Approve change" }), /Manage MCP servers/);
  });

  it("says when a server is gone or couldn't load", async () => {
    signedIn();
    serve([mcpServer()]);
    on("GET", SERVER, apiError(404, "not_found", "No such MCP server, or you cannot see it."));
    const { user } = await renderApp(`${PATH}?server=mcp_gone`);
    expect(await screen.findByText("This server doesn't exist or was deleted.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back to servers" }));
    await waitFor(() => expect(window.location.search).toBe(""));
    on("GET", SERVER, apiError(403, "permission_denied", "You can't see this server."));
    await user.click(await row("X"));
    expect(await screen.findByText("Couldn't load this server.")).toBeInTheDocument();
  });
});
