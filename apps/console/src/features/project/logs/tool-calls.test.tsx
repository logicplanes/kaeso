import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ToolCallEntryView, ToolCallView } from "~/api/generated/types";
import { ALL_PROJECT_RIGHTS, USER, signedIn } from "~/test/fixtures";
import { projectData, projectMember, projectPath } from "~/test/fixtures/project";
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
    await user.click(await screen.findByRole("combobox", { name: "Status" }));
    await user.click(await screen.findByRole("option", { name: "Refused" }));
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
    await user.click(screen.getByRole("combobox", { name: "Status" }));
    await user.click(await screen.findByRole("option", { name: "Every status" }));
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
