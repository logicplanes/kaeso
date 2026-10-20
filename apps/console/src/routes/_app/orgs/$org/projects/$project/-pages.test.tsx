import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, onTestFinished, vi } from "vitest";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { apiKey, catalogueModel, limit, listed, modelsApi, projectData, projectMember, serviceAccount } from "~/test/fixtures/project";
import { listPage, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { auditEvent } from "~/test/fixtures/shell";
import { on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const BASE = `/orgs/${ORG}/projects/${PROJECT}`;
const sent = (path: string) => requests("GET", path).map((r) => Object.fromEntries(r.query));

describe("the project overview address", () => {
  it("breaks usage down the way the address asks", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/usage/span", { first_day: "2026-09-01", last_day: "2026-10-15" });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    await renderApp(`${BASE}?by=key&period=7d`);
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    await waitFor(() => expect(sent("orgs/:org/projects/:project/usage").map((q) => q.group_by)).toContain("key"));
    expect((await screen.findAllByText("Last 7 days")).length).toBeGreaterThan(0);
  });

  it("ignores a breakdown and period it doesn't know", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/usage/span", { first_day: null, last_day: null });
    on("GET", "orgs/:org/projects/:project/usage", { data: [] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    await renderApp(`${BASE}?by=week&period=forever`);
    await waitFor(() => expect(sent("orgs/:org/projects/:project/usage").map((q) => q.group_by)).toContain("model"));
    expect(sent("orgs/:org/projects/:project/usage").map((q) => q.group_by)).not.toContain("week");
    expect(screen.getAllByText("Last 30 days").length).toBeGreaterThan(0);
  });
});

describe("the API keys address", () => {
  it("shows archived keys of one owner and opens the create dialog", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/keys", ({ query }: { query: URLSearchParams }) =>
      query.get("after") ? listed([apiKey({ id: "key_old", name: "old-batch", state: "archived" })]) : listed([apiKey()], "cur_2"),
    );
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    await renderApp(`${BASE}/keys?scope=all&owner=user:usr_ada&create=true`);
    expect(await screen.findByRole("dialog", { name: "Create key" })).toBeInTheDocument();
    expect(await screen.findByText("old-batch")).toBeInTheDocument();
    expect(screen.getByText("checkout-web")).toBeInTheDocument();
    const filtered = sent("orgs/:org/projects/:project/keys").filter((q) => q.include_archived === "true");
    expect(filtered).toEqual(
      expect.arrayContaining([
        { user: "usr_ada", include_archived: "true", limit: "100" },
        { user: "usr_ada", include_archived: "true", limit: "100", after: "cur_2" },
      ]),
    );
  });

  it("drops values it doesn't understand", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/keys", listed([apiKey()]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    await renderApp(`${BASE}/keys?archived=maybe&create=later&key=1`);
    expect(await screen.findByText("checkout-web")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Create key" })).not.toBeInTheDocument();
    expect(sent("orgs/:org/projects/:project/keys")).toEqual([{ include_archived: "true", limit: "100" }]);
  });
});

