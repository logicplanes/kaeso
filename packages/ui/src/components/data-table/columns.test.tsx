import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot } from "react-dom/client";
import { setFormatTimeZone } from "../../format/index.js";
import { DataTable } from "./data-table.js";
import { col } from "./columns.js";

interface Row {
  id: string;
  name: string;
  key: string;
  requests: number;
  cost: string;
  tokens: number;
  latency: number;
  at: string;
  status: { tone: "success" | "danger"; label: string };
  owner: { name: string; src?: string };
}

const rows: Row[] = [
  {
    id: "r1",
    name: "Checkout assistant",
    key: "sk-kso-1a2b",
    requests: 12400,
    cost: "12.5",
    tokens: 1_840_000,
    latency: 1500,
    at: "2026-10-15T08:00:00Z",
    status: { tone: "success", label: "Active" },
    owner: { name: "Ada Okafor", src: "/ada.png" },
  },
  {
    id: "r2",
    name: "Docs search",
    key: "sk-kso-3c4d",
    requests: 80,
    cost: "0.004",
    tokens: 900,
    latency: 240,
    at: "2026-10-14T08:00:00Z",
    status: { tone: "danger", label: "Disabled" },
    owner: { name: "Lin Park" },
  },
];

describe("column helpers", () => {
  it("formats every kind of cell", () => {
    render(
      <DataTable<Row>
        aria-label="Projects"
        data={rows}
        getRowId={(r) => r.id}
        columns={[
          col.text(
            { id: "name", header: "Name", grow: true, priority: 0, minWidth: 160, tone: (r) => (r.status.tone === "danger" ? "danger" : undefined) },
            (r) => r.name,
          ),
          col.mono<Row>({ id: "key", header: "Key" }, (r) => r.key),
          col.number<Row>({ id: "requests", header: "Requests", size: 120 }, (r) => r.requests),
          col.money<Row>({ id: "cost", header: "Cost" }, (r) => r.cost, { decimals: 2 }),
          col.money<Row>({ id: "cost2", header: "Exact" }, (r) => r.cost),
          col.tokens<Row>({ id: "tokens", header: "Tokens" }, (r) => r.tokens),
          col.duration<Row>({ id: "latency", header: "Latency" }, (r) => r.latency),
          col.date<Row>({ id: "at", header: "At" }, (r) => r.at),
          col.relative<Row>({ id: "seen", header: "Seen" }, (r) => r.at, { now: Date.parse("2026-10-15T09:00:00Z") }),
          col.relative<Row>({ id: "seen2", header: "Seen now" }, (r) => r.at),
          col.status<Row>({ id: "status", header: "Status" }, (r) => r.status),
          col.avatar<Row>({ id: "owner", header: "Owner" }, (r) => r.owner),
          col.custom(
            { id: "custom", header: "Custom" },
            (r) => r.id,
            (v) => <b>{v}</b>,
            { align: "end" },
          ),
        ]}
      />,
    );
    const table = screen.getByRole("table", { name: "Projects" });
    for (const text of ["Checkout assistant", "sk-kso-1a2b", "12,400", "$12.50", "$0.00400", "1.84M", "1.5 s", "1 h ago", "Active", "Ada Okafor", "r2"]) {
      expect(table).toHaveTextContent(text);
    }
    expect(table.querySelectorAll("time").length).toBeGreaterThanOrEqual(4);
    expect(table).toHaveTextContent("1,840,000 tokens");
    expect(table).toHaveTextContent(/1 h ago 15 Oct 2026, \d\d:00:00/);
    expect(table.querySelectorAll("[role=cell] [tabindex]")).toHaveLength(0);
  });
});

describe("time columns on a server-rendered page", () => {
  const zoneBefore = process.env.TZ;
  afterEach(() => {
    process.env.TZ = zoneBefore;
    setFormatTimeZone(undefined);
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  interface Seen {
    id: string;
    at: string;
  }
  const seen: Seen[] = [
    { id: "late", at: "2026-10-14T23:30:00Z" },
    { id: "old", at: "2026-10-01T23:30:00Z" },
  ];
  const table = (
    <DataTable<Seen>
      aria-label="Keys"
      data={seen}
      getRowId={(r) => r.id}
      columns={[
        col.text<Seen>({ id: "name", header: "Name" }, (r) => r.id),
        col.date<Seen>({ id: "at", header: "At" }, (r) => r.at),
        col.relative<Seen>({ id: "fixed", header: "Seen" }, (r) => r.at, { now: Date.parse("2026-10-15T09:00:00Z") }),
        col.relative<Seen>({ id: "live", header: "Seen now" }, (r) => r.at),
      ]}
    />
  );
  const cells = (container: HTMLElement, row: string) =>
    [...container.querySelectorAll(`[role=row]`)].find((r) => r.textContent?.startsWith(row))!.querySelectorAll("[role=cell] time");

  it("hydrates a page built in another time zone, minutes earlier, without a mismatch, then shows the viewer's zone and clock", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-10-15T09:00:00Z"));
    process.env.TZ = "Asia/Tokyo";
    const html = renderToString(table);
    const container = document.createElement("div");
    container.innerHTML = html;
    document.body.append(container);

    vi.setSystemTime(new Date("2026-10-15T09:20:00Z"));
    process.env.TZ = "America/Los_Angeles";
    const onRecoverableError = vi.fn();
    const root = await act(async () => hydrateRoot(container, table, { onRecoverableError }));
    expect(onRecoverableError).not.toHaveBeenCalled();
    expect(html).toContain("14 Oct 2026, 23:30 UTC");
    expect(html).toContain("1 Oct 2026, 23:30 UTC");
    expect([...cells(container, "late")].map((t) => t.textContent)).toEqual(["14 Oct 2026, 16:30", "10 h ago", "10 h ago"]);
    expect([...cells(container, "old")].map((t) => t.textContent)).toEqual(["1 Oct 2026, 16:30", "1 Oct", "1 Oct"]);
    expect(container).toHaveTextContent("14 Oct 2026, 16:30:00");
    expect(container).not.toHaveTextContent("UTC");
    act(() => root.unmount());
  });

  it("renders in the app's format zone from the first paint when one is set", async () => {
    setFormatTimeZone("Europe/Berlin");
    process.env.TZ = "Asia/Tokyo";
    const container = document.createElement("div");
    container.innerHTML = renderToString(table);
    document.body.append(container);
    expect(container).toHaveTextContent("15 Oct 2026, 01:30");
    expect(container).not.toHaveTextContent("UTC");
    process.env.TZ = "America/Los_Angeles";
    const onRecoverableError = vi.fn();
    const root = await act(async () => hydrateRoot(container, table, { onRecoverableError }));
    expect(onRecoverableError).not.toHaveBeenCalled();
    expect([...cells(container, "late")].map((t) => t.textContent)).toEqual(["15 Oct 2026, 01:30", "10 h ago", expect.any(String)]);
    act(() => root.unmount());
  });
});
