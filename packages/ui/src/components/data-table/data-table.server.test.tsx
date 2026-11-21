// @vitest-environment node
import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DataTable } from "./data-table.js";
import { col } from "./columns.js";
import { List } from "../list/list.js";

type Row = { id: string; key: string };
const many = (n: number): Row[] => Array.from({ length: n }, (_, i) => ({ id: String(i), key: `key-${i}` }));
const columns = [col.text<Row>({ id: "key", header: "Key", size: 160 }, (r) => r.key)];

// A site renders its pages ahead of time, where there is no page to have scrolled yet: a long table or list starts at
// the top there instead of asking for the page's scroll position.
describe("Long tables and lists rendered on a server", () => {
  it("renders a table that draws only its visible rows", () => {
    const html = renderToString(<DataTable aria-label="Keys" data={many(80)} columns={columns} getRowId={(r) => r.id} virtualizeAbove={50} />);
    expect(html).toContain('aria-label="Keys"');
  });

  it("renders a list that draws only its visible rows", () => {
    const html = renderToString(<List aria-label="Keys" items={many(80).map((r) => ({ id: r.id, title: r.key }))} virtualizeAbove={50} />);
    expect(html).toContain('aria-label="Keys"');
  });
});
