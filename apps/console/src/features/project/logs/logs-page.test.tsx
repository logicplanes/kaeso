import { act, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ALL_PROJECT_RIGHTS, signedIn } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { apiKey, catalogueModel, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { listPage, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const LOGS = "orgs/:org/projects/:project/logs";
const without = (...rights: string[]) => ALL_PROJECT_RIGHTS.filter((r) => !rights.includes(r));
const listQueries = () => requests("GET", LOGS).map((r) => Object.fromEntries(r.query));
const lastQuery = () => listQueries().at(-1) ?? {};
const requestsTable = () => screen.getByRole("table", { name: "Requests" });

describe("LogsPage", () => {
  it("lists requests with their model, outcome, tokens, key and sender", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()] });
    logsData({
      rows: [
        logRow(),
        logRow({
          id: "req_0002",
          status: 502,
          requested_model: "fast",
          model: "claude-sonnet-4-5",
          provider: "anthropic",
          error_kind: "provider_error",
          error_message: "Upstream said no",
          stream: true,
          ttfb_ms: null,
          key_id: null,
          source: "playground",
          principal_kind: "service_account",
          principal_id: "sa_deploy",
          usage: { input_tokens: 10, output_tokens: 5, cache_read_tokens: 3, cache_write_tokens: 2, reasoning_tokens: 1, estimated: true },
        }),
        logRow({ id: "req_0003", status: 499, error_kind: "client_disconnected", requested_model: "openai/gpt-5.4-mini", principal_id: "usr_grace" }),
      ],
    });
    const { user } = await renderApp(projectPath("logs"));

    const first = await screen.findByRole("row", { name: /200 OK/ });
    expect(first).toHaveTextContent(/200\s*OK/);
    expect(first).toHaveTextContent("Ada Okafor (you)");
    expect(first).toHaveTextContent("120 / 48");
    const failed = screen.getByRole("row", { name: /Upstream said no/ });
    expect(failed).toHaveTextContent("from fast");
    expect(failed).toHaveTextContent("Playground");
    expect(failed).toHaveTextContent("Provider error");
    expect(failed).toHaveTextContent("ci-deploy");
    expect(failed).toHaveTextContent("≈ 10 / 5");
    expect(failed).toHaveTextContent("—");
    expect(within(failed).getByText("Stream")).toBeInTheDocument();
    const disconnected = screen.getByRole("row", { name: /Client disconnected/ });
    expect(disconnected).not.toHaveTextContent("from openai/gpt-5.4-mini");
    expect(disconnected).toHaveTextContent("Grace Hopper");

    await hoverReason(user, within(failed).getByText("≈ 10 / 5"), /3 cache read · 2 cache write · 1 reasoning/);
    expect(lastQuery()).toMatchObject({ limit: "100" });
    expect(lastQuery().from).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect(lastQuery().to).toBeUndefined();
  });

  it("says when the range has no requests and blocks the export", async () => {
    signedIn();
    projectData();
    logsData();
    const { user } = await renderApp(projectPath("logs"));

    expect(await screen.findByText("No requests in this range")).toBeInTheDocument();
    expect(screen.getByText("Requests appear here as apps call the gateway with this project's keys.")).toBeInTheDocument();
    const exportButton = screen.getByRole("button", { name: "Export" });
    expect(exportButton).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, exportButton, "No requests match these filters");
  });

  it("suggests widening the search when the filters match nothing", async () => {
    signedIn();
    projectData();
    logsData();
    await renderApp(projectPath("logs?failed=true"));
    expect(await screen.findByText("No requests match these filters")).toBeInTheDocument();
    expect(screen.getByText("Remove a filter or widen the time range.")).toBeInTheDocument();
  });

  it("explains that request logs are unavailable", async () => {
    signedIn();
    projectData();
    on("GET", LOGS, apiError(503, "unavailable", "Log storage is down."));
    await renderApp(projectPath("logs"));
    expect(await screen.findByText("Request logs are unavailable for now")).toBeInTheDocument();
    expect(screen.getByText("Usage and limits still work.")).toBeInTheDocument();
  });

  it("shows a retry when the requests fail to load", async () => {
    signedIn();
    projectData();
    let calls = 0;
    on("GET", LOGS, () => (++calls === 1 ? apiError(403, "permission_denied", "These requests are hidden from you.") : listPage([logRow()])));
    const { user } = await renderApp(projectPath("logs"));
    expect(await screen.findByText("Couldn’t load these rows")).toBeInTheDocument();
    expect((await notifications().findAllByText("These requests are hidden from you.")).length).toBeGreaterThan(0);
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("row", { name: /key_web/ })).toBeInTheDocument();
  });

  it("sends every filter from the URL and shows each as a chip", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()], models: [catalogueModel()] });
    logsData();
    await renderApp(
      projectPath(
        "logs?range=7d&key=key_web&member=sa:sa_deploy&model=claude-sonnet-4-5&provider=anthropic&status=429&failed=true&error=rate_limited&stream=false&source=playground",
      ),
    );

    expect(await screen.findByText("No requests match these filters")).toBeInTheDocument();
    await waitFor(() =>
      expect(lastQuery()).toMatchObject({
        key: "key_web",
        service_account: "sa_deploy",
        model: "claude-sonnet-4-5",
        provider: "anthropic",
        status: "429",
        failed: "true",
        error_kind: "rate_limited",
        stream: "false",
        source: "playground",
      }),
    );
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Key" })).toHaveTextContent("checkout-web");
    expect(within(toolbar).getByRole("group", { name: "Filter: Member" })).toHaveTextContent("ci-deploy (service account)");
    expect(within(toolbar).getByRole("group", { name: "Filter: Model" })).toHaveTextContent("Claude Sonnet 4.5");
    expect(within(toolbar).getByRole("group", { name: "Filter: Provider" })).toHaveTextContent("Anthropic");
    expect(within(toolbar).getByRole("group", { name: "Filter: Status" })).toHaveTextContent("429");
    expect(within(toolbar).getByRole("group", { name: "Filter: Failed only" })).toHaveTextContent("yes");
    expect(within(toolbar).getByRole("group", { name: "Filter: Error kind" })).toHaveTextContent("Rate limited");
    expect(within(toolbar).getByRole("group", { name: "Filter: Streamed" })).toHaveTextContent("Not streamed");
    expect(within(toolbar).getByRole("group", { name: "Filter: Source" })).toHaveTextContent("Playground");
    expect(within(toolbar).getByRole("combobox", { name: "Time range" })).toHaveTextContent("Last 7 days");
  });

  it("removes a filter from the URL and the request", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    logsData();
    const { user, router } = await renderApp(projectPath("logs?member=user:usr_grace&failed=true"));

    await screen.findByText("No requests match these filters");
    expect(lastQuery()).toMatchObject({ user: "usr_grace", failed: "true" });
    await user.click(screen.getByRole("button", { name: "Remove the Member filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: true }));
    await user.click(screen.getByRole("button", { name: "Remove the Failed only filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(await screen.findByText("No requests in this range")).toBeInTheDocument();
    expect(lastQuery().user).toBeUndefined();
    expect(lastQuery().failed).toBeUndefined();
  });

  it("adds filters from the menu", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Failed only" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: true }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    const menu = await screen.findByRole("menu", { name: "Add filter" });
    expect(within(menu).queryByRole("menuitem", { name: "Failed only" })).not.toBeInTheDocument();
    await user.click(within(menu).getByRole("menuitem", { name: "Provider" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "OpenAI" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: true, provider: "openai" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ provider: "openai", failed: "true" }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Streamed" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Streamed" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ stream: true }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Status" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "503" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ status: 503 }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ status: "503", stream: "true" }));
  });

  it("drops a filter that was added but never chosen", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Source" }));
    expect(await screen.findByRole("menuitemradio", { name: "API" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    const chip = screen.getByRole("group", { name: "Filter: Source" });
    expect(chip).toHaveTextContent("any");
    await user.click(within(chip).getByRole("button", { name: "Remove the Source filter" }));
    await waitFor(() => expect(screen.queryByRole("group", { name: "Filter: Source" })).not.toBeInTheDocument());
  });

  it("switches the time range, including a custom span of days", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("combobox", { name: "Time range" }));
    await user.click(await screen.findByRole("option", { name: "Last hour" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ range: "1h" }));
    expect(screen.getByRole("row", { name: /key_web/ })).not.toHaveTextContent("Oct");

    await user.click(screen.getByRole("combobox", { name: "Time range" }));
    await user.click(await screen.findByRole("option", { name: "Custom range" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ from: expect.any(String), to: expect.any(String) }));
    expect(router.state.location.search).not.toHaveProperty("range");
    expect(await screen.findByRole("button", { name: "Days" })).toBeInTheDocument();
    await waitFor(() => expect(lastQuery().to).toMatch(/T00:00:00/));

    await user.click(screen.getByRole("combobox", { name: "Time range" }));
    await user.click(await screen.findByRole("option", { name: "Last 24 hours" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("opens a request from its row, steps with j and k, and closes it", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow(), logRow({ id: "req_0002", endpoint: "/v1/messages" }), logRow({ id: "req_0003", endpoint: "/v1/responses" })],
      entries: [logEntry(), logEntry({ id: "req_0002", endpoint: "/v1/messages" }), logEntry({ id: "req_0003", endpoint: "/v1/responses" })],
    });
    const { user, router } = await renderApp(projectPath("logs"));

    await user.click(await screen.findByRole("row", { name: /\/v1\/messages/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ log: "req_0002" }));
    expect(await screen.findByRole("dialog", { name: /\/v1\/messages/ })).toBeInTheDocument();
    await user.keyboard("j");
    await waitFor(() => expect(router.state.location.search).toEqual({ log: "req_0003" }));
    await user.keyboard("j");
    expect(router.state.location.search).toEqual({ log: "req_0003" });
    await user.keyboard("k");
    await user.keyboard("k");
    await waitFor(() => expect(router.state.location.search).toEqual({ log: "req_0001" }));
    await user.keyboard("x");
    await user.keyboard("{Control>}j{/Control}");
    expect(router.state.location.search).toEqual({ log: "req_0001" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("opens a request by its id", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()], entries: [logEntry({ id: "req_9999" })] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    const search = screen.getByRole("searchbox", { name: "Open a request by id" });
    await user.type(search, "{Enter}");
    expect(router.state.location.search).toEqual({});
    await user.type(search, "  req_9999 {Enter}");
    await waitFor(() => expect(router.state.location.search).toEqual({ log: "req_9999" }));
    await user.keyboard("j");
    expect(router.state.location.search).toEqual({ log: "req_9999" });
    await user.click(within(screen.getByRole("toolbar", { name: "Filter requests" })).getByRole("button", { name: /Clear/ }));
    expect(screen.getByRole("searchbox", { name: "Open a request by id" })).toHaveValue("");
  });

  it("turns live updates on and off", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    const live = await screen.findByRole("button", { name: "Live" });
    expect(live).toHaveAttribute("aria-pressed", "false");
    await user.click(live);
    await waitFor(() => expect(router.state.location.search).toEqual({ live: true }));
    expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "true");
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("loads the next page of requests", async () => {
    signedIn();
    projectData();
    on("GET", LOGS, ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? listPage([logRow({ id: "req_0002", endpoint: "/v1/embeddings" })]) : listPage([logRow()], "cur_2"),
    );
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await screen.findByRole("row", { name: /\/v1\/embeddings/ })).toBeInTheDocument();
    expect(listQueries().at(-1)).toMatchObject({ after: "cur_2" });
  });

  it("remembers the chosen columns", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    window.localStorage.setItem("kaeso:logs-columns", JSON.stringify(["time", "status", "request", "agent", "bogus"]));
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /openai-python/ });
    const headers = within(requestsTable())
      .getAllByRole("columnheader")
      .map((h) => h.textContent);
    expect(headers).toEqual(["Started", "Status", "Request id", "User agent"]);
    expect(screen.getByRole("button", { name: "req_0001" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Display options" }));
    const options = await screen.findByRole("dialog", { name: "Display options" });
    await user.click(within(options).getByRole("button", { name: "User agent" }));
    await waitFor(() => expect(JSON.parse(window.localStorage.getItem("kaeso:logs-columns") ?? "[]")).toEqual(["time", "status", "request"]));
    expect(within(requestsTable()).queryByRole("columnheader", { name: /User agent/ })).not.toBeInTheDocument();

    await user.click(within(options).getByRole("combobox", { name: "Density" }));
    await user.click(await screen.findByRole("option", { name: "Compact" }));
    expect(within(options).getByRole("combobox", { name: "Density" })).toHaveTextContent("Compact");
  });

  it("says which right export needs and names keys only with the right to see them", async () => {
    signedIn({ projectRights: without("logs.export", "keys.all.view", "project.members.view", "service_accounts.view") });
    projectData({ keys: [apiKey()] });
    logsData({ rows: [logRow({ principal_id: "usr_grace" })] });
    const { user } = await renderApp(projectPath("logs"));

    const row = await screen.findByRole("row", { name: /key_web/ });
    expect(row).toHaveTextContent("usr_grace");
    const exportButton = screen.getByRole("button", { name: "Export" });
    expect(exportButton).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, exportButton, /^You need Export logs; ask .+ for it\.$/);
    await user.click(exportButton);
    expect(screen.queryByRole("dialog", { name: "Export requests" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/keys")).toHaveLength(0);
  });

  it("narrows to the days picked in the calendar", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?from=2026-10-01&to=2026-10-03"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Days" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose a date range" });
    const start = within(dialog).getByRole("textbox", { name: "Start date" });
    const end = within(dialog).getByRole("textbox", { name: "End date" });
    await user.clear(start);
    await user.type(start, "2026-10-05");
    await user.clear(end);
    await user.type(end, "2026-10-07");
    await user.click(within(dialog).getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ from: "2026-10-05", to: "2026-10-07" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ from: "2026-10-05T00:00:00Z", to: "2026-10-08T00:00:00.000Z" }));
  });

  it("names a filter value it can't resolve by its id, and removes the status and streamed filters", async () => {
    signedIn();
    projectData({ models: [catalogueModel({ model_id: "gpt-5.4-mini", effective: { ...catalogueModel().effective, display_name: "gpt-5.4-mini" } })] });
    logsData();
    const { user, router } = await renderApp(projectPath("logs?key=key_gone&model=gpt-5.4-mini&status=500&stream=true"));

    await screen.findByText("No requests match these filters");
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Key" })).toHaveTextContent("key_gone");
    expect(within(toolbar).getByRole("group", { name: "Filter: Model" })).toHaveTextContent("gpt-5.4-mini");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Status filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ key: "key_gone", model: "gpt-5.4-mini", stream: true }));
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Streamed filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ key: "key_gone", model: "gpt-5.4-mini" }));
    await waitFor(() => expect(lastQuery().status).toBeUndefined());
    expect(lastQuery().stream).toBeUndefined();
  });

  it("loads the next page once the end of the list scrolls into view", async () => {
    const observers: { callback: IntersectionObserverCallback; targets: Element[] }[] = [];
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        targets: Element[] = [];
        constructor(public callback: IntersectionObserverCallback) {
          observers.push(this);
        }
        observe(el: Element) {
          this.targets.push(el);
        }
        disconnect() {
          this.targets = [];
        }
      },
    );
    signedIn();
    projectData();
    on("GET", LOGS, ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? listPage([logRow({ id: "req_0002", endpoint: "/v1/embeddings" })]) : listPage([logRow()], "cur_2"),
    );
    await renderApp(projectPath("logs"));

    await screen.findByRole("button", { name: "Load more" });
    const watching = () => observers.filter((o) => o.targets.length > 0).at(-1);
    await waitFor(() => expect(watching()).toBeDefined());
    const seen = (isIntersecting: boolean) => act(() => watching()?.callback([{ isIntersecting } as IntersectionObserverEntry], {} as IntersectionObserver));
    seen(false);
    expect(listQueries().some((q) => q.after === "cur_2")).toBe(false);
    seen(true);
    expect(await screen.findByRole("row", { name: /\/v1\/embeddings/ })).toBeInTheDocument();
    expect(listQueries().at(-1)).toMatchObject({ after: "cur_2" });
    vi.unstubAllGlobals();
  });
});