describe("the limits address", () => {
  it("shows one level and opens the add dialog", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/limits", listed([limit(), limit({ id: "lim_member", level: "member", subject: { type: "user", id: "usr_ada" } })]));
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    await renderApp(`${BASE}/limits?level=project&add=true`);
    expect(await screen.findByRole("dialog", { name: "Add limit" })).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/limits").length).toBeGreaterThan(0);
  });

  it("shows every level when the address names an unknown one", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/limits", listed([]));
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    await renderApp(`${BASE}/limits?level=galaxy&add=1`);
    expect(await screen.findByRole("heading", { name: "No limits yet" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Add limit" })).not.toBeInTheDocument();
  });
});

describe("the request logs address", () => {
  it("sends every filter the address holds", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    const filters =
      "from=2026-10-01&to=2026-10-07&key=key_web&member=user:usr_ada&model=gpt-5&provider=openai&status=429&failed=true&error=rate_limited&stream=false&source=api";
    await renderApp(`${BASE}/logs?${filters}`);
    await screen.findByRole("heading", { name: "Request logs", level: 1 });
    await waitFor(() =>
      expect(sent("orgs/:org/projects/:project/logs")).toContainEqual({
        from: "2026-10-01T00:00:00Z",
        to: "2026-10-08T00:00:00.000Z",
        key: "key_web",
        user: "usr_ada",
        model: "gpt-5",
        provider: "openai",
        status: "429",
        failed: "true",
        error_kind: "rate_limited",
        stream: "false",
        source: "api",
        limit: "100",
      }),
    );
  });

  it("reads a service account and a range", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    const before = Date.now();
    await renderApp(`${BASE}/logs?range=7d&member=sa:sa_deploy&stream=true`);
    await waitFor(() => expect(sent("orgs/:org/projects/:project/logs").length).toBeGreaterThan(0));
    const [query] = sent("orgs/:org/projects/:project/logs");
    expect(query).toMatchObject({ service_account: "sa_deploy", stream: "true", limit: "100" });
    expect(Date.parse(query?.from ?? "")).toBeLessThanOrEqual(before - 7 * 86_400_000 + 60_000);
    expect(Date.parse(query?.from ?? "")).toBeGreaterThan(before - 8 * 86_400_000);
  });

  it("asks for the next page over the same hours as the first", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/logs", ({ query }: { query: URLSearchParams }) =>
      query.get("after") === "cur_2" ? listPage([logRow({ id: "req_0002", endpoint: "/v1/embeddings" })]) : listPage([logRow()], "cur_2"),
    );
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    const { user } = await renderApp(`${BASE}/logs?range=1h`);
    await screen.findByRole("row", { name: /\/v1\/chat\/completions/ });
    const later = Date.now() + 10 * 60_000;
    vi.spyOn(Date, "now").mockReturnValue(later);
    onTestFinished(() => void vi.restoreAllMocks());
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(await screen.findByRole("row", { name: /\/v1\/embeddings/ })).toBeInTheDocument();
    const [first, next] = sent("orgs/:org/projects/:project/logs");
    expect(next).toEqual({ from: first?.from, after: "cur_2", limit: "100" });
  });

  it("ignores filters it doesn't know instead of showing them", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    await renderApp(`${BASE}/logs?range=2h&status=abc&source=evil&from=2026-10-09&to=2026-10-01&live=nope`);
    await screen.findByRole("heading", { name: "Request logs", level: 1 });
    await waitFor(() => expect(sent("orgs/:org/projects/:project/logs").length).toBeGreaterThan(0));
    expect(Object.keys(sent("orgs/:org/projects/:project/logs")[0] ?? {}).sort()).toEqual(["from", "limit"]);
    expect(screen.queryByText("abc")).not.toBeInTheDocument();
    expect(screen.queryByText("evil")).not.toBeInTheDocument();
    expect(screen.queryByText("2h")).not.toBeInTheDocument();
  });
});

describe("the members address", () => {
  it("lists every page of members and opens the add dialog", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/members", ({ query }: { query: URLSearchParams }) =>
      query.get("after")
        ? listed([projectMember({ user: { id: "usr_linus", name: "Linus Pauling", email: "linus@example.com" } })])
        : listed([projectMember()], "cur_2"),
    );
    on("GET", "orgs/:org/members", listed([]));
    await renderApp(`${BASE}/members?add=true`);
    expect(await screen.findByRole("dialog", { name: "Add member" })).toBeInTheDocument();
    expect(await screen.findByText("Linus Pauling")).toBeInTheDocument();
    expect(screen.getByText("Grace Hopper")).toBeInTheDocument();
  });
});

describe("the service accounts address", () => {
  it("includes archived accounts and opens the create dialog", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/service-accounts", ({ query }: { query: URLSearchParams }) =>
      query.get("after")
        ? listed([serviceAccount({ id: "sa_old", name: "old-runner", archived_at: "2026-10-01T00:00:00Z" })])
        : listed([serviceAccount()], "cur_2"),
    );
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(`${BASE}/service-accounts?scope=all&create=true`);
    expect(await screen.findByRole("dialog", { name: "Create service account" })).toBeInTheDocument();
    expect(await screen.findByText("old-runner")).toBeInTheDocument();
    expect(sent("orgs/:org/projects/:project/service-accounts")).toContainEqual({ limit: "100", include_archived: "true", after: "cur_2" });
  });

  it("starts the archived accounts with the loader's request instead of the active ones", async () => {
    signedIn();
    on(
      "GET",
      "orgs/:org/projects/:project/service-accounts",
      listed([serviceAccount({ id: "sa_old", name: "old-runner", archived_at: "2026-10-01T00:00:00Z" })]),
    );
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(`${BASE}/service-accounts?scope=all`);
    expect(await screen.findByText("old-runner")).toBeInTheDocument();
    expect(sent("orgs/:org/projects/:project/service-accounts")).toEqual([{ limit: "100", include_archived: "true" }]);
  });
});

describe("the models address", () => {
  it("opens the aliases tab", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/models", { data: [catalogueModel()] });
    on("GET", "orgs/:org/projects/:project/aliases", { data: [] });
    const { router } = await renderApp(`${BASE}/models?tab=aliases`);
    await screen.findByRole("heading", { name: "Models", level: 1 });
    expect(await screen.findByRole("tab", { name: /Aliases/, selected: true })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe(`${BASE}/models`);
  });

  it("filters the catalogue by capability and leaves unknown ones out", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/models", { data: [catalogueModel()] });
    on("GET", "orgs/:org/projects/:project/aliases", { data: [] });
    await renderApp(`${BASE}/models?cap=bogus&sort=size&overridden=perhaps`);
    expect(await screen.findByText("Claude Sonnet 4.5")).toBeInTheDocument();
    expect(screen.queryByText("bogus")).not.toBeInTheDocument();
  });
});

