import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { UsageRow } from "~/api/generated/types";
import { exchangeRates, myMembership, signedIn, without } from "~/test/fixtures";
import { apiKey, catalogueModel, listed, projectData, projectMember, projectPath, serviceAccount, usageRow } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const USAGE = "orgs/:org/projects/:project/usage";
const PERIOD = "from=2026-10-01&to=2026-10-14";
const breakdown = () => screen.findByRole("grid", { name: "Usage breakdown" });
const bodyRows = (table: HTMLElement) => within(table).getAllByRole("row").slice(1);

function usage(by: Partial<Record<string, UsageRow[] | Response>> = {}): void {
  on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
  on("GET", USAGE, ({ query }: { query: URLSearchParams }) => {
    const rows = by[query.get("group_by") ?? "day"] ?? [];
    return rows instanceof Response ? rows : { data: rows };
  });
}

function onPhone() {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query === "(max-width: 639px)",
    media: query,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
  }));
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 327,
    height: 0,
    top: 0,
    left: 0,
    right: 327,
    bottom: 0,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
}

describe("AnalyticsPage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("keeps requests and cost beside the model on a phone", async () => {
    onPhone();
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 120, errors: 0, cost: "2.5" })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(1));
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((h) => h.textContent),
    ).toEqual(["Model", "Requests", "Cost"]);
    expect(bodyRows(table)[0]).toHaveTextContent("$2.50");
  });

  it("gives the model the room the number columns leave, each wide enough for its header", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5" })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const header = within(await breakdown()).getAllByRole("row")[0] as HTMLElement;
    expect(header.style.gridTemplateColumns).toBe("minmax(200px, 1fr) 96px 88px 88px 104px 96px");
  });

  it("widens the number columns for German headers and values", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 2057, input_tokens: 4_500_000, output_tokens: 10_000 })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`), { locale: "de" });

    const table = await screen.findByRole("grid", { name: "Aufschlüsselung der Nutzung" });
    await waitFor(() => expect(bodyRows(table)[0]).toHaveTextContent("4,51 Mio."));
    const header = within(table).getAllByRole("row")[0] as HTMLElement;
    expect(header.style.gridTemplateColumns).toBe("minmax(200px, 1fr) 96px 88px 96px 128px 96px");
  });

  it("widens the cost column for a count of requests without a price", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 12, cost: "2.5", unpriced: 3 })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const header = within(await breakdown()).getAllByRole("row")[0] as HTMLElement;
    expect(header.style.gridTemplateColumns).toBe("minmax(200px, 1fr) 96px 88px 88px 104px 120px");
  });

  it("keeps the requests beside the count of requests without a price on a phone", async () => {
    onPhone();
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 12, cost: "2.5", unpriced: 3 })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(1));
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((h) => h.textContent),
    ).toEqual(["Model", "Requests", "Cost"]);
    expect(bodyRows(table)[0]).toHaveTextContent("+3 unknown");
    const actions = screen.getByRole("button", { name: "Download CSV" }).parentElement;
    expect(actions).toHaveClass("max-w-full", "min-w-0", "flex-wrap");
    expect(within(actions as HTMLElement).getByRole("radiogroup", { name: "Group by" })).toBeInTheDocument();
  });

  it("breaks usage down by model with names from the catalogue", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({
      day: [usageRow({ key: "2026-10-13" })],
      model: [
        usageRow({ key: "openai/gpt-5.4-mini", requests: 40, errors: 4, cost: null }),
        usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 120, errors: 0, cost: "2.5" }),
        usageRow({ key: "anthropic/", requests: 3, errors: 0, cost: "0.001" }),
      ],
    });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(3));
    const [first, second, third] = bodyRows(table);
    expect(first).toHaveTextContent("Claude Sonnet 4.5");
    expect(first).toHaveTextContent("120");
    expect(first).toHaveTextContent("$2.50");
    expect(second).toHaveTextContent("gpt-5.4-mini");
    expect(second).toHaveTextContent("—");
    expect(third).toHaveTextContent("No model");
    expect(third).toHaveTextContent("<$0.01");
    expect(within(table).getByRole("columnheader", { name: "Model" })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "Cost" })).toBeInTheDocument();
    expect(screen.getByText("Requests, errors, tokens, cache hits and cost in the period")).toBeInTheDocument();
    expect((await screen.findAllByText("Cost by model")).length).toBeGreaterThan(0);
    expect(requests("GET", USAGE).map((r) => Object.fromEntries(r.query))).toContainEqual({ from: "2026-10-01", to: "2026-10-14", group_by: "model" });
  });

  it("breaks usage down by route with the share of the input read from the cache", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({
      route: [
        usageRow({ key: "anthropic_messages:anthropic_messages:anthropic", requests: 80, cache_hit_rate: 0.78 }),
        usageRow({ key: "openai_responses:anthropic_messages:anthropic", requests: 120, cache_hit_rate: 0 }),
        usageRow({ key: "::anthropic", requests: 5, cache_hit_rate: null }),
        usageRow({ key: "openai_embeddings:openai_embeddings:openai", requests: 3, input_tokens: 900, cache_hit_rate: null }),
      ],
    });
    await renderApp(projectPath(`analytics?${PERIOD}&by=route`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(4));
    const [first, second, third, fourth] = bodyRows(table);
    expect(first).toHaveTextContent("Responses→ Messages");
    expect(first).toHaveTextContent("0%");
    expect(second).toHaveTextContent("Messages→ Messages");
    expect(second).toHaveTextContent("78%");
    expect(third).toHaveTextContent("Route not recorded");
    expect(third).toHaveTextContent("—");
    expect(fourth).toHaveTextContent("Embeddings");
    expect(fourth).toHaveTextContent("—");
    expect(fourth).not.toHaveTextContent("0%");
    expect(within(table).getByRole("columnheader", { name: "Route" })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "Cache hit" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "By route" })).toBeChecked();
  });

  it("names media routes and shows what they made, and a cost no price covers as unknown", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    const media = { images: 0, characters: 0, audio_ms: 0, videos: 0, video_ms: 0 };
    usage({
      route: [
        usageRow({ key: "openai_images:openai_images:openai", requests: 4, cost: "0.16", media: { ...media, images: 4 } }),
        usageRow({ key: "openai_videos:veo:gemini-agent-platform", requests: 2, cost: "0", unpriced: 2, media: { ...media, videos: 2, video_ms: 16000 } }),
        usageRow({ key: "elevenlabs:elevenlabs:elevenlabs", requests: 3, cost: "0.0008", unpriced: 1, media: { ...media, characters: 120, audio_ms: 2500 } }),
        usageRow({ key: "openai_chat:openai_chat:openai", requests: 5, cost: "0.01" }),
      ],
    });
    await renderApp(projectPath(`analytics?${PERIOD}&by=route`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(4));
    const rows = bodyRows(table);
    const row = (text: string) => rows.find((r) => r.textContent?.includes(text)) as HTMLElement;
    expect(row("Images→ Images")).toHaveTextContent("4 images");
    expect(row("Videos→ Veo")).toHaveTextContent("Unknown");
    expect(row("Videos→ Veo")).toHaveTextContent(/2 videos, 16/);
    expect(row("ElevenLabs→ ElevenLabs")).toHaveTextContent("+1 unknown");
    expect(within(row("ElevenLabs→ ElevenLabs")).getByLabelText("1 request unknown")).toBeInTheDocument();
    expect(row("ElevenLabs→ ElevenLabs")).toHaveTextContent(/120 characters · .+ of audio/);
    expect(within(table).getByRole("columnheader", { name: "Media" })).toBeInTheDocument();
    expect(row("Chat→ Chat")).toHaveTextContent("—");
  });

  it("tells a replaced key from its replacement in usage by key", async () => {
    signedIn();
    projectData({ keys: [apiKey({ replaced_by: "key_next" }), apiKey({ id: "key_next", rotated_from: "key_web" })] });
    usage({ key: [usageRow({ key: "key_next", requests: 10 }), usageRow({ key: "key_web", requests: 3 })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=key`));

    const table = await breakdown();
    await waitFor(() =>
      expect(bodyRows(table).map((r) => r.textContent)).toEqual([expect.stringMatching(/^checkout-web\d/), expect.stringContaining("checkout-web (replaced)")]),
    );
  });

  it("breaks a period of up to 62 days down by client, from the hourly timeline", async () => {
    signedIn();
    projectData();
    usage({ day: [usageRow()] });
    const bucket = (key: string, requests: number) => ({
      start: "2026-10-03T10:00:00Z",
      key,
      requests,
      errors: 0,
      input_tokens: 10,
      output_tokens: 5,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      reasoning_tokens: 0,
      cost: "0.1",
      duration_p50_ms: 100,
      duration_p95_ms: 200,
      duration_max_ms: 300,
    });
    on("GET", `${USAGE}/timeline`, ({ query }: { query: URLSearchParams }) => ({
      data: query.get("group_by") === "client" ? [bucket("codex", 3), bucket("codex", 4), bucket("", 2)] : [],
    }));
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}`));
    await breakdown();
    await user.click(screen.getByRole("radio", { name: "By client" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ by: "client" }));
    const table = await breakdown();
    await waitFor(() =>
      expect(bodyRows(table).map((r) => r.textContent)).toEqual([expect.stringContaining("Codex"), expect.stringContaining("No user agent")]),
    );
    expect(bodyRows(table)[0]).toHaveTextContent("7");
    expect(within(table).getByRole("columnheader", { name: "Client" })).toBeInTheDocument();
    const asked = requests("GET", `${USAGE}/timeline`).find((r) => r.query.get("group_by") === "client");
    expect(Object.fromEntries(asked?.query ?? [])).toMatchObject({ from: "2026-10-01T00:00:00Z", to: "2026-10-15T00:00:00Z", step: "hour" });

    const files: Blob[] = [];
    const created = vi.spyOn(URL, "createObjectURL").mockImplementation((b) => (files.push(b as Blob), "blob:csv"));
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    await user.click(screen.getByRole("button", { name: "Download CSV" }));
    const lines = (await (files[0] as Blob).text()).trimEnd().split("\r\n");
    expect(lines.slice(1).map((l) => l.split(",").slice(0, 2).join(","))).toEqual(["Codex,codex", "No user agent,"]);
    created.mockRestore();
    click.mockRestore();
    // Requests without a user agent name no client the log can filter by.
    await user.click(bodyRows(table)[1] as HTMLElement);
    expect(router.state.location.pathname).toBe(projectPath("analytics"));
    await user.click(bodyRows(table)[0] as HTMLElement);
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("logs")));
    expect(router.state.location.search).toEqual({ from: "2026-10-01", to: "2026-10-14", client: "codex" });
  });

  it("offers no client breakdown beyond 62 days, and shows the days instead", async () => {
    signedIn();
    projectData();
    usage({ week: [usageRow({ key: "2026-06-01" })] });
    await renderApp(projectPath("analytics?from=2026-06-03&to=2026-09-30&by=client"));
    await breakdown();
    expect(screen.queryByRole("radio", { name: "By client" })).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "By week" })).toBeChecked();
  });

  it("groups by key, member and day from the segmented control", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()] });
    usage({
      day: [usageRow({ key: "2026-10-12", requests: 5 }), usageRow({ key: "2026-10-13", requests: 9 })],
      key: [usageRow({ key: "key_web", requests: 10 }), usageRow({ key: "playground", requests: 4 }), usageRow({ key: "key_gone", requests: 1 })],
      member: [
        usageRow({ key: "user:usr_grace", requests: 8 }),
        usageRow({ key: "service_account:sa_deploy", requests: 6 }),
        usageRow({ key: "user:usr_ada", requests: 2 }),
      ],
    });
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}`));

    await breakdown();
    await user.click(screen.getByRole("radio", { name: "By key" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ by: "key" }));
    let table = await breakdown();
    await waitFor(() =>
      expect(bodyRows(table).map((r) => r.textContent)).toEqual([
        expect.stringContaining("checkout-web"),
        expect.stringContaining("Playground"),
        expect.stringContaining("key_gone"),
      ]),
    );
    expect(within(table).getByRole("columnheader", { name: "Key" })).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "By member" }));
    table = await breakdown();
    await waitFor(() =>
      expect(bodyRows(table).map((r) => r.textContent)).toEqual([
        expect.stringContaining("Grace Hopper"),
        expect.stringContaining("ci-deploy (service account)"),
        expect.stringContaining("Ada Okafor (you)"),
      ]),
    );
    expect(within(table).getByRole("columnheader", { name: "Member" })).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: "By day" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ by: "day" }));
    table = await breakdown();
    await waitFor(() => expect(bodyRows(table).map((r) => r.textContent)).toEqual([expect.stringContaining("13 Oct"), expect.stringContaining("12 Oct")]));
    expect(within(table).getByRole("columnheader", { name: "Day" })).toBeInTheDocument();
  });

  it("says when the period has no requests, and offers a retry when the rows fail", async () => {
    signedIn();
    projectData();
    let calls = 0;
    on("GET", `${USAGE}/span`, { first_day: null, last_day: null });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "key" && ++calls === 1 ? apiError(403, "permission_denied", "Usage by key is hidden.") : { data: [] },
    );
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}&by=key`));

    expect(await screen.findByText("Couldn't load these rows")).toBeInTheDocument();
    await user.click(within(await breakdown()).getByRole("button", { name: "Try again" }));
    expect(await within(await breakdown()).findByText("No requests in this period")).toBeInTheDocument();
  });

  it("sorts the numeric columns, an unknown cost last, and downloads the rows as shown", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({
      model: [
        usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 30, cost: "0.5" }),
        usageRow({ key: "openai/=gpt-x", requests: 20, cost: null, cache_hit_rate: 0.25 }),
        usageRow({ key: "openai/gpt-5.4-mini", requests: 10, cost: "2", errors: 3 }),
      ],
    });
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}&by=model`));
    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(3));
    const order = () => bodyRows(table).map((r) => (r.textContent?.includes("Sonnet") ? "sonnet" : r.textContent?.includes("mini") ? "mini" : "x"));
    expect(within(table).getByRole("columnheader", { name: "Model" })).not.toHaveAttribute("aria-sort");

    await user.click(within(table).getByRole("button", { name: "Cost" }));
    const cost = within(table).getByRole("columnheader", { name: /Cost/ });
    expect(order()).toEqual(cost.getAttribute("aria-sort") === "ascending" ? ["sonnet", "mini", "x"] : ["mini", "sonnet", "x"]);
    await user.click(within(table).getByRole("button", { name: "Cost" }));
    expect(order()).toEqual(cost.getAttribute("aria-sort") === "ascending" ? ["sonnet", "mini", "x"] : ["mini", "sonnet", "x"]);
    await user.click(within(table).getByRole("button", { name: "Errors" }));
    expect(order()[0] === "mini" || order()[2] === "mini").toBe(true);

    const files: Blob[] = [];
    const created = vi.spyOn(URL, "createObjectURL").mockImplementation((b) => (files.push(b as Blob), "blob:csv"));
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      expect(this.download).toBe("usage-model-2026-10-01-2026-10-14.csv");
    });
    await user.click(within(table).getByRole("button", { name: "Cost" }));
    await user.click(screen.getByRole("button", { name: "Download CSV" }));
    const lines = (await (files[0] as Blob).text()).trimEnd().split("\r\n");
    expect(lines[0]).toBe("Model,Id,Requests,Errors,Tokens,Cache hit,Cost (USD)");
    expect(lines.slice(1).map((l) => l.split(",")[1])).toEqual(
      order().map((o) => ({ sonnet: "anthropic/claude-sonnet-4-5", mini: "openai/gpt-5.4-mini", x: "openai/=gpt-x" })[o]),
    );
    // A name that starts like a formula stays text in a spreadsheet.
    expect(lines.find((l) => l.includes("gpt-x"))).toBe("'=gpt-x,openai/=gpt-x,20,0,1500,0.25,");
    created.mockRestore();
    click.mockRestore();
  });

  it("downloads each grouping named as the table names it, without cost for someone who can't see it", async () => {
    signedIn({ projectRights: without("costs.view") });
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()] });
    usage({
      day: [usageRow({ key: "2026-10-13", media: { images: 2, characters: 0, audio_ms: 0, videos: 0, video_ms: 0 } })],
      key: [usageRow({ key: "key_web" }), usageRow({ key: "playground", requests: 4 })],
      member: [usageRow({ key: "service_account:sa_deploy" }), usageRow({ key: "user:usr_grace", requests: 2 })],
      route: [
        usageRow({ key: "openai_chat:anthropic_messages:anthropic" }),
        usageRow({ key: "openai_images::", requests: 2 }),
        usageRow({ key: "", requests: 1 }),
      ],
    });
    const files: Blob[] = [];
    const created = vi.spyOn(URL, "createObjectURL").mockImplementation((b) => (files.push(b as Blob), "blob:csv"));
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const { router } = await renderApp(projectPath(`analytics?${PERIOD}`));
    const column = async (by: string, wait: string) => {
      if (by !== "day") await router.navigate({ to: projectPath("analytics"), search: { from: "2026-10-01", to: "2026-10-14", by } } as never);
      await within(await breakdown()).findByText(wait);
      await userEvent.click(screen.getByRole("button", { name: "Download CSV" }));
      const lines = (await (files.at(-1) as Blob).text()).trimEnd().split("\r\n");
      return { header: lines[0], names: lines.slice(1).map((l) => l.split(",")[0]) };
    };
    expect(await column("day", "13 Oct")).toEqual({ header: "Day,Id,Requests,Errors,Tokens,Cache hit,Media", names: ["13 Oct"] });
    expect((await column("key", "checkout-web")).names).toEqual(["checkout-web", "Playground"]);
    expect((await column("member", "Grace Hopper")).names).toEqual(["ci-deploy (service account)", "Grace Hopper"]);
    expect((await column("route", "Route not recorded")).names).toEqual(["Chat → Messages (Anthropic)", "Images", "Route not recorded"]);
    created.mockRestore();
    click.mockRestore();
  });

  it("opens the requests a row counts in the period's days", async () => {
    signedIn();
    projectData({ keys: [apiKey()], members: [projectMember()], accounts: [serviceAccount()], models: [catalogueModel()] });
    usage({
      model: [usageRow({ key: "anthropic/claude-sonnet-4-5" }), usageRow({ key: "unpriced", requests: 1 })],
      key: [usageRow({ key: "key_web" }), usageRow({ key: "playground", requests: 4 })],
      member: [usageRow({ key: "service_account:sa_deploy" }), usageRow({ key: "user:usr_grace", requests: 2 })],
      route: [usageRow({ key: "openai_chat:anthropic_messages:anthropic" }), usageRow({ key: "", requests: 1 })],
    });
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}&by=model`));
    const open = async (by: string | null, row: number) => {
      if (by) {
        await router.navigate({ to: projectPath("analytics"), search: { from: "2026-10-01", to: "2026-10-14", by } } as never);
      }
      const table = await breakdown();
      await waitFor(() => expect(bodyRows(table).length).toBeGreaterThan(row));
      await user.click(bodyRows(table)[row] as HTMLElement);
    };
    const opened = async (search: Record<string, string>) => {
      await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("logs")));
      expect(router.state.location.search).toEqual({ from: "2026-10-01", to: "2026-10-14", ...search });
    };
    await open(null, 0);
    await opened({ model: "claude-sonnet-4-5", provider: "anthropic" });
    await open("model", 1);
    expect(router.state.location.pathname).toBe(projectPath("analytics"));
    await open("key", 0);
    await opened({ key: "key_web" });
    await open("key", 1);
    await opened({ source: "playground" });
    await open("member", 0);
    await opened({ member: "sa:sa_deploy" });
    await open("member", 1);
    await opened({ member: "user:usr_grace" });
    await open("route", 0);
    await opened({ ingress: "openai_chat", egress: "anthropic_messages", provider: "anthropic" });
    await open("route", 1);
    expect(router.state.location.pathname).toBe(projectPath("analytics"));
  });

  it("opens a week's requests within the period, and offers no rows to open without the request log's right", async () => {
    signedIn();
    projectData();
    usage({ week: [usageRow({ key: "2026-06-01", requests: 3 }), usageRow({ key: "2026-09-28", requests: 5 })] });
    const { user, router } = await renderApp(projectPath("analytics?from=2026-06-03&to=2026-09-30"));
    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)).toHaveLength(2));
    await user.click(bodyRows(table)[1] as HTMLElement);
    await waitFor(() => expect(router.state.location.search).toEqual({ from: "2026-06-03", to: "2026-06-07" }));
    await router.navigate({ to: projectPath("analytics"), search: { from: "2026-06-03", to: "2026-09-30" } } as never);
    const again = await breakdown();
    await waitFor(() => expect(bodyRows(again)).toHaveLength(2));
    await user.click(bodyRows(again)[0] as HTMLElement);
    await waitFor(() => expect(router.state.location.search).toEqual({ from: "2026-09-28", to: "2026-09-30" }));
  });

  it("keeps the rows a plain table for someone who can't read the request log", async () => {
    signedIn({ projectRights: without("logs.metadata.view") });
    projectData();
    usage({ day: [usageRow()] });
    await renderApp(projectPath(`analytics?${PERIOD}`));
    expect(await screen.findByRole("table", { name: "Usage breakdown" })).toBeInTheDocument();
  });

  it("offers a retry across the whole row when the rows fail without the right to see cost", async () => {
    signedIn({ projectRights: without("costs.view") });
    projectData();
    usage({ model: apiError(500, "internal", "Usage is down.") });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const table = await breakdown();
    expect(await within(table).findByText("Couldn't load these rows")).toBeInTheDocument();
    expect(within(table).getByText("Couldn't load these rows").closest("[role=gridcell]")).toHaveAttribute("aria-colspan", "5");
  });

  it("leaves cost out without the right to see it", async () => {
    signedIn({ projectRights: without("costs.view", "models.view") });
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5" })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const table = await breakdown();
    await waitFor(() => expect(bodyRows(table)[0]).toHaveTextContent("claude-sonnet-4-5"));
    expect(bodyRows(table)[0]).not.toHaveTextContent("Claude Sonnet 4.5");
    expect(within(table).queryByRole("columnheader", { name: "Cost" })).not.toBeInTheDocument();
    expect(screen.getByText("Requests, errors, tokens and cache hits in the period")).toBeInTheDocument();
    expect(screen.queryByText("Cost by model")).not.toBeInTheDocument();
    expect(requests("GET", "orgs/:org/projects/:project/models")).toHaveLength(0);
  });

  it("reads usage from the first day when the whole span is chosen", async () => {
    signedIn();
    projectData();
    usage();
    await renderApp(projectPath("analytics?period=all"));

    await breakdown();
    await waitFor(() => expect(requests("GET", USAGE).map((r) => r.query.get("from"))).toContain("2026-09-01"));
    const month = `${new Date().toISOString().slice(0, 8)}01`;
    expect(requests("GET", USAGE).every((r) => ["2026-09-01", month].includes(r.query.get("from") ?? ""))).toBe(true);
  });

  it("loads the daily usage again from a chart that couldn't load", async () => {
    signedIn();
    projectData();
    let failing = true;
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "day" && failing
        ? apiError(403, "permission_denied", "Usage is hidden for now.")
        : { data: [usageRow({ key: "2026-10-13", requests: 7 })] },
    );
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}&by=day`));

    const requestsChart = (await screen.findByRole("heading", { name: "Requests per day, UTC" })).closest("section") as HTMLElement;
    expect(await within(requestsChart).findByText("Couldn't load this chart")).toBeInTheDocument();
    expect(await within(await breakdown()).findByText("Couldn't load these rows")).toBeInTheDocument();
    expect(screen.getAllByText("Couldn't load").length).toBeGreaterThanOrEqual(3);
    failing = false;
    await user.click(within(requestsChart).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(within(requestsChart).queryByText("Couldn't load this chart")).not.toBeInTheDocument());
    await waitFor(() => expect(bodyRows(screen.getByRole("grid", { name: "Usage breakdown" }))[0]).toHaveTextContent("13 Oct"));
  });

  it("loads the daily rows again from the breakdown", async () => {
    signedIn();
    projectData();
    let failing = true;
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) =>
      query.get("group_by") === "day" && failing
        ? apiError(403, "permission_denied", "Usage is hidden for now.")
        : { data: [usageRow({ key: "2026-10-13", requests: 7 })] },
    );
    const { user } = await renderApp(projectPath(`analytics?${PERIOD}&by=day`));

    expect(await within(await breakdown()).findByText("Couldn't load these rows")).toBeInTheDocument();
    failing = false;
    await user.click(within(await breakdown()).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(bodyRows(screen.getByRole("grid", { name: "Usage breakdown" }))[0]).toHaveTextContent("13 Oct"));
  });

  it("charts the weeks of a period longer than a quarter, and names the year of each when it spans two", async () => {
    signedIn();
    projectData();
    on("GET", `${USAGE}/span`, { first_day: "2025-10-01", last_day: "2026-10-14" });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) => ({
      data: query.get("group_by") === "week" ? [usageRow({ key: "2025-10-20", requests: 3 }), usageRow({ key: "2026-10-12", requests: 7 })] : [],
    }));
    await renderApp(projectPath("analytics?from=2025-10-21&to=2026-10-14&by=day"));
    await breakdown();
    await waitFor(() =>
      expect(bodyRows(screen.getByRole("grid", { name: "Usage breakdown" })).map((r) => within(r).getAllByRole("gridcell")[0]?.textContent)).toEqual([
        "Week of 12 Oct 2026",
        "Week of 20 Oct 2025",
      ]),
    );
  });

  it("shows every amount in the organization's currency, with the rates it used", async () => {
    signedIn({ memberships: [myMembership({ org_currency: "EUR" })] });
    projectData();
    usage({
      day: [usageRow({ key: "2026-10-13", cost: "2", converted_cost: "1.8" }), usageRow({ cost: "1.25", converted_cost: "1.15", currency: "EUR" })],
      model: [usageRow({ key: "openai/gpt-5.4-mini", cost: "3.25", converted_cost: "2.95", currency: "EUR" })],
    });
    await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByRole("tab", { name: /Cost.*€2\.95/ })).toBeInTheDocument();
    expect(screen.getByText("In EUR at each day's ECB rate; the latest is of 13 Nov 2026.")).toBeInTheDocument();
    const table = await breakdown();
    await waitFor(() => expect(within(table).getByText("€1.15")).toBeInTheDocument());
    expect(document.body).not.toHaveTextContent("$");
    expect(requests("GET", USAGE).some((r) => r.query.has("currency"))).toBe(false);
  });

  it("says which days use the latest rate when the period starts before the oldest rates", async () => {
    signedIn({ memberships: [myMembership({ org_currency: "GBP" })] });
    on("GET", "me/exchange-rates", exchangeRates({ first_day: "2026-10-05" }));
    projectData();
    usage({ day: [usageRow({ converted_cost: "0.94", currency: "GBP" })] });
    await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByText("In GBP at each day's ECB rate; before 5 Oct 2026 at the rate of 13 Nov 2026.")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Cost.*£0\.94/ })).toBeInTheDocument();
  });

  it("shows plain dollars without a note about rates in an organization that counts in US dollars", async () => {
    signedIn();
    projectData();
    usage({ day: [usageRow({ cost: "1.25" })] });
    await renderApp(projectPath(`analytics?${PERIOD}`));

    expect(await screen.findByRole("tab", { name: /Cost.*\$1\.25/ })).toBeInTheDocument();
    expect(screen.queryByText(/ECB rate/)).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Currency" })).not.toBeInTheDocument();
  });

  it("chooses a period from the picker", async () => {
    signedIn();
    projectData();
    usage();
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}`));

    await breakdown();
    await user.click(screen.getByRole("button", { description: "Period" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose a date range" });
    await user.click(within(dialog).getByRole("option", { name: "Last 7 days" }));
    await waitFor(() => expect(router.state.location.search).toMatchObject({ period: "7d" }));
  });
});

describe("the analytics views", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("breaks the period down by model with shares, movers and a table that opens the request logs", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    on("GET", "orgs/:org/projects/:project/logs", listed([]));
    on("GET", `${USAGE}/span`, { first_day: "2026-09-01", last_day: "2026-10-14" });
    on("GET", `${USAGE}/timeline`, { data: [] });
    on("GET", USAGE, ({ query }: { query: URLSearchParams }) => {
      if (query.get("group_by") !== "model") return { data: [] };
      return query.get("from") === "2026-10-01"
        ? {
            data: [
              usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 30, errors: 3, cost: "3" }),
              usageRow({ key: "openai/gpt-5.4-mini", requests: 10, cost: "1" }),
            ],
          }
        : { data: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 20 }), usageRow({ key: "openai/gpt-5.4-mini", requests: 20 })] };
    });
    const { user, router } = await renderApp(projectPath(`analytics/models?${PERIOD}`));

    expect(await screen.findByRole("tab", { name: "Models", selected: true })).toBeInTheDocument();
    const table = await screen.findByRole("grid", { name: "Models in the period" });
    await waitFor(() => expect(bodyRows(table)).toHaveLength(2));
    expect(bodyRows(table)[0]).toHaveTextContent(/Claude Sonnet 4\.5.*30.*↑50%.*75%.*10%/);
    expect(screen.getByRole("list", { name: "Grew the most" })).toHaveTextContent(/Claude Sonnet 4\.5.*↑50%.*\+10/);
    expect(screen.getByRole("list", { name: "Shrank the most" })).toHaveTextContent(/gpt-5\.4-mini.*↓50%.*−10/);

    await user.click(bodyRows(table)[1] as HTMLElement);
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("logs")));
    // The model with its provider, in the period's days.
    expect(router.state.location.searchStr).toBe("?model=gpt-5.4-mini&provider=openai&from=2026-10-01&to=2026-10-14&utc=true");
  });

  it("sizes the Models view's columns for German headers and values", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5", input_tokens: 129_400, output_tokens: 500 })] });
    await renderApp(projectPath(`analytics/models?${PERIOD}`), { locale: "de" });

    const table = await screen.findByRole("grid", { name: "Modelle im Zeitraum" });
    await waitFor(() => expect(bodyRows(table)[0]).toHaveTextContent("129,9 Tsd."));
    const header = within(table).getAllByRole("row")[0] as HTMLElement;
    expect(header.style.gridTemplateColumns).toBe("minmax(220px, 1fr) 96px 96px 84px 112px 104px 112px 128px 96px 112px");
  });

  it("keeps the period when it moves between views", async () => {
    signedIn();
    projectData();
    usage({});
    on("GET", `${USAGE}/timeline`, { data: [] });
    const { user, router } = await renderApp(projectPath(`analytics?${PERIOD}&by=key`));
    await user.click(await screen.findByRole("tab", { name: "Members" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(projectPath("analytics/members")));
    expect(router.state.location.search).toEqual({ from: "2026-10-01", to: "2026-10-14" });
  });
});
