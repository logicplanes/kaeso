import { act, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, onTestFinished, vi } from "vitest";
import { myMembership, providers as providerList, signedIn, whoAmI, without } from "~/test/fixtures";
import { credential, hoverReason, notifications } from "~/test/fixtures/org";
import { alias, apiKey, catalogueModel, projectData, projectMember, projectPath, serviceAccount } from "~/test/fixtures/project";
import { errorGroup, listPage, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const LOGS = "orgs/:org/projects/:project/logs";
const listQueries = () => requests("GET", LOGS).map((r) => Object.fromEntries(r.query));
const lastQuery = () => listQueries().at(-1) ?? {};
const requestsTable = () => screen.getByRole("grid", { name: "Requests" });

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
        logRow({ id: "req_0004", status: 202, requested_model: "house-video", cost: "0", cost_known: false, principal_id: "usr_grace" }),
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
    expect(failed).toHaveTextContent("≈ 15 / 5");
    expect(failed).toHaveTextContent("—");
    expect(within(failed).getByText("Stream")).toBeInTheDocument();
    const disconnected = screen.getByRole("row", { name: /Client disconnected/ });
    expect(disconnected).not.toHaveTextContent("from openai/gpt-5.4-mini");
    expect(disconnected).toHaveTextContent("Grace Hopper");
    expect(screen.getByRole("row", { name: /house-video/ })).toHaveTextContent("Unknown");

    await hoverReason(user, within(failed).getByText("≈ 15 / 5"), /3 cache read · 2 cache write · 1 reasoning/);
    expect(lastQuery()).toMatchObject({ limit: "100" });
    expect(lastQuery().from).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect(lastQuery().to).toBeUndefined();
  });

  it("shows each cost in the organization's currency, in the list, the filter and the request's details", async () => {
    signedIn({ memberships: [myMembership({ org_currency: "EUR" })] });
    projectData({ keys: [apiKey()] });
    logsData({
      rows: [logRow({ converted_cost: "0.000378", currency: "EUR" }), logRow({ id: "req_0002", cost: "0", cost_known: false, currency: "EUR" })],
      entries: [logEntry({ converted_cost: "0.000378", currency: "EUR" })],
    });
    const { user } = await renderApp(projectPath("logs?min_cost=0.1"));

    const table = await screen.findByRole("grid", { name: "Requests" });
    await waitFor(() => expect(within(table).getByText("€0.000378")).toBeInTheDocument());
    expect(within(table).queryByText(/\$/)).not.toBeInTheDocument();
    expect(lastQuery()).not.toHaveProperty("currency");
    expect(screen.getByRole("group", { name: "Filter: Cost at least" })).toHaveTextContent("€0.10");
    expect(screen.queryByRole("combobox", { name: "Currency" })).not.toBeInTheDocument();

    await user.click(within(table).getAllByRole("row")[1] as HTMLElement);
    const details = await screen.findByRole("dialog", { name: /^\d{3}/ });
    await waitFor(() => expect(within(details).getByText("€0.000378")).toBeInTheDocument());
    expect(within(details).queryByText(/\$/)).not.toBeInTheDocument();
  });

  it("says a decision's output tokens cost nothing", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()] });
    logsData({
      rows: [
        logRow({
          ingress_dialect: "system_one",
          egress_dialect: "openai_decisions",
          usage: { input_tokens: 200, output_tokens: 0, cache_read_tokens: 1000, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false },
        }),
      ],
    });
    const { user } = await renderApp(projectPath("logs"));

    const decision = await screen.findByRole("row", { name: /200 OK/ });
    await hoverReason(user, within(decision).getByText("1,200 / 0"), /1,000 cache read · A decision; its output tokens cost nothing/);
  });

  it("writes the gateway's messages in the person's language and names the provider whose own words a message is", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [
        logRow({
          id: "req_0010",
          status: 404,
          provider: "groq",
          model: "llama-3.1-8b-instant",
          requested_model: "llama-3.1-8b-instant",
          error_kind: "not_found",
          error_message:
            "Groq does not serve the model `llama-3.1-8b-instant` to this project's provider key; check the key's access to it, or remove the model from the catalogue.",
        }),
        logRow({ id: "req_0011", status: 429, provider: "openrouter", error_kind: "provider_error", error_message: "Provider returned error" }),
        logRow({ id: "req_0012", status: 529, provider: "anthropic", error_kind: "overloaded", error_message: "Overloaded" }),
      ],
    });
    await renderApp(projectPath("logs"), { locale: "de" });

    const unserved = await screen.findByRole("row", { name: /llama-3\.1-8b-instant Nicht gefunden Groq bietet/ });
    expect(within(unserved).getAllByText("llama-3.1-8b-instant", { selector: "code" })).toHaveLength(1);
    expect(screen.getByRole("row", { name: /OpenRouter: Provider returned error/ })).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /Anbieter überlastet/ })).not.toHaveTextContent("Overloaded");
  });

  it("on a phone, writes the start time above its day and folds Add filter into an icon", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query === "(max-width: 639px)",
      media: query,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
    }));
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()] });
    logsData({ rows: [logRow()] });
    await renderApp(projectPath("logs"));

    const row = await screen.findByRole("row", { name: /200 OK/ });
    const started = within(row).getAllByRole("gridcell")[0] as HTMLElement;
    expect(within(started).getByText(/^\d{2}:\d{2}$/)).toBeInTheDocument();
    expect(within(started).getByText(/^\d{1,2} [A-Z][a-z]{2,3}$/)).toBeInTheDocument();
    expect(within(requestsTable()).getAllByRole("columnheader")[0]).toHaveTextContent(/^Started$/);
    const add = screen.getByRole("button", { name: "Add filter" });
    expect(add).not.toHaveTextContent("Add filter");
    vi.unstubAllGlobals();
  });

  it("on a phone, keeps the seconds when the range is a single hour", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query === "(max-width: 639px)",
      media: query,
      addEventListener() {},
      removeEventListener() {},
    }));
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()] });
    logsData({ rows: [logRow()] });
    await renderApp(projectPath("logs?range=1h"));

    const row = await screen.findByRole("row", { name: /200 OK/ });
    const [started] = within(row).getAllByRole("gridcell");
    expect(started?.textContent).toMatch(/^\d{1,2}:\d{2}:\d{2}$/);
    vi.unstubAllGlobals();
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
    let failing = true;
    on("GET", LOGS, () => (failing ? apiError(403, "permission_denied", "These requests are hidden from you.") : listPage([logRow()])));
    const { user } = await renderApp(projectPath("logs"));
    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    expect((await notifications().findAllByText("These requests are hidden from you.")).length).toBeGreaterThan(0);
    failing = false;
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("row", { name: /key_web/ })).toBeInTheDocument();
  });

  it("sends every filter from the URL and shows each as a chip", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()], models: [catalogueModel()] });
    logsData();
    await renderApp(
      projectPath(
        "logs?range=7d&key=key_web&member=sa:sa_deploy&model=claude-sonnet-4-5&provider=anthropic&ingress=openai_responses&egress=anthropic_messages&status=429&failed=true&error=rate_limited&stream=false&source=playground",
      ),
    );

    expect(await screen.findByText("No requests match these filters")).toBeInTheDocument();
    await waitFor(() =>
      expect(lastQuery()).toMatchObject({
        key: "key_web",
        service_account: "sa_deploy",
        model: "claude-sonnet-4-5",
        provider: "anthropic",
        ingress_dialect: "openai_responses",
        egress_dialect: "anthropic_messages",
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
    expect(within(toolbar).getByRole("group", { name: "Filter: App format" })).toHaveTextContent("OpenAI Responses");
    expect(within(toolbar).getByRole("group", { name: "Filter: Provider format" })).toHaveTextContent("Anthropic Messages");
    expect(within(toolbar).getByRole("group", { name: "Filter: Status" })).toHaveTextContent("429");
    expect(within(toolbar).getByRole("group", { name: "Filter: Outcome" })).toHaveTextContent("Failed");
    expect(within(toolbar).getByRole("group", { name: "Filter: Error kind" })).toHaveTextContent("Rate limited");
    expect(within(toolbar).getByRole("group", { name: "Filter: Streaming" })).toHaveTextContent("Not streamed");
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
    await user.click(screen.getByRole("button", { name: "Remove the Outcome filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    expect(await screen.findByText("No requests in this range")).toBeInTheDocument();
    expect(lastQuery().user).toBeUndefined();
    expect(lastQuery().failed).toBeUndefined();
  });

  it("lists providers with a key first and groups a long model list by provider", async () => {
    signedIn();
    const models = Array.from({ length: 60 }, (_, i) =>
      catalogueModel({
        id: `cm_${i}`,
        provider: i % 2 ? "anthropic" : "openai",
        model_id: `model-${i}`,
        effective: { ...catalogueModel().effective, display_name: null },
      }),
    );
    projectData({ models });
    logsData({ rows: [logRow()] });
    on("GET", "orgs/:org/credentials", { data: [credential({ provider: "deepseek" })] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(within(await screen.findByRole("menu", { name: "Add filter" })).getByRole("menuitem", { name: "Provider" }));
    expect(within(await screen.findByRole("group", { name: "Connected" })).getByRole("option")).toHaveAccessibleName("DeepSeek");
    expect(screen.getByRole("group", { name: "Other providers" })).toContainElement(screen.getByRole("option", { name: "Anthropic" }));
    expect(screen.getByRole("combobox", { name: "Search provider values" })).toBeInTheDocument();
    const providers = screen.getAllByRole("option");
    expect(providers[0]).toHaveAccessibleName("DeepSeek");
    const others = providerList
      .filter((p) => p.id !== "deepseek")
      .map((p) => p.name)
      .sort((a, b) => a.localeCompare(b, "en", { sensitivity: "base" }));
    others.forEach((name, i) => expect(providers[i + 1]).toHaveAccessibleName(name));
    await user.keyboard("{Escape}");

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(within(await screen.findByRole("menu", { name: "Add filter" })).getByRole("menuitem", { name: "Model" }));
    const list = await screen.findByRole("listbox", { name: "Model" });
    expect(within(list).getByRole("group", { name: "Anthropic" })).toBeInTheDocument();
    await user.type(screen.getByRole("combobox", { name: "Search model values" }), "model-59");
    await user.click(within(list).getByRole("option", { name: /^model-59/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ model: "model-59" }));
  });

  it("adds filters from the menu", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Outcome" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Failed" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: true }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    const menu = await screen.findByRole("menu", { name: "Add filter" });
    expect(within(menu).queryByRole("menuitem", { name: "Outcome" })).not.toBeInTheDocument();
    await user.click(within(menu).getByRole("menuitem", { name: "Provider" }));
    await user.click(await screen.findByRole("option", { name: "OpenAI" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: true, provider: "openai" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ provider: "openai", failed: "true" }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Streaming" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Streamed" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ stream: true }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Status" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "503" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ status: 503 }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ status: "503", stream: "true" }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Source" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Playground" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ source: "playground" }));
  });

  it("filters by alias, fallback, endpoint and cost, from the URL and the menu", async () => {
    signedIn();
    projectData({ aliases: [alias()] });
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?alias=fast&fell_back=true&min_cost=0.1&endpoint=nowhere"));

    await waitFor(() => expect(lastQuery()).toMatchObject({ alias: "fast", fell_back: "true", min_cost: "0.1", endpoint: "nowhere" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Alias" })).toHaveTextContent("fast");
    expect(within(toolbar).getByRole("group", { name: "Filter: Fell back" })).toHaveTextContent("yes");
    expect(within(toolbar).getByRole("group", { name: "Filter: Cost at least" })).toHaveTextContent("$0.10");
    await user.click(screen.getByRole("button", { name: "Remove the Endpoint filter" }));
    await user.click(screen.getByRole("button", { name: "Remove the Alias filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ fell_back: true, min_cost: "0.1" }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Alias" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "fast" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Endpoint" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "responses" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ alias: "fast", endpoint: "responses" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ alias: "fast", endpoint: "responses", fell_back: "true", min_cost: "0.1" }));
    await user.click(screen.getByRole("button", { name: "Remove the Fell back filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ alias: "fast", endpoint: "responses", min_cost: "0.1" }));
  });

  it("finds the requests for a name the app sent, a deleted alias's too, and shows the name where no model answered", async () => {
    signedIn();
    projectData({ aliases: [alias()], models: [catalogueModel()] });
    logsData({ rows: [logRow({ id: "req_gone", model: "", provider: "", requested_model: "old-name", status: 404, error_kind: "not_found" })] });
    const { user, router } = await renderApp(projectPath("logs?requested_model=old-name"));

    await waitFor(() => expect(lastQuery()).toMatchObject({ requested_model: "old-name" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Requested model" })).toHaveTextContent("old-name");
    expect(await within(requestsTable()).findByText("old-name")).toBeInTheDocument();

    await user.click(within(toolbar).getByRole("button", { name: "Remove the Requested model filter" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Requested model" }));
    expect(await screen.findByRole("menuitemradio", { name: "fast" })).toBeInTheDocument();
    expect(screen.getByRole("menuitemradio", { name: "claude-sonnet-4-5" })).toBeInTheDocument();
    await user.click(screen.getByRole("menuitemradio", { name: "anthropic/claude-sonnet-4-5" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ requested_model: "anthropic/claude-sonnet-4-5" }));
  });

  it("lists the most common errors above the failures, and a row lists the requests it counts", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow({ status: 429 })],
      errors: [errorGroup(), errorGroup({ error_kind: "", status: 500, provider: "", count: 1, example: "" })],
    });
    const { user, router } = await renderApp(projectPath("logs?failed=true"));
    const list = await screen.findByRole("grid", { name: "Most common errors" });
    await waitFor(() => expect(within(list).getAllByRole("row")).toHaveLength(2));
    expect(within(list).getAllByRole("row")[0]).toHaveTextContent(/Rate limit reached for gpt-5\.4-mini.*Rate limited · 429 · OpenAI.*12 requests/);
    expect(requests("GET", "orgs/:org/projects/:project/logs/errors").at(-1)?.query.get("limit")).toBe("10");
    await user.click(within(list).getAllByRole("gridcell")[0] as HTMLElement);
    await waitFor(() =>
      expect(router.state.location.search).toEqual({
        failed: true,
        error: "rate_limited",
        status: 429,
        provider: "openai",
        error_message: "Rate limit reached for gpt",
      }),
    );
    await waitFor(() => expect(lastQuery()).toMatchObject({ error_message: "Rate limit reached for gpt", error_kind: "rate_limited" }));
  });

  it("shows no most common errors without failures to count, and says when they couldn't load", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { router } = await renderApp(projectPath("logs?failed=true"));
    await screen.findByRole("grid", { name: "Requests" });
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/logs/errors")).toHaveLength(1));
    expect(screen.queryByText("Most common errors")).not.toBeInTheDocument();
    on("GET", "orgs/:org/projects/:project/logs/errors", apiError(503, "unavailable", "The request log is unavailable."));
    await router.navigate({ to: projectPath("logs"), search: { failed: true, status: 500 } } as never);
    expect(await screen.findByText("The most common errors couldn't load.")).toBeInTheDocument();
  });

  it("filters by text the error message contains, typed or chosen", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?error_message=quota"));
    await waitFor(() => expect(lastQuery()).toMatchObject({ error_message: "quota" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Error message" })).toHaveTextContent("Error messagecontainsquota");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Error message filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Error message" }));
    await user.keyboard("context length");
    await user.click(await screen.findByRole("menuitem", { name: "Contains “context length”" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ error_message: "context length" }));
  });

  it("filters by the client app, from the address or the menu", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?client=claude-code"));
    await waitFor(() => expect(lastQuery()).toMatchObject({ client: "claude-code" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Client" })).toHaveTextContent("Claude Code");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Client filter" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Client" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Codex" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ client: "codex" }));
  });

  it("filters by a tag from the address or from the tags of the requests shown", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow({ tags: ["search", "checkout"] }), logRow({ id: "req_0002", tags: ["checkout"] })] });
    const { user, router } = await renderApp(projectPath("logs?tag=eval-7"));
    await waitFor(() => expect(lastQuery()).toMatchObject({ tag: "eval-7" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Tag" })).toHaveTextContent("eval-7");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Tag filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Tag" }));
    expect((await screen.findAllByRole("menuitemradio")).map((o) => o.textContent)).toEqual(["checkout", "search"]);
    await user.click(screen.getByRole("menuitemradio", { name: "search" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ tag: "search" }));
  });

  it("filters by what a guardrail did", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?guardrail_action=block"));
    await waitFor(() => expect(lastQuery()).toMatchObject({ guardrail_action: "block" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Guardrail" })).toHaveTextContent("Blocked");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Guardrail filter" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Guardrail" }));
    expect((await screen.findAllByRole("menuitemradio")).map((o) => o.textContent)).toEqual(["Allowed", "Blocked", "Rewritten"]);
    await user.click(screen.getByRole("menuitemradio", { name: "Rewritten" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ guardrail_action: "replace" }));
  });

  it("ignores a guardrail action it doesn't know in the address", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    await renderApp(projectPath("logs?guardrail_action=maybe"));
    await waitFor(() => expect(lastQuery()).not.toHaveProperty("guardrail_action"));
  });

  it("narrows to the requests that worked, the response cache's answers and slow requests", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?failed=false&response_cache=hit&min_duration_ms=2345"));

    await waitFor(() => expect(lastQuery()).toMatchObject({ failed: "false", response_cache: "hit", min_duration_ms: "2345" }));
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Outcome" })).toHaveTextContent("Succeeded");
    expect(within(toolbar).getByRole("group", { name: "Filter: Response cache" })).toHaveTextContent("Answered from the cache");
    expect(within(toolbar).getByRole("group", { name: "Filter: Slower than" })).toHaveTextContent("2.3 s");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Slower than filter" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Slower than" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "10 s" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: false, response_cache: "hit", min_duration_ms: 10000 }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ min_duration_ms: "10000" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Fell back" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ fell_back: true }));
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Response cache filter" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Response cache" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Sent to the provider" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ response_cache: "miss" }));
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Cost at least" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "$1.00" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ min_cost: "1" }));
  });

  it("ignores a cost in the URL that isn't an amount", async () => {
    signedIn();
    projectData();
    logsData();
    await renderApp(projectPath("logs?min_cost=-1"));
    await screen.findByText("No requests in this range");
    expect(lastQuery().min_cost).toBeUndefined();
    expect(screen.queryByRole("group", { name: "Filter: Cost at least" })).not.toBeInTheDocument();
  });

  it("narrows the list to one route by the app's and the provider's format", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "App format" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "OpenAI Responses" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ ingress: "openai_responses" }));

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Provider format" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Anthropic Messages" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ ingress: "openai_responses", egress: "anthropic_messages" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ ingress_dialect: "openai_responses", egress_dialect: "anthropic_messages" }));
  });

  it("keeps the not streamed filter", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Streaming" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Not streamed" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ stream: false }));
    expect(within(screen.getByRole("toolbar", { name: "Filter requests" })).getByRole("group", { name: "Filter: Streaming" })).toHaveTextContent(
      "Not streamed",
    );
    await waitFor(() => expect(lastQuery()).toMatchObject({ stream: "false" }));
  });

  it("drops a filter that was added but closed without a value", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    logsData({ rows: [logRow()] });
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Source" }));
    expect(await screen.findByRole("menuitemradio", { name: "API" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("group", { name: "Filter: Source" })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Model" }));
    expect(await screen.findByRole("listbox", { name: "Model" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("group", { name: "Filter: Model" })).not.toBeInTheDocument());
  });

  it("puts a filter just added after the ones already there", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?status=429"));

    await screen.findByRole("row", { name: /checkout-web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Key" }));
    expect(await screen.findByRole("searchbox", { name: "Search key values" })).toBeInTheDocument();
    await user.click(await screen.findByRole("menuitemradio", { name: "checkout-web" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ key: "key_web" }));
    const chips = within(screen.getByRole("toolbar", { name: "Filter requests" })).getAllByRole("group", { name: /^Filter: / });
    expect(chips.map((c) => c.getAttribute("aria-label"))).toEqual(["Filter: Status", "Filter: Key"]);
  });

  it("offers only the chosen provider's models, under their names", async () => {
    signedIn();
    projectData({
      models: [
        catalogueModel({
          id: "cm_1",
          provider: "mistral",
          model_id: "mistral-small-latest",
          effective: { ...catalogueModel().effective, display_name: "Mistral Small" },
        }),
        catalogueModel({ id: "cm_2", provider: "groq", model_id: "llama-3.1-8b-instant", effective: { ...catalogueModel().effective, display_name: null } }),
      ],
    });
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?provider=mistral"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(await screen.findByRole("menuitem", { name: "Model" }));
    const list = await screen.findByRole("listbox", { name: "Model" });
    expect(
      within(list)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([expect.stringMatching(/^Mistral Small/)]);
    await user.click(within(list).getByRole("option", { name: /Mistral Small/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ provider: "mistral", model: "mistral-small-latest" }));
    expect(screen.getByRole("group", { name: "Filter: Model" })).toHaveTextContent("Mistral Small");
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
    expect(await screen.findByRole("button", { description: "Days" })).toBeInTheDocument();
    await waitFor(() => expect(lastQuery().to).toMatch(/T00:00:00/));

    await user.click(screen.getByRole("combobox", { name: "Time range" }));
    await user.click(await screen.findByRole("option", { name: "Last 24 hours" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
  });

  it("starts a custom span on today in the install's time zone", async () => {
    vi.setSystemTime(new Date("2026-10-14T20:00:00Z"));
    onTestFinished(() => void vi.useRealTimers());
    signedIn({ session: whoAmI({ timezone: "Asia/Tokyo" }) });
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("combobox", { name: "Time range" }));
    await user.click(await screen.findByRole("option", { name: "Custom range" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ from: "2026-10-15", to: "2026-10-15" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ from: "2026-10-14T15:00:00.000Z", to: "2026-10-15T15:00:00.000Z" }));
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
    expect(await screen.findByRole("dialog", { name: /^\d{3}/ })).toHaveTextContent("/v1/messages");
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
    expect(headers).toEqual(["Started (UTC)", "Status", "Request id", "User agent"]);
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

  it("shows the provider's mark beside the model on a tablet and says which chosen columns don't fit", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width: 700,
      height: 0,
      top: 0,
      left: 0,
      right: 700,
      bottom: 0,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user } = await renderApp(projectPath("logs"));

    const row = await screen.findByRole("row", { name: /200 OK/ });
    const model = within(row)
      .getAllByRole("gridcell")
      .find((c) => c.textContent?.includes("gpt-5.4-mini")) as HTMLElement;
    expect(within(model).getByRole("img", { name: "OpenAI" })).toBeInTheDocument();
    expect(within(requestsTable()).queryByRole("columnheader", { name: /Provider/ })).not.toBeInTheDocument();
    expect(within(requestsTable()).queryByRole("columnheader", { name: /Endpoint/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Display options" }));
    const options = await screen.findByRole("dialog", { name: "Display options" });
    expect(within(options).getByRole("button", { name: "Endpoint" })).toHaveAttribute("aria-description", "Doesn't fit at this width");
    expect(within(options).getByText(/^Endpoint, .* don't fit at this width\.$/)).toBeInTheDocument();
    vi.restoreAllMocks();
  });

  it.each([
    ["a tablet", 700],
    ["a laptop", 1000],
  ])("keeps the provider's own column with its name when Model is hidden on %s", async (_device, width) => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
      width,
      height: 0,
      top: 0,
      left: 0,
      right: width,
      bottom: 0,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    window.localStorage.setItem("kaeso:logs-columns", JSON.stringify(["time", "status", "provider", "duration"]));
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    await renderApp(projectPath("logs"));

    const row = await screen.findByRole("row", { name: /200 OK/ });
    expect(within(requestsTable()).getByRole("columnheader", { name: /^Provider/ })).toHaveTextContent("Provider");
    expect(within(requestsTable()).queryByRole("columnheader", { name: /^Model/ })).not.toBeInTheDocument();
    expect(within(row).getByText("OpenAI")).toBeInTheDocument();
    vi.restoreAllMocks();
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

  it("narrows to the days picked in the calendar, from midnight in the install's time zone", async () => {
    signedIn({ session: whoAmI({ timezone: "Asia/Tokyo" }) });
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?from=2026-10-01&to=2026-10-03"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { description: "Days" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose a date range" });
    const start = within(dialog).getByRole("textbox", { name: "Start date" });
    const end = within(dialog).getByRole("textbox", { name: "End date" });
    await user.clear(start);
    await user.type(start, "2026-10-05");
    await user.clear(end);
    await user.type(end, "2026-10-07");
    await user.click(within(dialog).getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ from: "2026-10-05", to: "2026-10-07" }));
    await waitFor(() => expect(lastQuery()).toMatchObject({ from: "2026-10-04T15:00:00.000Z", to: "2026-10-07T15:00:00.000Z" }));
  });

  it("reads Analytics' days in UTC, and the install's days again once the range is picked", async () => {
    signedIn({ session: whoAmI({ timezone: "Asia/Tokyo" }) });
    projectData();
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs?failed=true&from=2026-10-01&to=2026-10-03&utc=true"));

    await screen.findByRole("row", { name: /key_web/ });
    expect(screen.getByText("Days in UTC")).toBeInTheDocument();
    await waitFor(() => expect(lastQuery()).toMatchObject({ from: "2026-10-01T00:00:00.000Z", to: "2026-10-04T00:00:00.000Z" }));
    await user.click(screen.getByRole("button", { description: "Days" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose a date range" });
    const start = within(dialog).getByRole("textbox", { name: "Start date" });
    await user.clear(start);
    await user.type(start, "2026-10-02");
    await user.click(within(dialog).getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ failed: true, from: "2026-10-02", to: "2026-10-03" }));
    expect(screen.queryByText("Days in UTC")).not.toBeInTheDocument();
    await waitFor(() => expect(lastQuery()).toMatchObject({ from: "2026-10-01T15:00:00.000Z", to: "2026-10-03T15:00:00.000Z" }));
  });

  it("names a filter value it can't resolve by its id, and removes the status and streamed filters", async () => {
    signedIn();
    projectData({ models: [catalogueModel({ model_id: "gpt-5.4-mini", effective: { ...catalogueModel().effective, display_name: "gpt-5.4-mini" } })] });
    logsData();
    const { user, router } = await renderApp(projectPath("logs?key=key_gone&model=gpt-5.4-mini&status=500&stream=true&error=timeout"));

    await screen.findByText("No requests match these filters");
    const toolbar = screen.getByRole("toolbar", { name: "Filter requests" });
    expect(within(toolbar).getByRole("group", { name: "Filter: Key" })).toHaveTextContent("key_gone");
    expect(within(toolbar).getByRole("group", { name: "Filter: Model" })).toHaveTextContent("gpt-5.4-mini");
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Status filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ key: "key_gone", model: "gpt-5.4-mini", stream: true, error: "timeout" }));
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Streaming filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({ key: "key_gone", model: "gpt-5.4-mini", error: "timeout" }));
    await waitFor(() => expect(lastQuery().status).toBeUndefined());
    expect(lastQuery().stream).toBeUndefined();
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Error kind filter" }));
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Key filter" }));
    await user.click(within(toolbar).getByRole("button", { name: "Remove the Model filter" }));
    await waitFor(() => expect(router.state.location.search).toEqual({}));
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

  it("waits while Load more has focus, then takes focus to the first new request when it's pressed", async () => {
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
    const { user } = await renderApp(projectPath("logs"));

    const more = await screen.findByRole("button", { name: "Load more" });
    const watching = () => observers.filter((o) => o.targets.length > 0).at(-1);
    await waitFor(() => expect(watching()).toBeDefined());
    more.focus();
    act(() => watching()?.callback([{ isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver));
    expect(listQueries().some((q) => q.after === "cur_2")).toBe(false);
    expect(more).toHaveFocus();

    await user.keyboard("{Enter}");
    const added = await screen.findByRole("row", { name: /\/v1\/embeddings/ });
    await waitFor(() => expect(added).toHaveFocus());
    expect(screen.getByText("1 more row loaded; all rows shown")).toBeInTheDocument();
    vi.unstubAllGlobals();
  });

  it("loads by itself again once focus leaves Load more while the end of the list is in view", async () => {
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
    const { user } = await renderApp(projectPath("logs"));

    const more = await screen.findByRole("button", { name: "Load more" });
    const watching = () => observers.filter((o) => o.targets.length > 0).at(-1);
    await waitFor(() => expect(watching()).toBeDefined());
    more.focus();
    act(() => watching()?.callback([{ isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver));
    expect(listQueries().some((q) => q.after === "cur_2")).toBe(false);
    await user.tab({ shift: true });
    expect(await screen.findByRole("row", { name: /\/v1\/embeddings/ })).toBeInTheDocument();
    vi.unstubAllGlobals();
  });

  it("stops loading on its own after the next page fails, and Load more tries again", async () => {
    const observers: { live: boolean }[] = [];
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        live = false;
        constructor(public callback: IntersectionObserverCallback) {
          observers.push(this);
        }
        observe() {
          this.live = true;
          queueMicrotask(() => this.live && this.callback([{ isIntersecting: true } as IntersectionObserverEntry], this as unknown as IntersectionObserver));
        }
        disconnect() {
          this.live = false;
        }
      },
    );
    signedIn();
    projectData();
    let nextPage: Response | ReturnType<typeof listPage> = apiError(500, "internal", "The log store didn't answer.");
    on("GET", LOGS, ({ query }: { query: URLSearchParams }) => (query.get("after") === "cur_2" ? nextPage : listPage([logRow()], "cur_2")));
    const { user } = await renderApp(projectPath("logs"));
    const nextPageAsked = () => listQueries().filter((q) => q.after === "cur_2").length;

    expect(await notifications().findByText("The log store didn't answer.")).toBeInTheDocument();
    const asked = nextPageAsked();
    await act(async () => {});
    expect(nextPageAsked()).toBe(asked);
    expect(observers.filter((o) => o.live)).toHaveLength(0);
    expect(screen.getByRole("row", { name: /\/v1\/chat\/completions/ })).toBeInTheDocument();

    nextPage = listPage([logRow({ id: "req_0002", endpoint: "/v1/embeddings" })]);
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await screen.findByRole("row", { name: /\/v1\/embeddings/ })).toBeInTheDocument();
    expect(nextPageAsked()).toBe(asked + 1);
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

  it("keeps the requests that came in live after live updates are turned off", async () => {
    signedIn();
    projectData();
    let newest = [logRow()];
    on("GET", LOGS, () => listPage(newest));
    const { user } = await renderApp(projectPath("logs"));
    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    await goLive(user);

    newest = [embeddings, logRow()];
    await vi.advanceTimersByTimeAsync(5000);
    await screen.findByRole("row", { name: /\/v1\/embeddings/ });
    await user.click(screen.getByRole("button", { name: "Live" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Live" })).toHaveAttribute("aria-pressed", "false"));

    expect(shownEndpoints()).toEqual(["/v1/embeddings", "/v1/chat/completions"]);
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

  it("ends the list at the newest requests when a burst no longer reaches the ones shown", async () => {
    signedIn();
    projectData();
    on("GET", LOGS, ({ query }: { query: URLSearchParams }) => {
      if (!query.get("from")?.startsWith("2026-10-15T11:58")) return listPage([logRow()], "cur_2");
      const n = Number(query.get("after") ?? 0);
      return listPage([logRow({ id: `req_burst_${n}`, endpoint: "/v1/embeddings", started_at: "2026-10-15T11:59:00Z" })], String(n + 1));
    });
    const { user } = await renderApp(projectPath("logs"));
    await screen.findByRole("button", { name: "Load more" });
    await goLive(user);

    await vi.advanceTimersByTimeAsync(5000);
    await waitFor(() => expect(shownEndpoints()).toEqual(Array(5).fill("/v1/embeddings")));
    expect(screen.queryByRole("button", { name: "Load more" })).not.toBeInTheDocument();
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
    expect(dialog).toHaveTextContent("Every request from 1 Oct 2026 to 3 Oct 2026 that matches these filters, newest first.");
    const applied = [...dialog.querySelectorAll("dt")].map((dt) => [dt.textContent, dt.nextElementSibling?.textContent]);
    expect(applied).toEqual([
      ["Key", "checkout-web"],
      ["Member", "Grace Hopper"],
      ["Model", "Claude Sonnet 4.5"],
    ]);
    await user.click(within(dialog).getByRole("radio", { name: "JSON Lines", description: /^Metadata/ }));
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

  it("opens on CSV again after another format was picked and the dialog cancelled", async () => {
    signedIn();
    projectData();
    logsData({ rows: [logRow()] });
    const { user } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    await user.click(screen.getByRole("button", { name: "Export" }));
    const dialog = await screen.findByRole("dialog", { name: "Export requests" });
    await user.click(within(dialog).getByRole("radio", { name: "JSON Lines", description: /^Metadata/ }));
    expect(within(dialog).getByRole("radio", { name: "JSON Lines", description: /^Metadata/ })).toBeChecked();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Export requests" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Export" }));
    const again = await screen.findByRole("dialog", { name: "Export requests" });
    expect(within(again).getByRole("radio", { name: "CSV" })).toBeChecked();
    expect(within(again).getByRole("radio", { name: "JSON Lines", description: /^Metadata/ })).not.toBeChecked();
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
    expect(dialog).toHaveTextContent("Every request from the last 24 hours that matches these filters, newest first.");
    expect([...dialog.querySelectorAll("dt")].map((dt) => [dt.textContent, dt.nextElementSibling?.textContent])).toEqual([
      ["Outcome", "Failed"],
      ["Streaming", "Streamed"],
      ["Source", "API"],
    ]);
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

describe("LogsPage on an install without request logs", () => {
  it("says logs are off and never asks for them", async () => {
    signedIn({ session: whoAmI({ request_logs: false }) });
    projectData();
    await renderApp(projectPath("logs"));
    expect(await screen.findByText("Request logs are off on this install")).toBeInTheDocument();
    expect(screen.getByText("Usage, limits and budgets still count every request.")).toBeInTheDocument();
    expect(screen.queryByRole("grid", { name: "Requests" })).not.toBeInTheDocument();
    expect(requests("GET", LOGS)).toHaveLength(0);
  });
});
