import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { UsageRow } from "~/api/generated/types";
import { atFixtureTime } from "~/test/clock";
import { signedIn, whoAmI, without } from "~/test/fixtures";
import { apiKey, catalogueModel, limit, listed, me, projectData, projectMember, projectPath, serviceAccount, usageRow } from "~/test/fixtures/project";
import { logRow } from "~/test/fixtures/project-logs";
import { auditEvent } from "~/test/fixtures/shell";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const USAGE = "orgs/:org/projects/:project/usage";

function usage(byFrom: Record<string, UsageRow[]>, byGroup: Partial<Record<string, UsageRow[]>> = {}): void {
  on("GET", USAGE, ({ query }: { query: URLSearchParams }) => {
    const group = query.get("group_by") ?? "day";
    if (group !== "day") return { data: byGroup[group] ?? [] };
    return { data: byFrom[query.get("from") ?? ""] ?? [] };
  });
  on("GET", `${USAGE}/timeline`, { data: [] });
}

const cell = (title: string) => screen.getByRole("link", { name: title }).closest("section") as HTMLElement;

describe("the project overview", () => {
  atFixtureTime();

  it("shows the last 14 days against the 14 before, and the last 7 days when asked", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage({
      "2026-10-02": [usageRow({ key: "2026-10-14", requests: 40, errors: 2, cost: "1.5", cache_read_tokens: 1000 })],
      "2026-09-18": [usageRow({ key: "2026-09-20", requests: 20, errors: 2, cost: "1", cache_read_tokens: 500 })],
    });
    const { user } = await renderApp(projectPath());

    expect(await screen.findByRole("heading", { name: "Activity", level: 2 })).toBeInTheDocument();
    await waitFor(() => expect(cell("Requests")).toHaveTextContent("40"));
    expect(cell("Requests")).toHaveTextContent("↑100%vs. the 14 days before");
    expect(cell("Failed requests")).toHaveTextContent("↓50% better5% of requests");
    expect(cell("Cache hit")).toHaveTextContent(/50%.*vs\. the 14 days before/);
    expect(screen.getByRole("link", { name: "Requests" })).toHaveAttribute("href", `${projectPath("analytics")}?period=30d&metric=requests`);

    await user.click(screen.getByRole("radio", { name: "7d" }));
    await waitFor(() => expect(requests("GET", USAGE).map((r) => r.query.get("from"))).toContain("2026-10-09"));
    expect(screen.getByRole("link", { name: "Requests" })).toHaveAttribute("href", `${projectPath("analytics")}?period=7d&metric=requests`);
  });

  it("counts the requests the cost leaves out because no price covers them", async () => {
    signedIn();
    projectData();
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage({ "2026-10-02": [usageRow({ key: "2026-10-14", requests: 40, cost: "1.5", unpriced: 2 })] });
    await renderApp(projectPath());

    await waitFor(() => expect(cell("Cost")).toHaveTextContent(/\$1\.50.*2 requests unknown/));
  });

  it("lists the latest requests with their models and keys, and opens one in the request logs", async () => {
    signedIn();
    projectData({ keys: [apiKey()] });
    usage({});
    on("GET", "orgs/:org/projects/:project/logs", listed([logRow()]));
    const { user, router } = await renderApp(projectPath());

    const latest = await screen.findByRole("list", { name: "Latest requests" });
    const row = await within(latest).findByRole("button", { name: /checkout-web/ });
    expect(row).toHaveTextContent("200");
    expect(row).toHaveTextContent("gpt-5.4-mini");
    expect(row).toHaveTextContent("2 min ago");
    expect(row).toHaveTextContent("1.3 s");
    await user.click(row);
    await waitFor(() => expect(router.state.location.searchStr).toBe("?log=req_0001"));
    expect(router.state.location.pathname).toBe(projectPath("logs"));
  });

  it("shows the project's limits, its models by use, its keys by last use and its members", async () => {
    signedIn();
    projectData({
      limits: [limit()],
      models: [
        catalogueModel(),
        catalogueModel({ id: "cm_mini", provider: "openai", model_id: "gpt-5.4-mini" }),
        catalogueModel({ id: "cm_b", provider: "openai", model_id: "b-model", effective: { ...catalogueModel().effective, display_name: "b-model" } }),
        catalogueModel({ id: "cm_a", provider: "openai", model_id: "a-model", effective: { ...catalogueModel().effective, display_name: "a-model" } }),
      ],
      keys: [
        apiKey({ id: "key_old", name: "old-batch", last_used_at: null }),
        apiKey({ last_used_at: "2026-10-15T11:00:00Z" }),
        apiKey({ id: "key_idle", name: "idle", last_used_at: null }),
      ],
      members: [projectMember()],
    });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage({}, { model: [usageRow({ key: "openai/gpt-5.4-mini", requests: 30 }), usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 10 })] });
    await renderApp(projectPath());

    const models = await screen.findByRole("list", { name: "Models" });
    await waitFor(() => expect(within(models).getAllByRole("button")[0]).toHaveTextContent("75%"));
    expect(within(models).getAllByRole("button")[0]).toHaveTextContent("gpt-5.4-mini");
    expect(
      within(models)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual([
      expect.stringContaining("gpt-5.4-mini"),
      expect.stringContaining("Claude Sonnet 4.5"),
      expect.stringContaining("a-model"),
      expect.stringContaining("b-model"),
    ]);
    const keys = screen.getByRole("list", { name: "API keys" });
    expect(within(keys).getAllByRole("button")[0]).toHaveTextContent("checkout-web");
    expect(await screen.findByRole("list", { name: "Members" })).toHaveTextContent("Developer");
    const limits = screen.getByRole("heading", { name: "Limits" }).closest("section") as HTMLElement;
    expect(within(limits).getByText("Hard limit")).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "Manage" }).map((l) => l.getAttribute("href"))).toEqual([
      projectPath("limits"),
      projectPath("models"),
      projectPath("keys"),
      projectPath("members"),
    ]);
  });

  it("words every kind of limit by its window", async () => {
    signedIn();
    projectData({
      limits: [
        limit({ id: "lim_rate", kind: "request_rate", period: "minute", amount: "600", used: "12", hard: false }),
        limit({ id: "lim_tokens", kind: "token_budget", period: "day", amount: "1000000", used: null, resets_at: null }),
      ],
    });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage({});
    await renderApp(projectPath());
    const limits = (await screen.findByRole("heading", { name: "Limits" })).closest("section") as HTMLElement;
    await waitFor(() => expect(limits).toHaveTextContent("Over the last minute"));
    expect(limits).toHaveTextContent("12 of 600");
    expect(limits).toHaveTextContent("0 of 1,000,000");
    expect(limits).toHaveTextContent("Never resets");
  });

  it("says when there are no limits, and when the latest requests couldn't load", async () => {
    signedIn();
    projectData();
    usage({});
    on("GET", "orgs/:org/projects/:project/logs", () => apiError(500, "api_error", "The log store is down."));
    await renderApp(projectPath());
    expect(await screen.findByText("No project limits. Keys and members can still have their own.")).toBeInTheDocument();
    expect(await screen.findByText("The latest requests couldn't load; the notification says why.")).toBeInTheDocument();
  });

  it("says why the limits couldn't load when no notification did", async () => {
    signedIn();
    projectData();
    usage({});
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on("GET", "orgs/:org/projects/:project/limits", () => apiError(404, "not_found", "This project's limits are gone."));
    await renderApp(projectPath());
    expect(await screen.findByText("This project's limits are gone.")).toBeInTheDocument();
  });

  it("names requests from the playground, from keys it doesn't know and to models no provider answered", async () => {
    signedIn();
    projectData({
      keys: [apiKey()],
      models: [catalogueModel({ model_id: "gpt-5.4-mini", provider: "openai", effective: { ...catalogueModel().effective, display_name: "GPT mini" } })],
    });
    usage({});
    on(
      "GET",
      "orgs/:org/projects/:project/logs",
      listed([logRow({ id: "req_play", key_id: null }), logRow({ id: "req_gone", key_id: "key_deleted", provider: "", model: "", requested_model: "fast" })]),
    );
    await renderApp(projectPath());
    const latest = await screen.findByRole("list", { name: "Latest requests" });
    await waitFor(() => expect(within(latest).getAllByRole("button")).toHaveLength(2));
    const [play, gone] = within(latest).getAllByRole("button");
    expect(play).toHaveTextContent(/GPT mini.*Playground.*1\.3 s/);
    expect(gone).toHaveTextContent("fast");
    expect(gone).not.toHaveTextContent("key_deleted");
  });

  it("opens models, keys and members from their lists", async () => {
    signedIn();
    projectData({
      models: [catalogueModel()],
      keys: [apiKey()],
      members: [me({ user: { avatar_url: "https://avatars.example/ada.png" } }), projectMember()],
    });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    usage({});
    const { user, router } = await renderApp(projectPath());

    await user.click(await within(await screen.findByRole("list", { name: "Models" })).findByRole("button", { name: /Claude Sonnet 4\.5/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("models")));
    expect(router.state.location.searchStr).toBe("?provider=anthropic");

    await router.navigate({ to: projectPath() });
    await user.click(await within(await screen.findByRole("list", { name: "API keys" })).findByRole("button", { name: /checkout-web/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("keys")));
    expect(router.state.location.searchStr).toBe("?key=key_web");

    await router.navigate({ to: projectPath() });
    const members = await screen.findByRole("list", { name: "Members" });
    expect(await within(members).findByRole("button", { name: /Ada Okafor \(you\)/ })).toBeInTheDocument();
    await user.click(within(members).getByRole("button", { name: /Grace Hopper/ }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("members")));
  });

  it("lists the project's recent changes by who made them", async () => {
    signedIn();
    projectData({ accounts: [serviceAccount()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on(
      "GET",
      "orgs/:org/projects/:project/audit-log",
      listed([
        auditEvent({ action: "key.create", actor: { kind: "service_account", id: serviceAccount().id } }),
        auditEvent({ id: "evt_2", action: "key.create" }),
      ]),
    );
    usage({});
    const { user, router } = await renderApp(projectPath());
    const changes = await screen.findByRole("list", { name: "Recent changes" });
    const rows = await within(changes).findAllByRole("button", { name: /Key created/ });
    expect(rows).toHaveLength(2);
    await user.click(rows[0] as HTMLElement);
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("audit-log")));
    expect(router.state.location.searchStr).toBe("?event=evt_1");
  });

  it("gives the latest requests the whole row without the right to see limits", async () => {
    signedIn({ projectRights: without("limits.view", "project.audit_log.view") });
    projectData();
    usage({});
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    await renderApp(projectPath());
    expect(await screen.findByRole("heading", { name: "Latest requests" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Limits" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Recent changes" })).not.toBeInTheDocument();
  });

  it("leaves out the latest requests when request logs are off", async () => {
    signedIn({ session: whoAmI({ request_logs: false }) });
    projectData();
    usage({});
    await renderApp(projectPath());
    expect(await screen.findByRole("heading", { name: "Limits" })).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Latest requests" })).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/logs")).toHaveLength(0);
  });
});
