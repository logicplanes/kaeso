import { act, screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { McpServerView } from "~/api/generated/types";
import { ORG, OTHER_PROJECT, PROJECT, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { SERVER, SERVERS, SERVER_TOOLS, mcpConnection, mcpServer, mcpTool, obj, probeView } from "~/test/fixtures/mcp";
import { apiError, on, page, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { AddServerDialog } from "./mcp-server-dialog";

const PROBE = `${SERVERS}/probe`;
const PROJECT_TOOLS = "orgs/:org/projects/:project/tools";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function newTab() {
  const tab = { opener: window as unknown, location: { href: "" }, close: vi.fn() };
  vi.spyOn(window, "open").mockReturnValue(tab as unknown as Window);
  return tab;
}

async function open() {
  signedIn();
  const onClose = vi.fn();
  const onAdded = vi.fn();
  const view = await renderUi(<AddServerDialog org={ORG} open onClose={onClose} onAdded={onAdded} />);
  const dialog = await screen.findByRole("dialog", { name: "Add MCP server" });
  return { ...view, dialog, onClose, onAdded };
}

async function address(user: UserEvent, dialog: HTMLElement, url = "https://mcp.x.example/mcp") {
  await user.type(within(dialog).getByRole("textbox", { name: "Address" }), url);
  await user.click(within(dialog).getByRole("button", { name: "Check address" }));
}

const next = (user: UserEvent, dialog: HTMLElement) => user.click(within(dialog).getByRole("button", { name: "Continue" }));

describe("Add MCP server", () => {
  it("adds a server without sign-in and turns its read-only tools on in the chosen projects", async () => {
    let answer: (v: unknown) => void = () => undefined;
    on("POST", PROBE, () => new Promise((resolve) => (answer = resolve)));
    on("POST", SERVERS, ({ body }: { body: Partial<McpServerView> }) => mcpServer({ ...body, id: "mcp_new", name: "mock-x", connection: null }));
    on("GET", SERVER_TOOLS, page(["create_post", "get_me", "search"].map((name) => mcpTool({ name }))));
    on("PATCH", PROJECT_TOOLS, page([]));
    const { user, dialog, onClose, onAdded } = await open();

    await hoverReason(user, within(dialog).getByRole("button", { name: "Check address" }), "Enter the server's address");
    await address(user, dialog);
    expect(await within(dialog).findByText("Checking the address…")).toBeInTheDocument();
    answer(probeView());
    expect(await within(dialog).findByText("mock-x 0.1.0")).toBeInTheDocument();
    expect(within(dialog).getByText("3 tools")).toBeInTheDocument();
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("mock-x");
    const handle = within(dialog).getByRole("textbox", { name: "Handle" });
    expect(handle).toHaveValue("mock-x");
    await user.clear(handle);
    await user.type(handle, "x");
    await next(user, dialog);

    expect(await within(dialog).findByRole("heading", { name: "How does Kaeso sign in?" })).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: /No sign-in/ })).toBeChecked();
    expect(within(dialog).queryByRole("radiogroup", { name: "Whose account" })).not.toBeInTheDocument();
    await next(user, dialog);

    expect(await within(dialog).findByRole("heading", { name: "Which tools should projects offer?" })).toBeInTheDocument();
    const tools = within(dialog).getByRole("grid", { name: "Tools" });
    expect(within(tools).getByText("Destructive")).toBeInTheDocument();
    expect(within(tools).getByRole("checkbox", { name: "Select create_post" })).not.toBeChecked();
    expect(within(tools).getByRole("checkbox", { name: "Select get_me" })).toBeChecked();
    await user.click(within(tools).getByRole("checkbox", { name: "Select search" }));
    await next(user, dialog);

    expect(await within(dialog).findByText("Their portals offer the tools you chose.")).toBeInTheDocument();
    await user.click(await within(dialog).findByRole("checkbox", { name: "Checkout assistant" }));
    await user.click(within(dialog).getByRole("button", { name: "Add server" }));

    expect(await notifications().findByText("mock-x added")).toBeInTheDocument();
    expect(requests("POST", SERVERS)[0]?.body).toEqual({
      handle: "x",
      name: "mock-x",
      url: "https://mcp.x.example/mcp",
      auth_method: "none",
      credential_mode: "shared",
      change_policy: "hold",
      projects: [PROJECT],
    });
    expect(requests("PATCH", PROJECT_TOOLS).map((r) => [r.params.project, r.body])).toEqual([[PROJECT, { tools: [{ id: "tool_get_me", enabled: true }] }]]);
    expect(onAdded).toHaveBeenCalledWith(expect.objectContaining({ id: "mcp_new" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("refuses an address Kaeso can't use and checks a changed one again", async () => {
    on("POST", PROBE, probeView({ problem: "unreachable", sign_in: null, tools: null, server_info: null, protocol_version: null }));
    const { user, dialog } = await open();
    await address(user, dialog);
    expect(await within(dialog).findByText("Nothing answered at this address; check it and try again.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Kaeso can't use this address");
    await user.type(within(dialog).getByRole("textbox", { name: "Address" }), "2");
    on("POST", PROBE, probeView({ url: "https://mcp.x.example/mcp2", server_info: obj({ name: "other" }), handle: "other" }));
    await user.click(within(dialog).getByRole("button", { name: "Check address" }));
    expect(await within(dialog).findByText("other")).toBeInTheDocument();
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("x.example");
    expect(within(dialog).getByRole("textbox", { name: "Handle" })).toHaveValue("mock-x");
  });

  it("keeps you on the address when the check is refused", async () => {
    on("POST", PROBE, apiError(400, "invalid_request", "`url` points to an address this install does not allow.", { param: "url" }));
    const { user, dialog } = await open();
    await address(user, dialog, "http://10.0.0.1/mcp");
    await waitFor(() => expect(requests("POST", PROBE)).toHaveLength(1));
    expect(within(dialog).getByRole("button", { name: "Check address" })).toBeInTheDocument();
  });

  it("asks for a name and a valid handle", async () => {
    on("POST", PROBE, probeView({ server_info: obj({ title: "X", version: 3 }), sign_in: null, protocol_version: null }));
    const { user, dialog } = await open();
    await address(user, dialog);
    const name = await within(dialog).findByRole("textbox", { name: "Name" });
    expect(name).toHaveValue("X");
    expect(within(dialog).getByText("X")).toBeInTheDocument();
    expect(within(dialog).queryByText("MCP version")).not.toBeInTheDocument();
    await user.clear(name);
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Give the server a name");
    await user.type(name, "X");
    const handle = within(dialog).getByRole("textbox", { name: "Handle" });
    await user.clear(handle);
    expect(within(dialog).getByText("Its tools are named x__tool outside Kaeso.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Use 1 to 24 lowercase letters, digits and hyphens");
  });

  it("adds a server with a shared secret header after the server accepts it", async () => {
    let refused = true;
    on("POST", PROBE, ({ body }: { body: { header?: unknown } }) =>
      body.header
        ? refused
          ? probeView({ problem: "header_refused", tools: null })
          : probeView({ sign_in: { method: "header", issuer: null, client_metadata_documents: false, registration: false, scopes: [] }, tools: null })
        : probeView({
            url: "https://mcp.github.example/mcp",
            sign_in: { method: "header", issuer: null, client_metadata_documents: false, registration: false, scopes: [] },
            tools: null,
            server_info: null,
          }),
    );
    on("POST", SERVERS, mcpServer({ id: "mcp_github", name: "github.example", auth_method: "header", connection: null }));
    on("POST", `${SERVER}/connection`, { redirect_url: null, server: mcpServer({ id: "mcp_github", name: "github.example", auth_method: "header" }) });
    const { user, dialog } = await open();
    await address(user, dialog, "https://mcp.github.example/mcp");
    expect(await within(dialog).findByText("Listed after the sign-in")).toBeInTheDocument();
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("github.example");
    await next(user, dialog);

    expect(await within(dialog).findByRole("radio", { name: /Secret header/ })).toBeChecked();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Paste the header's value");
    await user.clear(within(dialog).getByRole("textbox", { name: "Header" }));
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Name the header");
    await user.type(within(dialog).getByRole("textbox", { name: "Header" }), "Authorization");
    await user.type(within(dialog).getByLabelText("Value"), "Bearer t-1");
    await next(user, dialog);
    expect(await within(dialog).findByText("The server refused the header; check its name and value.")).toBeInTheDocument();
    refused = false;
    await next(user, dialog);

    expect(await within(dialog).findByRole("heading", { name: "Which tools should projects offer?" })).toBeInTheDocument();
    expect(within(dialog).getByText("The server lists no tools.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("switch", { name: /Hold changed tools for review/ }));
    await next(user, dialog);
    await user.click(within(dialog).getByRole("button", { name: "Add server" }));

    expect(await notifications().findByText("github.example added")).toBeInTheDocument();
    expect(requests("POST", PROBE).at(-1)?.body).toEqual({ url: "https://mcp.github.example/mcp", header: { name: "Authorization", value: "Bearer t-1" } });
    expect(requests("POST", SERVERS)[0]?.body).toMatchObject({ auth_method: "header", credential_mode: "shared", change_policy: "accept", projects: [] });
    expect(requests("POST", `${SERVER}/connection`)[0]?.body).toEqual({ header: { name: "Authorization", value: "Bearer t-1" } });
    expect(requests("PATCH", PROJECT_TOOLS)).toHaveLength(0);
  });

  it("keeps you on the sign-in when checking the header fails", async () => {
    on("POST", PROBE, ({ body }: { body: { header?: unknown } }) =>
      body.header
        ? apiError(400, "invalid_request", "The header name must be a valid HTTP header name.", { param: "header.name" })
        : probeView({ sign_in: { method: "header", issuer: null, client_metadata_documents: false, registration: false, scopes: [] }, tools: null }),
    );
    const { user, dialog } = await open();
    await address(user, dialog);
    await within(dialog).findByText("Listed after the sign-in");
    await next(user, dialog);
    await user.type(await within(dialog).findByLabelText("Value"), "Bearer t-1");
    await next(user, dialog);
    await waitFor(() => expect(requests("POST", PROBE)).toHaveLength(2));
    expect(within(dialog).getByRole("heading", { name: "How does Kaeso sign in?" })).toBeInTheDocument();
  });

  it("signs in with OAuth in a new tab, waits for the tools and adds the server", async () => {
    const tab = newTab();
    on(
      "POST",
      PROBE,
      probeView({
        sign_in: { method: "oauth", issuer: "https://auth.x.example", client_metadata_documents: true, registration: false, scopes: [] },
        tools: null,
      }),
    );
    const made = mcpServer({ id: "mcp_new", name: "mock-x", handle: "mock-x", connection: null, status: "unchecked", projects: [] });
    on("POST", SERVERS, made);
    on("POST", `${SERVER}/connection`, { redirect_url: "https://auth.x.example/authorize?state=s", server: made });
    let checks = 0;
    on("GET", SERVER, () => {
      checks += 1;
      return mcpServer({ id: "mcp_new", name: "mock-x", status: checks < 3 ? "unchecked" : "ok" });
    });
    on(
      "GET",
      SERVER_TOOLS,
      page([mcpTool({ name: "create_post", annotations: obj({ destructiveHint: true }) }), mcpTool(), mcpTool({ name: "old", state: "removed" })]),
    );
    on("PATCH", SERVER, mcpServer({ id: "mcp_new", name: "mock-x", projects: [PROJECT, OTHER_PROJECT] }));
    on("PATCH", PROJECT_TOOLS, page([]));
    const { user, dialog } = await open();
    await address(user, dialog);
    await within(dialog).findByText("Listed after the sign-in");
    await next(user, dialog);
    expect(await within(dialog).findByRole("radio", { name: /OAuth/ })).toBeChecked();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Sign in first");
    await user.click(within(dialog).getByRole("button", { name: "Sign in to mock-x" }));

    expect(await within(dialog).findByText("Connected as @kaesoapp", undefined, { timeout: 5000 })).toBeInTheDocument();
    expect(tab.location.href).toBe("https://auth.x.example/authorize?state=s");
    expect(requests("POST", SERVERS)[0]?.body).toMatchObject({ auth_method: "oauth", credential_mode: "shared", projects: [] });
    await next(user, dialog);
    const tools = await within(dialog).findByRole("grid", { name: "Tools" }, { timeout: 6000 });
    expect(within(tools).queryByText("old")).not.toBeInTheDocument();
    await next(user, dialog);
    await user.click(await within(dialog).findByRole("checkbox", { name: "Checkout assistant" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "Support triage" }));
    await user.click(within(dialog).getByRole("button", { name: "Add server" }));

    expect(await notifications().findByText("mock-x added")).toBeInTheDocument();
    expect(requests("POST", SERVERS)).toHaveLength(1);
    expect(requests("PATCH", SERVER)[0]?.body).toEqual({ change_policy: "hold", projects: [PROJECT, OTHER_PROJECT] });
    expect(requests("PATCH", PROJECT_TOOLS).map((r) => r.params.project)).toEqual([PROJECT, OTHER_PROJECT]);
  });

  it("renames a server created during the sign-in when it's added", async () => {
    newTab();
    on(
      "POST",
      PROBE,
      probeView({
        sign_in: { method: "oauth", issuer: "https://auth.x.example", client_metadata_documents: false, registration: false, scopes: [] },
        tools: null,
      }),
    );
    const made = mcpServer({ id: "mcp_new", name: "mock-x", handle: "mock-x", connection: null, projects: [] });
    on("POST", SERVERS, made);
    on("POST", `${SERVER}/connection`, { redirect_url: "https://auth.x.example/authorize?state=s", server: made });
    on("GET", SERVER, mcpServer({ id: "mcp_new", name: "mock-x", handle: "mock-x", connection: mcpConnection({ account: null }) }));
    on("GET", SERVER_TOOLS, page([]));
    on("PATCH", SERVER, mcpServer({ id: "mcp_new", name: "X" }));
    const { user, dialog } = await open();
    await address(user, dialog);
    await within(dialog).findByText("Listed after the sign-in");
    await next(user, dialog);
    await user.type(await within(dialog).findByRole("textbox", { name: "Client id" }), "kaeso-public");
    await user.click(within(dialog).getByRole("button", { name: "Sign in to mock-x" }));
    expect(await within(dialog).findByText("Connected")).toBeInTheDocument();
    expect(requests("POST", SERVERS)[0]?.body).toMatchObject({ oauth_client: { client_id: "kaeso-public" } });
    await user.click(within(dialog).getByRole("button", { name: "Back" }));
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "X");
    const handle = within(dialog).getByRole("textbox", { name: "Handle" });
    await user.clear(handle);
    await user.type(handle, "x");
    await next(user, dialog);
    await next(user, dialog);
    expect(await within(dialog).findByText("The server lists no tools.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("switch", { name: /Hold changed tools for review/ }));
    await next(user, dialog);
    await user.click(within(dialog).getByRole("button", { name: "Add server" }));
    expect(await notifications().findByText("X added")).toBeInTheDocument();
    expect(requests("PATCH", SERVER)[0]?.body).toEqual({ name: "X", handle: "x", change_policy: "accept", projects: [] });
  });

  it("takes a client registered by hand and links to the sign-in when the tab is blocked, until it expires", async () => {
    vi.spyOn(window, "open").mockReturnValue(null);
    on(
      "POST",
      PROBE,
      probeView({
        sign_in: { method: "oauth", issuer: "https://auth.x.example", client_metadata_documents: false, registration: false, scopes: [] },
        tools: null,
      }),
    );
    const made = mcpServer({ id: "mcp_new", name: "mock-x", connection: null });
    on("POST", SERVERS, made);
    on("POST", `${SERVER}/connection`, { redirect_url: "https://auth.x.example/authorize?state=s", server: made });
    on("GET", SERVER, made);
    const { user, dialog } = await open();
    await address(user, dialog);
    await within(dialog).findByText("Listed after the sign-in");
    await next(user, dialog);
    expect(await within(dialog).findByText("This server needs a client registered by hand at its sign-in.")).toBeInTheDocument();
    await hoverReason(user, within(dialog).getByRole("button", { name: "Continue" }), "Paste the client id");
    await hoverReason(user, within(dialog).getByRole("button", { name: "Sign in to mock-x" }), "Paste the client id");
    await user.type(within(dialog).getByRole("textbox", { name: "Client id" }), "kaeso-client");
    await user.type(within(dialog).getByLabelText("Client secret"), "s3cret");
    vi.useFakeTimers({ now: Date.now(), toFake: ["setTimeout", "clearTimeout", "Date"] });
    await act(async () => {
      within(dialog).getByRole("button", { name: "Sign in to mock-x" }).click();
      for (let i = 0; i < 10; i++) await vi.advanceTimersByTimeAsync(0);
    });
    expect(within(dialog).getByRole("link", { name: /Open the sign-in page/ })).toHaveAttribute("href", "https://auth.x.example/authorize?state=s");
    expect(requests("POST", SERVERS)[0]?.body).toMatchObject({ oauth_client: { client_id: "kaeso-client", client_secret: "s3cret" } });
    expect(within(dialog).getByText("Waiting for the sign-in in the new tab…")).toBeInTheDocument();
    await act(() => vi.advanceTimersByTimeAsync(10 * 60_000));
    expect(within(dialog).getByText("The sign-in took too long; start it again.")).toBeInTheDocument();
    expect(within(dialog).queryByText("Waiting for the sign-in in the new tab…")).not.toBeInTheDocument();
    await act(async () => {
      within(dialog).getByRole("button", { name: "Sign in to mock-x" }).click();
      for (let i = 0; i < 10; i++) await vi.advanceTimersByTimeAsync(0);
    });
    expect(requests("POST", SERVERS)).toHaveLength(1);
    expect(requests("POST", `${SERVER}/connection`)).toHaveLength(2);
  });

  it("adds a server each person signs in to, without choosing tools", async () => {
    on(
      "POST",
      PROBE,
      probeView({
        sign_in: { method: "oauth", issuer: "https://auth.x.example", client_metadata_documents: false, registration: false, scopes: [] },
        tools: null,
      }),
    );
    on("POST", SERVERS, mcpServer({ id: "mcp_new", name: "mock-x", credential_mode: "per_user", connection: null }));
    const { user, dialog } = await open();
    await address(user, dialog);
    await within(dialog).findByText("Listed after the sign-in");
    await next(user, dialog);
    await user.click(await within(dialog).findByRole("radio", { name: "Each person signs in" }));
    expect(within(dialog).getByText("Each person connects their own account under Account › Connections.")).toBeInTheDocument();
    await next(user, dialog);
    expect(await within(dialog).findByText("Each project chooses its tools once the server lists them.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Add server" }));
    expect(await notifications().findByText("mock-x added")).toBeInTheDocument();
    expect(requests("POST", SERVERS)[0]?.body).toMatchObject({ auth_method: "oauth", credential_mode: "per_user" });
    expect(requests("POST", SERVERS)[0]?.body).not.toHaveProperty("oauth_client");
  });

  it("suggests a secret header for a server whose OAuth Kaeso can't do", async () => {
    on("POST", PROBE, probeView({ problem: "sign_in", sign_in: null, tools: null }));
    const { user, dialog } = await open();
    await address(user, dialog);
    expect(await within(dialog).findByText("The server asks for an OAuth sign-in Kaeso can't do; try a secret header.")).toBeInTheDocument();
    await next(user, dialog);
    expect(await within(dialog).findByRole("radio", { name: /Secret header/ })).toBeChecked();
    await user.click(within(dialog).getByRole("radio", { name: "Each person signs in" }));
    expect(within(dialog).queryByLabelText("Value")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: "Shared" }));
    expect(within(dialog).getByLabelText("Value")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: /No sign-in/ }));
    await user.click(within(dialog).getByRole("radio", { name: /OAuth/ }));
    expect(within(dialog).queryByText("This server needs a client registered by hand at its sign-in.")).not.toBeInTheDocument();
  });
});
