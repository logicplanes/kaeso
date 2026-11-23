import { describe, expect, it } from "vitest";
import { screen, within } from "@testing-library/react";
import type { SharedRow, SharedView } from "~/api/generated/types";
import { signedOut } from "~/test/fixtures";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { forecast, rowLabel } from "./shared-page";

const TOKEN = "9fQ2kX7mVb3pL8rT1wZ5yH4uJ6sD0gA2cE7nM1qR3tU";

const row = (overrides: Partial<SharedRow> = {}): SharedRow => ({
  key: "2026-10-14",
  name: null,
  number: null,
  requests: 120,
  errors: 6,
  input_tokens: 1000,
  output_tokens: 500,
  cache_read_tokens: 500,
  cache_write_tokens: 0,
  reasoning_tokens: 0,
  cost: null,
  tool_calls: 0,
  ...overrides,
  converted_cost: overrides.converted_cost !== undefined ? overrides.converted_cost : (overrides.cost ?? null),
});

// The end of today's day budget, UTC, so the day is under way whenever the test runs.
const tomorrow = () => `${new Date(Date.now() + 86_400_000).toISOString().slice(0, 10)}T00:00:00Z`;

const view = (overrides: Partial<SharedView> = {}): SharedView => ({
  kind: "analytics",
  title: "Checkout usage",
  organization: "Northwind Labs",
  project: "Checkout assistant",
  member: null,
  live: true,
  from: "2026-10-13",
  to: "2026-10-14",
  metric: "requests",
  costs: false,
  currency: "USD",
  created_at: "2026-10-14T09:00:00Z",
  expires_at: "2026-11-13T09:00:00Z",
  made_with_kaeso: true,
  days: [row({ key: "2026-10-13", requests: 80 }), row()],
  previous_days: [row({ key: "2026-10-12", requests: 100 })],
  breakdowns: [
    { by: "model", rows: [row({ key: "openai/gpt-5.4-mini", requests: 150 }), row({ key: "anthropic/claude-sonnet-4-5", requests: 50 })] },
    { by: "member", rows: [row({ key: null, number: 1, requests: 190 }), row({ key: null, name: "Ada Okafor", requests: 10 })] },
  ],
  limits: [],
  ...overrides,
});

