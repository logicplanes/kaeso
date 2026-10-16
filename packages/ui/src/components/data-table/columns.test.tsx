import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
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
    key: "sk-kaeso-1a2b",
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
    key: "sk-kaeso-3c4d",
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
    for (const text of ["Checkout assistant", "sk-kaeso-1a2b", "12,400", "$12.50", "$0.00400", "1.84M", "1.5 s", "1 h ago", "Active", "Ada Okafor", "r2"]) {
      expect(table).toHaveTextContent(text);
    }
    expect(table.querySelectorAll("time").length).toBeGreaterThanOrEqual(4);
    expect(table).toHaveTextContent("1,840,000 tokens");
    expect(table).toHaveTextContent(/1 h ago 15 Oct 2026, \d\d:00:00/);
    expect(table.querySelectorAll("[role=cell] [tabindex]")).toHaveLength(0);
  });
});
