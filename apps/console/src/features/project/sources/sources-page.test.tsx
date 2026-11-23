import { act, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { externalSource, projectData, projectPath, serviceAccount } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { WATCH_MS } from "./connection";

const SOURCES = "orgs/:org/projects/:project/sources";
const SOURCE = `${SOURCES}/:source`;
const KEY = "sk-kso-src-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789AbCdEfGhIjKl";
const claude = externalSource({
  activity: [
    { day: "2026-10-14", requests: 12, tool_calls: 3, refused: 0, duplicates: 1 },
    { day: "2026-10-15", requests: 5, tool_calls: 0, refused: 2, duplicates: 0 },
  ],
  first_event_at: "2026-10-14T09:00:00Z",
  last_event_at: "2026-10-15T11:58:00Z",
});
const proxy = externalSource({ id: "src_proxy", kind: "litellm", name: "LiteLLM proxy", status: "waiting", user_id: null, service_account_id: "sa_deploy" });
const old = externalSource({ id: "src_old", kind: "codex", name: "Old Codex", status: "archived", archived_at: "2026-10-01T12:00:00Z" });

afterEach(() => vi.useRealTimers());

describe("SourcesPage", () => {
  it("lists the active sources by name with their client, status and recent events", async () => {
    signedIn();
    projectData({ sources: [proxy, claude, old] });
    const { user } = await renderApp(projectPath("sources"));

    const grid = await screen.findByRole("grid", { name: "Sources" });
    const rows = within(grid).getAllByRole("gridcell");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Claude Code, Ada's laptop");
    expect(rows[0]).toHaveTextContent("Claude Code · last event");
    expect(rows[0]).toHaveTextContent("Connected");
    expect(rows[0]).toHaveTextContent("2 refused");
    expect(within(rows[0] as HTMLElement).queryByRole("img", { name: "Claude Code" })).not.toBeInTheDocument();
    expect((rows[0] as HTMLElement).querySelector("[data-source=claude_code]")).toHaveAttribute("aria-hidden", "true");
    expect(rows[1]).toHaveTextContent("LiteLLM proxy");
    expect(rows[1]).toHaveTextContent("Waiting");

    await user.click(screen.getByRole("tab", { name: "Archived" }));
    await waitFor(() => expect(within(screen.getByRole("grid", { name: "Sources" })).getAllByRole("gridcell")).toHaveLength(1));
    expect(screen.getByRole("grid", { name: "Sources" })).toHaveTextContent("Old Codex");
    await user.click(screen.getByRole("tab", { name: "All" }));
    await waitFor(() => expect(within(screen.getByRole("grid", { name: "Sources" })).getAllByRole("gridcell")).toHaveLength(3));
  });

  it("offers to add the first source, and says when none is archived", async () => {
    signedIn();
    projectData();
    const { user } = await renderApp(projectPath("sources"));
    expect(await screen.findByText("No sources yet")).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "Add source" }).at(-1) as HTMLElement);
    expect(await screen.findByRole("dialog", { name: /Add source/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("tab", { name: "Archived" }));
    expect(await screen.findByText("No archived sources")).toBeInTheDocument();
  });

  it("adds a source from the header or with C, lists service accounts by name, and opens a source", async () => {
    signedIn();
    projectData({ sources: [claude], accounts: [serviceAccount(), serviceAccount({ id: "sa_audit", name: "audit" })] });
    on("GET", SOURCE, claude);
    const { user, router } = await renderApp(projectPath("sources"));
    const grid = await screen.findByRole("grid", { name: "Sources" });

    await user.click(screen.getAllByRole("button", { name: "Add source" })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: /Add source/ });
    await user.click(within(dialog).getByRole("radio", { name: /HTTP/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("combobox", { name: "Counts for" }));
    const options = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(options).toEqual(["You", "audit (service account)", "ci-deploy (service account)"]);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("option")).not.toBeInTheDocument());
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add source/ })).not.toBeInTheDocument());

    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: /Add source/ })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: /Add source/ })).not.toBeInTheDocument());

    await user.click(within(grid).getByText("Claude Code, Ada's laptop"));
    expect(await screen.findByRole("dialog", { name: "Claude Code, Ada's laptop" })).toBeInTheDocument();
    expect(router.state.location.search).toMatchObject({ source: "src_claude" });
  });

  it("says why the sources couldn't load", async () => {
    signedIn();
    projectData();
    on("GET", SOURCES, apiError(400, "invalid_request", "That project can't list sources."));
    await renderApp(projectPath("sources"));
    expect((await notifications().findAllByText("That project can't list sources.")).length).toBeGreaterThan(0);
    expect(await screen.findByText("The sources couldn't load")).toBeInTheDocument();
  });

  it("asks for the right to add a source", async () => {
    signedIn({ projectRights: without("sources.manage") });
    projectData({ sources: [claude] });
    const { user } = await renderApp(projectPath("sources"));
    const add = await screen.findByRole("button", { name: "Add source" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, add, /Manage sources/);
  });

  it("doesn't open the add dialog from the address without the right", async () => {
    signedIn({ projectRights: without("sources.manage") });
    projectData();
    await renderApp(projectPath("sources?add=true"));
    expect(await screen.findByText("No sources yet")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: /Add source/ })).not.toBeInTheDocument();
  });

  it("adds a source, shows its key with the client's setup once, and waits for its first event", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"], shouldAdvanceTime: true });
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    const created = externalSource({ id: "src_new", kind: "codex", name: "Codex", status: "waiting", activity: [] });
    on("POST", SOURCES, { source: created, key: KEY });
    let reported = false;
    on("GET", SOURCE, () =>
      reported
        ? {
            ...created,
            status: "connected",
            first_event_at: "2026-10-15T11:59:00Z",
            activity: [{ day: "2026-10-15", requests: 2, tool_calls: 1, refused: 0, duplicates: 0 }],
          }
        : created,
    );
    const { user, router } = await renderApp(projectPath("sources?add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add source/ });
    expect(within(dialog).getByText("What reports to Kaeso?")).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: /Claude Code/ })).toBeChecked();
    await user.click(within(dialog).getByRole("radio", { name: /Codex/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const name = await within(dialog).findByRole("textbox", { name: "Name" });
    expect(name).toHaveValue("Codex");
    await user.clear(name);
    await user.click(within(dialog).getByRole("button", { name: "Add source" }));
    expect(requests("POST", SOURCES)).toHaveLength(0);
    await user.type(name, "Codex");
    await user.click(within(dialog).getByRole("checkbox", { name: /Keep prompts and replies/ }));
    await user.click(within(dialog).getByRole("checkbox", { name: /Keep prompts and replies/ }));
    await user.click(within(dialog).getByRole("button", { name: "Add source" }));

    expect(await notifications().findByText("Source Codex added")).toBeInTheDocument();
    expect(requests("POST", SOURCES).map((r) => r.body)).toEqual([{ kind: "codex", name: "Codex", match_members: false, content: false }]);
    const result = await screen.findByRole("dialog", { name: /Connect Codex/ });
    expect(within(result).getByRole("textbox", { name: "Key for Codex" })).toHaveTextContent(KEY);
    expect(within(result).getByText("~/.codex/config.toml")).toBeInTheDocument();
    expect(within(result).getAllByText("Waiting for the first event; use the client once.").length).toBeGreaterThan(0);

    const listed = requests("GET", SOURCES).length;
    reported = true;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(WATCH_MS);
    });
    await waitFor(() => expect(within(result).getByRole("status")).toHaveTextContent("Connected"));
    expect(within(result).getByRole("status")).toHaveTextContent("2 requests and 1 tool call so far");
    await waitFor(() => expect(requests("GET", SOURCES).length).toBeGreaterThan(listed));
    await user.click(within(result).getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("counts a new source's requests for a service account when chosen, and says what Kaeso refused", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    const created = externalSource({
      id: "src_new",
      kind: "http",
      name: "Scripts",
      status: "waiting",
      activity: [],
      last_error: "no_model",
      last_error_at: "2026-10-15T11:59:00Z",
    });
    on("POST", SOURCES, { source: created, key: KEY });
    on("GET", SOURCE, created);
    const { user } = await renderApp(projectPath("sources?add=true"));

    const dialog = await screen.findByRole("dialog", { name: /Add source/ });
    await user.click(within(dialog).getByRole("radio", { name: /HTTP/ }));
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    const name = await within(dialog).findByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "Scripts");
    await user.click(within(dialog).getByRole("combobox", { name: "Counts for" }));
    await user.click(await screen.findByRole("option", { name: "ci-deploy (service account)" }));
    const matching = within(dialog).getByRole("checkbox", { name: /Match members by email/ });
    expect(matching).not.toBeChecked();
    await user.click(matching);
    await user.click(within(dialog).getByRole("button", { name: "Add source" }));

    await waitFor(() =>
      expect(requests("POST", SOURCES).map((r) => r.body)).toEqual([
        { kind: "http", name: "Scripts", match_members: true, content: false, service_account_id: "sa_deploy" },
      ]),
    );
    const result = await screen.findByRole("dialog", { name: /Connect Scripts/ });
    expect(within(result).getByRole("status")).toHaveTextContent("Kaeso refused what it sent");
    expect(within(result).getByRole("status")).toHaveTextContent("An event named no model.");
  });

  it("puts a refused name back under the name field", async () => {
    signedIn();
    projectData();
    on("POST", SOURCES, apiError(400, "invalid_request", "A name can't contain control characters.", { param: "name" }));
    const { user } = await renderApp(projectPath("sources?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add source/ });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await user.click(await within(dialog).findByRole("button", { name: "Add source" }));
    const name = await within(dialog).findByRole("textbox", { name: "Name" });
    await waitFor(() => expect(name).toHaveAccessibleDescription("A name can't contain control characters."));
    await user.type(name, "x");
    expect(within(dialog).queryByText("A name can't contain control characters.")).not.toBeInTheDocument();
  });
});
