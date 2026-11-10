import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { UsageRow } from "~/api/generated/types";
import { signedIn, without } from "~/test/fixtures";
import { apiKey, catalogueModel, listed, projectData, projectMember, projectPath, serviceAccount, usageRow } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const USAGE = "orgs/:org/projects/:project/usage";
const PERIOD = "from=2026-10-01&to=2026-10-14";
const breakdown = () => screen.findByRole("table", { name: "Usage breakdown" });
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

  it("gives the model the room the number columns leave", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5" })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const header = within(await breakdown()).getAllByRole("row")[0] as HTMLElement;
    expect(header.style.gridTemplateColumns).toBe("minmax(200px, 1fr) 88px 88px 88px 88px 96px");
  });

  it("widens the cost column for a count of requests without a price", async () => {
    signedIn();
    projectData({ models: [catalogueModel()] });
    usage({ model: [usageRow({ key: "anthropic/claude-sonnet-4-5", requests: 12, cost: "2.5", unpriced: 3 })] });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const header = within(await breakdown()).getAllByRole("row")[0] as HTMLElement;
    expect(header.style.gridTemplateColumns).toBe("minmax(200px, 1fr) 88px 88px 88px 88px 120px");
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

  it("offers a retry across the whole row when the rows fail without the right to see cost", async () => {
    signedIn({ projectRights: without("costs.view") });
    projectData();
    usage({ model: apiError(500, "internal", "Usage is down.") });
    await renderApp(projectPath(`analytics?${PERIOD}&by=model`));

    const table = await breakdown();
    expect(await within(table).findByText("Couldn't load these rows")).toBeInTheDocument();
    expect(within(table).getByText("Couldn't load these rows").closest("[role=cell]")).toHaveAttribute("aria-colspan", "5");
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
    await waitFor(() => expect(bodyRows(screen.getByRole("table", { name: "Usage breakdown" }))[0]).toHaveTextContent("13 Oct"));
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
    await waitFor(() => expect(bodyRows(screen.getByRole("table", { name: "Usage breakdown" }))[0]).toHaveTextContent("13 Oct"));
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
      expect(bodyRows(screen.getByRole("table", { name: "Usage breakdown" })).map((r) => within(r).getAllByRole("cell")[0]?.textContent)).toEqual([
        "Week of 12 Oct 2026",
        "Week of 20 Oct 2025",
      ]),
    );
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
    expect(router.state.location.searchStr).toBe("?model=gpt-5.4-mini");
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