describe("Live updates", () => {
  const embeddings = logRow({ id: "req_0002", endpoint: "/v1/embeddings" });
  const responses = logRow({ id: "req_0003", endpoint: "/v1/responses" });
  const shownEndpoints = () =>
    within(requestsTable())
      .getAllByRole("row")
      .slice(1)
      .map((r) => within(r).getByText(/^\/v1\//).textContent);

  afterEach(() => vi.useRealTimers());

  async function goLive(user: Awaited<ReturnType<typeof renderApp>>["user"]) {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "true"));
  }

  it("adds new requests at the top, marks them for a moment and skips a failed check", async () => {
    signedIn();
    projectData();
    let newest = [logRow()];
    on("GET", LOGS, () => listPage(newest));
    const { user } = await renderApp(projectPath("logs"));
    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    await goLive(user);

    await vi.advanceTimersByTimeAsync(5000);
    await waitFor(() => expect(requests("GET", LOGS)).toHaveLength(2));
    expect(shownEndpoints()).toEqual(["/v1/chat/completions"]);

    newest = [embeddings, logRow()];
    await vi.advanceTimersByTimeAsync(5000);
    const added = await screen.findByRole("row", { name: /\/v1\/embeddings/ });
    expect(shownEndpoints()).toEqual(["/v1/embeddings", "/v1/chat/completions"]);
    expect(added).toHaveAttribute("data-fresh");
    await waitFor(() => expect(added).not.toHaveAttribute("data-fresh"));

    on("GET", LOGS, apiError(429, "rate_limited", "Slow down."));
    await vi.advanceTimersByTimeAsync(5000);
    await waitFor(() => expect(requests("GET", LOGS)).toHaveLength(4));
    expect(shownEndpoints()).toEqual(["/v1/embeddings", "/v1/chat/completions"]);
    expect(lastQuery()).toMatchObject({ limit: "100" });
  });

  it("holds new requests behind a button while the list is scrolled down", async () => {
    signedIn();
    projectData();
    let newest = [logRow()];
    on("GET", LOGS, () => listPage(newest));
    const { user } = await renderApp(projectPath("logs"));
    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    const body = requestsTable().closest<HTMLElement>(".overflow-y-auto");
    if (!body) throw new Error("no scrolling body");
    body.style.overflowY = "auto";
    body.scrollTop = 400;
    await goLive(user);

    newest = [embeddings, logRow()];
    await vi.advanceTimersByTimeAsync(5000);
    expect(await screen.findByRole("button", { name: "Show 1 new request" })).toBeInTheDocument();
    expect(shownEndpoints()).toEqual(["/v1/chat/completions"]);

    newest = [responses, embeddings, logRow()];
    await vi.advanceTimersByTimeAsync(5000);
    await user.click(await screen.findByRole("button", { name: "Show 2 new requests" }));
    expect(shownEndpoints()).toEqual(["/v1/responses", "/v1/embeddings", "/v1/chat/completions"]);
    expect(screen.queryByRole("button", { name: /new request/ })).not.toBeInTheDocument();
    expect(body.scrollTop).toBe(0);
  });

  it("drops a check that answers after live updates were turned off", async () => {
    signedIn();
    projectData();
    on("GET", LOGS, listPage([logRow()]));
    const { user } = await renderApp(projectPath("logs"));
    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    await goLive(user);

    let release: (page: unknown) => void = () => {};
    on("GET", LOGS, () => new Promise((resolve) => (release = resolve)));
    await vi.advanceTimersByTimeAsync(5000);
    await waitFor(() => expect(requests("GET", LOGS)).toHaveLength(2));
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "false"));
    release(listPage([embeddings, logRow()]));
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "true"));
    await vi.advanceTimersByTimeAsync(0);

    expect(shownEndpoints()).toEqual(["/v1/chat/completions"]);
  });

  it("shows a request once when the first page arrives after it came in live", async () => {
    signedIn();
    projectData();
    let release: (page: unknown) => void = () => {};
    on("GET", LOGS, () => new Promise((resolve) => (release = resolve)));
    const { user } = await renderApp(projectPath("logs"));
    await waitFor(() => expect(requests("GET", LOGS)).toHaveLength(1));
    await goLive(user);

    on("GET", LOGS, listPage([embeddings]));
    await vi.advanceTimersByTimeAsync(5000);
    await screen.findByRole("row", { name: /\/v1\/embeddings/ });
    release(listPage([embeddings, logRow()]));
    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    expect(shownEndpoints()).toEqual(["/v1/embeddings", "/v1/chat/completions"]);
  });
});