describe("the project audit log address", () => {
  it("sends the filters and opens the event it names", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/audit-log", listed([auditEvent()]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(`${BASE}/audit-log?actor=%20usr_ada%20&action=key.created&target=key_web&from=2026-10-01&to=2026-10-02&event=evt_1`);
    await screen.findByRole("heading", { name: "Audit log", level: 1 });
    await waitFor(() =>
      expect(sent("orgs/:org/projects/:project/audit-log")).toContainEqual({
        actor: "usr_ada",
        action: "key.created",
        target: "key_web",
        from: "2026-10-01T00:00:00Z",
        to: "2026-10-03T00:00:00.000Z",
        limit: "100",
      }),
    );
    expect(await screen.findByRole("dialog", { name: "key.created" })).toBeInTheDocument();
    expect(requests("GET", "orgs/:org/audit-log/:event")).toHaveLength(0);
  });

  it("drops an empty actor and a backwards date range", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/audit-log", listed([]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    on("GET", "orgs/:org/projects/:project/keys", listed([]));
    await renderApp(`${BASE}/audit-log?actor=%20%20&from=2026-10-05&to=2026-10-01`);
    await waitFor(() => expect(sent("orgs/:org/projects/:project/audit-log")).toContainEqual({ limit: "100" }));
  });
});

describe("the project pages without search", () => {
  it("opens the playground and the settings", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/playground/models", { data: [] });
    on("GET", "orgs/:org/projects/:project/logs/settings", { content_logging: true });
    const playground = await renderApp(`${BASE}/playground`);
    expect(await screen.findByRole("heading", { name: "Playground", level: 1 })).toBeInTheDocument();
    playground.unmount();
    await renderApp(`${BASE}/settings`);
    expect(await screen.findByRole("heading", { name: "Settings", level: 1 })).toBeInTheDocument();
    expect(within(screen.getByRole("main")).getByRole("heading", { name: "Project" })).toBeInTheDocument();
  });

  it("starts the keys page with the loader's request and asks for the list once", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/keys", page([]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    await renderApp(`${BASE}/keys`);
    expect(await screen.findByRole("heading", { name: "No keys yet" })).toBeInTheDocument();
    expect(sent("orgs/:org/projects/:project/keys")).toEqual([{ include_archived: "true", limit: "100" }]);
  });

  it("starts an owner's archived keys with the loader's request", async () => {
    signedIn();
    on("GET", "orgs/:org/projects/:project/keys", listed([apiKey({ state: "archived" })]));
    on("GET", "orgs/:org/projects/:project/members", listed([]));
    on("GET", "orgs/:org/projects/:project/service-accounts", listed([]));
    await renderApp(`${BASE}/keys?owner=sa:sa_deploy&scope=all`);
    expect(await screen.findByText("checkout-web")).toBeInTheDocument();
    expect(sent("orgs/:org/projects/:project/keys")).toEqual([{ service_account: "sa_deploy", include_archived: "true", limit: "100" }]);
  });
});

describe("sheets the address opens", () => {
  it("opens the request log it names", async () => {
    signedIn();
    logsData({ rows: [logRow()], entries: [logEntry()] });
    on("GET", "orgs/:org/projects/:project/models", { data: [] });
    await renderApp(`${BASE}/logs?log=req_0001`);
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/logs/:log").map((r) => r.params.log)).toContain("req_0001"));
  });

  it("opens the model it names", async () => {
    signedIn();
    modelsApi({ models: [catalogueModel()] });
    on("GET", "orgs/:org/projects/:project/aliases", { data: [] });
    await renderApp(`${BASE}/models?model=cm_sonnet`);
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/models/:model").map((r) => r.params.model)).toContain("cm_sonnet"));
  });

  it("opens a member with their rights", async () => {
    signedIn();
    projectData({ members: [projectMember()] });
    on("GET", "orgs/:org/projects/:project/members/:user/rights", ({ params }: { params: Record<string, string> }) => ({
      project_id: PROJECT,
      user_id: params.user,
      role: { id: "developer", kind: "builtin", name: "Developer" },
      rights: [],
    }));
    await renderApp(`${BASE}/members?member=usr_grace`);
    await waitFor(() => expect(requests("GET", "orgs/:org/projects/:project/members/:user/rights").map((r) => r.params.user)).toContain("usr_grace"));
  });
});
