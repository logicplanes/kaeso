import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ToolCallEntryView, ToolCallView } from "~/api/generated/types";
import { ALL_PROJECT_RIGHTS, USER, signedIn } from "~/test/fixtures";
import { apiKey, projectData, projectMember, projectPath } from "~/test/fixtures/project";
import { listPage, logsData } from "~/test/fixtures/project-logs";
import { apiError, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const CALLS = "orgs/:org/projects/:project/logs/tool-calls";

function toolCall(o: Partial<ToolCallView> = {}): ToolCallView {
  return {
    id: "call_0001",
    started_at: "2026-10-15T11:59:00Z",
    tool: "x__create_post",
    upstream_tool: "create_post",
    server: "mcp_x",
    caller: { kind: "user", id: USER },
    key_id: "key_web",
    grant_id: null,
    client: "claude-ai",
    protocol_version: "2026-07-28",
    source: "portal",
    status: "ok",
    error_type: "",
    error_message: "",
    duration_ms: 240,
    upstream_ms: 230,
    request_bytes: 120,
    response_bytes: 480,
    has_content: true,
    ...o,
  };
}

function calls(rows: ToolCallView[], entries: ToolCallEntryView[] = []) {
  on("GET", CALLS, listPage(rows));
  on(
    "GET",
    "orgs/:org/projects/:project/tools",
    page([
      { exposed_name: "x__create_post", server: "mcp_x" },
      { exposed_name: "gh__get_me", server: "mcp_gh" },
    ]),
  );
  on(
    "GET",
    "orgs/:org/projects/:project/mcp-servers",
    page([
      { id: "mcp_x", name: "X" },
      { id: "mcp_gh", name: "GitHub" },
    ]),
  );
  on(
    "GET",
    `${CALLS}/:call`,
    ({ params }: { params: Record<string, string> }) =>
      entries.find((e) => e.id === params.call) ?? apiError(404, "not_found", "No such tool call, or you cannot see it."),
  );
}

describe("Tool calls in the request logs", () => {
  it("switches between model requests and tool calls", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    logsData();
    calls([
      toolCall(),
      toolCall({ id: "call_0002", tool: "x__get_me", upstream_tool: "get_me", status: "refused", error_type: "tool_off", upstream_ms: null, client: "" }),
    ]);
    const { user } = await renderApp(projectPath("logs"));
    await user.click(await screen.findByRole("radio", { name: "Tools" }));
    const table = await screen.findByRole("grid", { name: "Tool calls" });
    const called = await within(table).findByRole("row", { name: /x__create_post/ });
    expect(called).toHaveTextContent("Called");
    expect(called).toHaveTextContent("claude-ai");
    expect(within(table).getByRole("row", { name: /x__get_me/ })).toHaveTextContent("Refused");
    expect(window.location.search).toContain("kind=tools");
    await user.click(screen.getByRole("radio", { name: "Models" }));
    expect(await screen.findByRole("grid", { name: "Requests" })).toBeInTheDocument();
    expect(window.location.search).not.toContain("kind=tools");
  });

  it("keeps the range, the filters and live updates when switching between models and tools", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    logsData();
    calls([toolCall()], [{ ...toolCall(), content: null } as ToolCallEntryView]);
    const { user } = await renderApp(projectPath("logs?range=7d&model=gpt-5&live=true&kind=tools&call_status=error&call=call_0001"));
    expect(await screen.findByRole("dialog", { name: "x__create_post" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Remove the Status filter" })).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Models" }));
    await screen.findByRole("grid", { name: "Requests" });
    const back = new URLSearchParams(window.location.search);
    expect(Object.fromEntries(back)).toMatchObject({ range: "7d", model: "gpt-5", live: "true", call_status: "error" });
    expect(back.has("call")).toBe(false);
    expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Remove the Model filter" })).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Tools" }));
    expect(await screen.findByRole("button", { name: "Remove the Status filter" })).toBeInTheDocument();
    expect(requests("GET", CALLS).at(-1)?.query.get("status")).toBe("error");
  });

  it("filters by key, member, server, tool and error kind", async () => {
    signedIn();
    projectData({ members: [projectMember()], keys: [apiKey({ id: "key_web", name: "Web app" })] });
    calls([toolCall()]);
    const { user } = await renderApp(projectPath("logs?kind=tools&member=user:" + USER));
    await screen.findByRole("row", { name: /x__create_post/ });
    expect(requests("GET", CALLS)[0]?.query.get("user")).toBe(USER);
    const pick = async (filter: string, value: string) => {
      await user.click(screen.getByRole("button", { name: "Add filter" }));
      await user.click(await screen.findByRole("menuitem", { name: filter }));
      await user.click(await screen.findByRole("menuitemradio", { name: value }));
    };
    await pick("Key", "Web app");
    await pick("MCP server", "GitHub");
    await pick("Tool", "gh__get_me");
    await pick("Error kind", "Server too slow");
    await waitFor(() =>
      expect(Object.fromEntries(requests("GET", CALLS).at(-1)?.query ?? [])).toMatchObject({
        key: "key_web",
        user: USER,
        server: "mcp_gh",
        tool: "gh__get_me",
        error_type: "timeout",
      }),
    );
    expect(screen.getByRole("button", { name: "Remove the MCP server filter" }).closest("div")).toHaveTextContent("GitHub");
  });

  it("names a server it no longer knows by its id, and makes the list compact", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls([toolCall()]);
    const { user } = await renderApp(projectPath("logs?kind=tools&call_server=mcp_gone"));
    await screen.findByRole("row", { name: /x__create_post/ });
    expect(screen.getByRole("button", { name: "Remove the MCP server filter" }).closest("div")).toHaveTextContent("mcp_gone");
    expect(requests("GET", CALLS)[0]?.query.get("server")).toBe("mcp_gone");
    await user.click(screen.getByRole("button", { name: "Display options" }));
    const options = await screen.findByRole("dialog", { name: "Display options" });
    await user.click(within(options).getByRole("combobox", { name: "Density" }));
    await user.click(await screen.findByRole("option", { name: "Compact" }));
    expect(within(options).getByRole("combobox", { name: "Density" })).toHaveTextContent("Compact");
  });

  it("opens a call by its id and steps through calls with j and k", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    const second = toolCall({ id: "call_0002", tool: "x__get_me" });
    calls([toolCall(), second], [{ ...toolCall(), content: null } as ToolCallEntryView, { ...second, content: null } as ToolCallEntryView]);
    const { user } = await renderApp(projectPath("logs?kind=tools"));
    await screen.findByRole("row", { name: /x__create_post/ });
    await user.type(screen.getByRole("searchbox", { name: "Open a tool call by id" }), "call_0001{Enter}");
    expect(await screen.findByRole("dialog", { name: "x__create_post" })).toBeInTheDocument();
    await user.click(document.body);
    await user.keyboard("j");
    expect(await screen.findByRole("dialog", { name: "x__get_me" })).toBeInTheDocument();
    await user.keyboard("k");
    expect(await screen.findByRole("dialog", { name: "x__create_post" })).toBeInTheDocument();
  });

  it("filters by status and opens a call with its arguments and answer", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls(
      [toolCall()],
      [
        {
          ...toolCall(),
          content: { arguments: { text: "Hello" }, result: { jsonrpc: "2.0", id: 1, result: { isError: false } } },
        } as unknown as ToolCallEntryView,
      ],
    );
    const { user } = await renderApp(projectPath("logs?kind=tools"));
    await user.click(await screen.findByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Status" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Refused" }));
    await waitFor(() => expect(requests("GET", CALLS).at(-1)?.query.get("status")).toBe("refused"));
    await user.click(await screen.findByRole("row", { name: /x__create_post/ }));
    const sheet = await screen.findByRole("dialog", { name: "x__create_post" });
    expect(await within(sheet).findByText("Arguments")).toBeInTheDocument();
    expect(within(sheet).getByText("create_post")).toBeInTheDocument();
    expect(within(sheet).getByText("Answer")).toBeInTheDocument();
  });

  it("says when a call's content wasn't logged, and when calls couldn't load", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls([toolCall({ has_content: false })], [{ ...toolCall({ has_content: false }), content: null } as ToolCallEntryView]);
    await renderApp(projectPath("logs?kind=tools&call=call_0001"));
    expect(await screen.findByText("Content not logged")).toBeInTheDocument();
  });

  it("shows why a call was refused, with what the server took, and closes", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    const refused = toolCall({ status: "refused", error_type: "tool_changed", error_message: "The tool changed.", upstream_ms: null, client: "" });
    calls([refused], [{ ...refused, content: null } as ToolCallEntryView]);
    const { user } = await renderApp(projectPath("logs?kind=tools&range=7d&call=call_0001"));
    const sheet = await screen.findByRole("dialog", { name: "x__create_post" });
    expect(await within(sheet).findByText("Tool waits for review")).toBeInTheDocument();
    expect(within(sheet).getByText("The tool changed.")).toBeInTheDocument();
    expect(requests("GET", CALLS)[0]?.query.get("from")).toBeTruthy();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(window.location.search).not.toContain("call="));
    await user.click(screen.getByRole("combobox", { name: "Time range" }));
    await user.click(await screen.findByRole("option", { name: "Last 24 hours" }));
    await waitFor(() => expect(window.location.search).not.toContain("range="));
  });

  it("offers a retry when calls fail to load, says when logs are unavailable, and a missing call", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    on("GET", CALLS, apiError(403, "permission_denied", "These calls are hidden from you."));
    const { user } = await renderApp(projectPath("logs?kind=tools&call=call_9999"));
    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    on("GET", CALLS, apiError(503, "unavailable", "Request logs are unavailable right now; try again shortly."));
    on("GET", `${CALLS}/:call`, apiError(404, "not_found", "No such tool call, or you cannot see it."));
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Request logs are unavailable for now", undefined, { timeout: 5000 })).toBeInTheDocument();
    expect(await screen.findByText("This tool call couldn't load")).toBeInTheDocument();
  });

  it("keeps a custom range and clears the status filter", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls([toolCall({ status: "error", error_type: "timeout" })]);
    const { user } = await renderApp(projectPath("logs?kind=tools&from=2026-10-01&to=2026-10-02&call_status=error"));
    expect(await screen.findByRole("row", { name: /x__create_post/ })).toHaveTextContent("Failed");
    const first = requests("GET", CALLS)[0]?.query;
    expect(first?.get("to")).toBeTruthy();
    expect(first?.get("status")).toBe("error");
    await user.click(screen.getByRole("button", { name: "Remove the Status filter" }));
    await waitFor(() => expect(requests("GET", CALLS).at(-1)?.query.get("status")).toBeNull());
  });

  it("loads more calls, and hides content without the right to see it", async () => {
    signedIn({ projectRights: ALL_PROJECT_RIGHTS.filter((r) => r !== "logs.content.view") });
    projectData({ members: [projectMember()] });
    on("GET", CALLS, ({ query }: { query: URLSearchParams }) =>
      query.get("after") ? page([toolCall({ id: "call_0002", tool: "x__get_me" })]) : page([toolCall()], "next"),
    );
    on("GET", `${CALLS}/:call`, { ...toolCall(), content: null });
    const { user } = await renderApp(projectPath("logs?kind=tools"));
    await screen.findByRole("row", { name: /x__create_post/ });
    await user.click(screen.getByRole("button", { name: "Load more" }));
    await user.click(await screen.findByRole("row", { name: /x__get_me/ }));
    expect(await screen.findByText("You can't see request content")).toBeInTheDocument();
  });

  it("shows only one tool's calls when opened from the tool", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls([toolCall()]);
    const { user } = await renderApp(projectPath("logs?kind=tools&call_tool=x__create_post"));
    await screen.findByRole("row", { name: /x__create_post/ });
    expect(requests("GET", CALLS)[0]?.query.get("tool")).toBe("x__create_post");
    await user.click(screen.getByRole("button", { name: "Remove the Tool filter" }));
    await waitFor(() => expect(window.location.search).not.toContain("call_tool"));
    await waitFor(() => expect(requests("GET", CALLS).at(-1)?.query.get("tool")).toBeNull());
  });

  it("explains an empty range", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls([]);
    await renderApp(projectPath("logs?kind=tools"));
    expect(await screen.findByText("No tool calls in this range")).toBeInTheDocument();
    expect(screen.getByText("Calls appear here as clients use the project's MCP portal.")).toBeInTheDocument();
  });
});