describe("Export dialog", () => {
  it("summarises the filters and downloads the chosen format", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], models: [catalogueModel()] });
    logsData({ rows: [logRow()] });
    const clicked: string[] = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      clicked.push(this.href);
    });
    const { user } = await renderApp(projectPath("logs?key=key_web&member=user:usr_grace&model=claude-sonnet-4-5&from=2026-10-01&to=2026-10-03"));

    await screen.findByRole("row", { name: /checkout-web/ });
    await user.click(screen.getByRole("button", { name: "Export" }));
    const dialog = await screen.findByRole("dialog", { name: "Export requests" });
    expect(dialog).toHaveTextContent(
      "Every request from 1 Oct 2026 to 3 Oct 2026 by Grace Hopper with key checkout-web and model Claude Sonnet 4.5, newest first.",
    );
    await user.click(within(dialog).getByRole("radio", { name: /^JSON Lines Metadata/ }));
    await user.click(within(dialog).getByRole("button", { name: "Export" }));

    await waitFor(() => expect(clicked).toHaveLength(1));
    const url = new URL(clicked[0] ?? "");
    expect(url.pathname).toBe("/platform/v1/orgs/org_northwind/projects/proj_checkout/logs/export");
    expect(Object.fromEntries(url.searchParams)).toMatchObject({ format: "jsonl", key: "key_web", user: "usr_grace", model: "claude-sonnet-4-5" });
    expect(url.searchParams.has("include_content")).toBe(false);
    expect(requests("GET", LOGS).some((r) => r.query.get("limit") === "1")).toBe(true);
    expect(await notifications().findByText("Export started")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Export requests" })).not.toBeInTheDocument();
    click.mockRestore();
  });

  it("adds the content with the right, and explains who can give it without", async () => {
    signedIn({ projectRights: without("logs.content.view") });
    projectData();
    logsData({ rows: [logRow()] });
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Export" }));
    const dialog = await screen.findByRole("dialog", { name: "Export requests" });
    expect(dialog).toHaveTextContent("Every request from the last 24 hours, newest first.");
    const content = within(dialog).getByRole("radio", { name: /JSON Lines with content/ });
    expect(content).toHaveAttribute("aria-disabled", "true");
    expect(await within(dialog).findByText("You need View prompts and outputs; ask Lin Wei for it.")).toBeInTheDocument();
  });

  it("asks for the stored bodies with the content format", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const clicked: string[] = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      clicked.push(this.href);
    });
    const { user } = await renderApp(projectPath("logs?failed=true&stream=true&source=api"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Export" }));
    const dialog = await screen.findByRole("dialog", { name: "Export requests" });
    expect(dialog).toHaveTextContent("Every failed streamed API request from the last 24 hours, newest first.");
    await user.click(within(dialog).getByRole("radio", { name: /JSON Lines with content/ }));
    await user.click(within(dialog).getByRole("button", { name: "Export" }));
    await waitFor(() => expect(clicked).toHaveLength(1));
    expect(new URL(clicked[0] ?? "").searchParams.get("include_content")).toBe("true");
    click.mockRestore();
  });

  it("shows the API's message when the export is refused", async () => {
    signedIn();
    projectData();
    on("GET", LOGS, ({ query }: { query: URLSearchParams }) =>
      query.get("limit") === "1" ? apiError(403, "permission_denied", "Exports are paused for this project.") : listPage([logRow()]),
    );
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Export" }));
    const dialog = await screen.findByRole("dialog", { name: "Export requests" });
    await user.click(within(dialog).getByRole("button", { name: "Export" }));
    expect((await notifications().findAllByText("Exports are paused for this project.")).length).toBeGreaterThan(0);
  });

  it("saves to the chosen file and shows the export while it runs", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const written: string[] = [];
    const decoder = new TextDecoder();
    Object.defineProperty(window, "showSaveFilePicker", {
      configurable: true,
      writable: true,
      value: async () => ({
        name: "requests.csv",
        createWritable: async () => new WritableStream<Uint8Array>({ write: (chunk) => void written.push(decoder.decode(chunk)) }),
      }),
    });
    let finish: (r: Response) => void = () => {};
    on("GET", `${LOGS}/export`, () => new Promise<Response>((resolve) => (finish = resolve)));
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Export" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Export requests" })).getByRole("button", { name: "Export" }));
    expect((await notifications().findAllByText("Exporting requests")).length).toBeGreaterThan(0);
    await waitFor(() => expect(requests("GET", `${LOGS}/export`)).toHaveLength(1));
    expect(requests("GET", `${LOGS}/export`)[0]?.query.get("format")).toBe("csv");

    await user.click(screen.getByRole("button", { name: "Export" }));
    expect(screen.queryByRole("dialog", { name: "Export requests" })).not.toBeInTheDocument();

    finish(new Response("id,status\nreq_0001,200\n", { status: 200, headers: { "content-type": "text/csv" } }));
    expect(await notifications().findByText("Requests exported")).toBeInTheDocument();
    expect(written.join("")).toBe("id,status\nreq_0001,200\n");
    Reflect.deleteProperty(window, "showSaveFilePicker");
  });
});