describe("a shared link's page", () => {
  it("shows the shared usage to someone without an account, with hidden names as numbers", async () => {
    signedOut();
    on("GET", "share/:token", view());
    await renderApp(`/share/${TOKEN}`);

    expect(await screen.findByRole("heading", { name: "Checkout usage", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("Live")).toBeInTheDocument();
    expect(screen.getByText(/^Northwind Labs · Checkout assistant · 13 Oct 2026 to 14 Oct 2026$/)).toBeInTheDocument();
    const tiles = screen.getByRole("tablist", { name: "Metric to chart" });
    expect(within(tiles).getByRole("tab", { name: /Requests\s*200/ })).toBeInTheDocument();
    expect(within(tiles).queryByRole("tab", { name: /Cost/ })).not.toBeInTheDocument();
    expect(screen.getByText("Member 1")).toBeInTheDocument();
    expect(screen.getByText("Ada Okafor")).toBeInTheDocument();
    expect(screen.getByText("openai/gpt-5.4-mini")).toBeInTheDocument();
    expect(screen.getByText("A read-only view shared from Kaeso, until 13 Nov 2026.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /^Made with Kaeso/ })).toHaveAttribute("href", "https://kaeso.app");
    expect(screen.getByRole("link", { name: /^Made with Kaeso/ }).parentElement).toHaveClass("text-caption");
    expect(screen.getByRole("link", { name: /^Made with Kaeso/ })).toHaveClass("tap-target");
    expect(screen.getByText("Live").parentElement).toBe(screen.getByRole("heading", { level: 1 }).parentElement);
    expect(screen.getByText("Live").parentElement).not.toHaveClass("flex-wrap");
    expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(expect.arrayContaining(["Models", "Members"]));
    expect(requests("GET", "share/:token").map((r) => r.path)).toEqual([`/platform/v1/share/${TOKEN}`]);
  });

  it("shows a project's summary of one day with its top five models, charting requests for a metric it can't show", async () => {
    signedOut();
    const models = ["a", "b", "c", "d", "e", "f"].map((m, i) => row({ key: `openai/model-${m}`, requests: 60 - i }));
    on(
      "GET",
      "share/:token",
      view({
        kind: "project_card",
        metric: "cost",
        from: "2026-10-14",
        to: "2026-10-14",
        breakdowns: [
          { by: "model", rows: models },
          { by: "tool", rows: [] },
        ],
      }),
    );
    await renderApp(`/share/${TOKEN}`);
    expect(await screen.findByText(/^Northwind Labs · Checkout assistant · 14 Oct 2026$/)).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Requests/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("openai/model-e")).toBeInTheDocument();
    expect(screen.queryByText("openai/model-f")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "tool" })).toBeInTheDocument();
    expect(screen.getByText("Nothing in this period")).toBeInTheDocument();
  });

  it("gives the last of an odd number of breakdowns the whole row, so no empty slot is left", async () => {
    signedOut();
    const one = [row({ key: "x" })];
    on("GET", "share/:token", view({ project: null, breakdowns: ["key", "route", "client"].map((by) => ({ by, rows: one })) }));
    await renderApp(`/share/${TOKEN}`);
    const cell = (name: string) => screen.getByRole("heading", { name }).closest("section");
    expect(await screen.findByRole("heading", { name: "Clients" })).toBeInTheDocument();
    expect(cell("Keys")).toHaveClass("lg:col-span-1");
    expect(cell("Routes")).toHaveClass("lg:col-span-1");
    expect(cell("Clients")).toHaveClass("lg:col-span-2");
  });

  it("titles each breakdown by what it splits the usage by", async () => {
    signedOut();
    const one = [row({ key: "x" })];
    on("GET", "share/:token", view({ project: null, breakdowns: ["key", "route", "project", "client"].map((by) => ({ by, rows: one })) }));
    await renderApp(`/share/${TOKEN}`);
    for (const name of ["Keys", "Routes", "Projects", "Clients"]) expect(await screen.findByRole("heading", { name })).toBeInTheDocument();
  });

  it("charts another metric when one is picked, and costs only when the link shows them", async () => {
    signedOut();
    on("GET", "share/:token", view({ costs: true, made_with_kaeso: false, expires_at: null, live: false, days: [row({ cost: "1.5" })] }));
    const { user } = await renderApp(`/share/${TOKEN}`);

    expect(await screen.findByText("Snapshot")).toBeInTheDocument();
    expect(screen.getByText("A read-only view shared from Kaeso.")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /^Made with Kaeso/ })).not.toBeInTheDocument();
    const cost = screen.getByRole("tab", { name: /Cost/ });
    await user.click(cost);
    expect(cost).toHaveAttribute("aria-selected", "true");
  });

  it("shows cost and budgets in the organization's currency, never in dollars", async () => {
    signedOut();
    on(
      "GET",
      "share/:token",
      view({
        costs: true,
        currency: "EUR",
        metric: "cost",
        days: [row({ key: "2026-10-13", cost: "11", converted_cost: "10.08" }), row({ cost: "2.2", converted_cost: "2.02" })],
        previous_days: [],
      }),
    );
    await renderApp(`/share/${TOKEN}`);

    expect(await screen.findByRole("tab", { name: /Cost.*€12\.10/ })).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent("$");
  });

  it("shows a spend budget in the currency it was set in", async () => {
    signedOut();
    on(
      "GET",
      "share/:token",
      view({
        kind: "budget",
        costs: true,
        currency: "EUR",
        breakdowns: [],
        days: [],
        previous_days: [],
        limits: [
          {
            level: "project",
            name: null,
            number: null,
            kind: "spend_budget",
            model: null,
            period: "total",
            amount: "40",
            currency: "EUR",
            used: "12.1",
            hard: true,
            resets_at: null,
          },
        ],
      }),
    );
    await renderApp(`/share/${TOKEN}`);

    expect(await screen.findByText("€12.10 of €40.00")).toBeInTheDocument();
  });

  it("shows a project's budgets with what they used and where they're heading", async () => {
    signedOut();
    on(
      "GET",
      "share/:token",
      view({
        kind: "budget",
        title: "Checkout budgets",
        breakdowns: [],
        days: [],
        previous_days: [],
        limits: [
          {
            level: "project",
            name: null,
            number: null,
            kind: "spend_budget",
            model: null,
            period: "month",
            amount: "50",
            currency: "USD",
            used: "12.5",
            hard: true,
            resets_at: "2026-11-01T00:00:00Z",
          },
          {
            level: "member",
            name: null,
            number: 1,
            kind: "request_rate",
            model: "openai/gpt-5.4-mini",
            period: "minute",
            amount: "60",
            currency: null,
            used: "6",
            hard: true,
            resets_at: null,
          },
          {
            level: "key",
            name: "checkout-web",
            number: null,
            kind: "token_budget",
            model: null,
            period: "total",
            amount: "1000",
            currency: null,
            used: null,
            hard: false,
            resets_at: null,
          },
          {
            level: "key",
            name: null,
            number: 2,
            kind: "token_budget",
            model: null,
            period: "day",
            amount: "100000000",
            currency: null,
            used: "10624184",
            hard: true,
            resets_at: tomorrow(),
          },
        ],
      }),
    );
    await renderApp(`/share/${TOKEN}`);

    expect(await screen.findByText("The project · Spend budget")).toBeInTheDocument();
    expect(screen.getByText("$12.50 of $50.00")).toBeInTheDocument();
    expect(screen.getByText("Member 1 · Request rate")).toBeInTheDocument();
    expect(screen.getByText("6 of 60")).toBeInTheDocument();
    expect(screen.getByText("checkout-web · Token budget")).toBeInTheDocument();
    expect(screen.getByText("Only alerts once it's used up")).toBeInTheDocument();
    expect(screen.getByText("Key 2 · Token budget")).toBeInTheDocument();
    expect(screen.getByText("10.6M of 100M")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByText("10,624,184 of 100,000,000")).toBeInTheDocument();
    expect(screen.getAllByRole("meter").map((m) => m.getAttribute("aria-valuetext"))).toContain("10,624,184 of 100,000,000");
    expect(screen.getByText(/^Heading for \d+(\.\d+)?[MB] at this pace$/)).toBeInTheDocument();
  });

  it("says when the project has nothing to show", async () => {
    signedOut();
    on("GET", "share/:token", view({ kind: "budget", limits: [] }));
    await renderApp(`/share/${TOKEN}`);
    expect(await screen.findByText("The project has no limits or budgets that this link shows.")).toBeInTheDocument();
  });

  it("asks a member to sign in for a members-only link", async () => {
    signedOut();
    on("GET", "share/:token", apiError(401, "authentication", "This link is for the organization's members; sign in to see it."));
    await renderApp(`/share/${TOKEN}`);
    expect(await screen.findByRole("heading", { name: "This link is for an organization's members", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute("href", `/sign-in?next=${encodeURIComponent(`/share/${TOKEN}`)}`);
  });

  it("says a link that expired or was revoked doesn't work", async () => {
    signedOut();
    on("GET", "share/:token", apiError(404, "not_found", "This shared link doesn't exist, has expired or was revoked."));
    await renderApp(`/share/${TOKEN}`);
    expect(await screen.findByRole("heading", { name: "This link doesn't work", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 }).parentElement?.lastElementChild).toHaveTextContent(
      "It may have expired or been revoked; ask whoever shared it for a new one.",
    );
  });
});

describe("shared rows", () => {
  it("are named by what they are", () => {
    expect(rowLabel("key", row({ key: "playground" }))).toBe("Playground");
    expect(rowLabel("key", row({ key: null, number: 2 }))).toBe("Key 2");
    expect(rowLabel("key", row({ key: "source", number: 4 }))).toBe("Source 4");
    expect(rowLabel("project", row({ key: null, number: 3 }))).toBe("Project 3");
    expect(rowLabel("client", row({ key: "" }))).toBe("No user agent");
    expect(rowLabel("route", row({ key: "openai_chat:anthropic_messages:anthropic" }))).toBe("OpenAI Chat Completions to Anthropic Messages at anthropic");
    expect(rowLabel("model", row({ key: null }))).toBe("Unknown");
  });

  it("forecast a day or month budget at its pace, and nothing else", () => {
    const budget = { level: "project", kind: "spend_budget", model: null, amount: "50", hard: true, name: null, number: null };
    const now = Date.parse("2026-10-16T00:00:00Z");
    expect(forecast({ ...budget, period: "month", used: "15", resets_at: "2026-11-01T00:00:00Z" }, now)).toBeCloseTo(31);
    expect(forecast({ ...budget, period: "day", used: "1", resets_at: "2026-10-16T12:00:00Z" }, Date.parse("2026-10-16T00:00:00Z"))).toBeCloseTo(2);
    expect(forecast({ ...budget, period: "total", used: "1", resets_at: null }, now)).toBeUndefined();
    expect(forecast({ ...budget, period: "month", used: null, resets_at: "2026-11-01T00:00:00Z" }, now)).toBeUndefined();
  });
});