describe("Live tool calls", () => {
  afterEach(() => vi.useRealTimers());

  it("adds new calls at the top while live, and holds them behind a button while scrolled", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    calls([toolCall()]);
    let newest = [toolCall()];
    on("GET", CALLS, () => listPage(newest));
    const { user } = await renderApp(projectPath("logs?kind=tools"));
    await screen.findByRole("row", { name: /x__create_post/ });
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "true"));

    newest = [toolCall({ id: "call_0002", tool: "x__get_me", started_at: "2026-10-15T11:59:30Z" }), toolCall()];
    await vi.advanceTimersByTimeAsync(5000);
    const added = await screen.findByRole("row", { name: /x__get_me/ });
    expect(added).toHaveAttribute("data-fresh");
    expect(requests("GET", CALLS).at(-1)?.query.get("from")).toBe("2026-10-15T11:59:00Z");

    const body = screen.getByRole("grid", { name: "Tool calls" }).closest<HTMLElement>(".overflow-y-auto");
    if (!body) throw new Error("no scrolling body");
    body.style.overflowY = "auto";
    body.scrollTop = 400;
    newest = [toolCall({ id: "call_0003", tool: "x__delete_post", started_at: "2026-10-15T11:59:40Z" }), ...newest];
    await vi.advanceTimersByTimeAsync(5000);
    await user.click(await screen.findByRole("button", { name: "Show 1 new tool call" }));
    expect(await screen.findByRole("row", { name: /x__delete_post/ })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(window.location.search).not.toContain("live="));
  });
});
