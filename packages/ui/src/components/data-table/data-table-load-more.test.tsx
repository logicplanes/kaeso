import { afterEach, describe, expect, it, vi } from "vitest";
import { useEffect, useState } from "react";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DataTable } from "./data-table.js";
import { col } from "./columns.js";

type Row = { id: string; key: string };
const named = (from: number, count: number): Row[] => Array.from({ length: count }, (_, i) => ({ id: `r${from + i}`, key: `key-${from + i}` }));
const columns = [col.text<Row>({ id: "key", header: "Key", size: 160 }, (r) => r.key)];

// A paged source like an infinite query: Load more asks for the next page, which arrives when the test says so.
function Paged({ pages = 3, size = 3, clickable = false }: { pages?: number; size?: number; clickable?: boolean }) {
  const [rows, setRows] = useState(() => named(1, size));
  const [loading, setLoading] = useState(false);
  const page = rows.length / size;
  useEffect(() => {
    pager.arrive = () => {
      setRows((r) => [...r, ...named(r.length + 1, size)]);
      setLoading(false);
    };
    pager.start = () => setLoading(true);
  }, [size]);
  return (
    <>
      <DataTable
        aria-label="Keys"
        data={rows}
        columns={columns}
        getRowId={(r) => r.id}
        hasMore={page < pages}
        loadingMore={loading}
        onLoadMore={() => setLoading(true)}
        {...(clickable ? { onRowClick: () => {} } : {})}
      />
      <button>After the table</button>
    </>
  );
}
// The test's hands on the source: set once it renders.
const pager = { arrive: () => {}, start: () => {} };
const row = (key: string) => screen.getByText(key).closest<HTMLElement>("[role=row]")!;
const status = () => screen.getByRole("status");

afterEach(() => vi.restoreAllMocks());

describe("DataTable Load more", () => {
  it("moves focus to the first new row once the next page has loaded, and says how many rows came", async () => {
    const user = userEvent.setup();
    render(<Paged />);
    screen.getByRole("button", { name: "Load more" }).focus();
    await user.keyboard("{Enter}");
    expect(status()).toHaveTextContent("");
    act(() => pager.arrive());
    expect(row("key-4")).toHaveFocus();
    expect(status()).toHaveTextContent("3 more rows loaded");
    await user.tab();
    expect(screen.getByRole("button", { name: "Load more" })).toHaveFocus();
  });

  it("makes the first new row the table's stop when rows take focus", async () => {
    const user = userEvent.setup();
    render(<Paged clickable />);
    await user.click(screen.getByRole("button", { name: "Load more" }));
    act(() => pager.arrive());
    expect(row("key-4")).toHaveFocus();
    expect(row("key-4")).toHaveAttribute("tabindex", "0");
    expect(row("key-1")).toHaveAttribute("tabindex", "-1");
    await user.keyboard("{ArrowDown}");
    expect(row("key-5")).toHaveFocus();
  });

  it("moves to the first new row in the table's order when the rows are sorted, and counts only new rows", async () => {
    const user = userEvent.setup();
    const keys = (...k: string[]): Row[] => k.map((key) => ({ id: key, key }));
    const props = { "aria-label": "Keys", columns, getRowId: (r: Row) => r.id, sorting: [{ id: "key", desc: false }], hasMore: true };
    const { rerender } = render(<DataTable {...props} data={keys("key-b", "key-d", "key-f")} onLoadMore={() => {}} />);
    await user.click(screen.getByRole("button", { name: "Load more" }));
    rerender(<DataTable {...props} data={keys("key-b", "key-d", "key-f")} loadingMore onLoadMore={() => {}} />);
    // A live row at the top while the page loads is no part of the page.
    rerender(<DataTable {...props} data={keys("key-0", "key-b", "key-d", "key-f")} loadingMore onLoadMore={() => {}} />);
    rerender(<DataTable {...props} data={keys("key-0", "key-b", "key-d", "key-f", "key-c", "key-e", "key-g")} onLoadMore={() => {}} />);
    expect(row("key-c")).toHaveFocus();
    expect(status()).toHaveTextContent("3 more rows loaded");
  });

  it("makes a plain table's first new row focusable only until focus leaves it", async () => {
    const user = userEvent.setup();
    render(<Paged />);
    await user.click(screen.getByRole("button", { name: "Load more" }));
    act(() => pager.arrive());
    expect(row("key-4")).toHaveAttribute("tabindex", "-1");
    await user.tab();
    expect(row("key-4")).not.toHaveAttribute("tabindex");
  });

  it("forgets a page that failed, so rows that come later neither take focus nor are announced", async () => {
    const user = userEvent.setup();
    const props = { "aria-label": "Keys", columns, getRowId: (r: Row) => r.id, hasMore: true, onLoadMore: () => {} };
    const { rerender } = render(<DataTable {...props} data={named(1, 3)} />);
    const more = screen.getByRole("button", { name: "Load more" });
    await user.click(more);
    rerender(<DataTable {...props} data={named(1, 3)} loadingMore />);
    rerender(<DataTable {...props} data={named(1, 3)} />);
    rerender(<DataTable {...props} data={[...named(0, 1), ...named(1, 3)]} />);
    expect(more).toHaveFocus();
    expect(status()).toHaveTextContent("");
  });

  it("says when the last page has come, and still moves to its first row", async () => {
    const user = userEvent.setup();
    render(<Paged pages={2} />);
    await user.click(screen.getByRole("button", { name: "Load more" }));
    act(() => pager.arrive());
    expect(row("key-4")).toHaveFocus();
    expect(status()).toHaveTextContent("3 more rows loaded; all rows shown");
    expect(screen.getByText("All rows shown")).toBeInTheDocument();
  });

  it("keeps a focused Load more in view when rows arrive without it being pressed", async () => {
    const scrolled = vi.spyOn(HTMLElement.prototype, "scrollIntoView").mockImplementation(() => {});
    render(<Paged />);
    const more = screen.getByRole("button", { name: "Load more" });
    more.focus();
    act(() => pager.start());
    act(() => pager.arrive());
    expect(more).toHaveFocus();
    expect(scrolled).toHaveBeenCalledWith({ block: "nearest" });
    expect(status()).toHaveTextContent("3 more rows loaded");
  });

  it("leaves focus alone when it moved on before the rows arrived", async () => {
    const user = userEvent.setup();
    render(<Paged />);
    await user.click(screen.getByRole("button", { name: "Load more" }));
    await user.tab();
    expect(screen.getByRole("button", { name: "After the table" })).toHaveFocus();
    act(() => pager.arrive());
    expect(screen.getByRole("button", { name: "After the table" })).toHaveFocus();
    expect(status()).toHaveTextContent("3 more rows loaded");
  });

  it("says nothing when rows change without a page loading", () => {
    const { rerender } = render(<DataTable aria-label="Keys" data={named(1, 3)} columns={columns} getRowId={(r) => r.id} hasMore onLoadMore={() => {}} />);
    rerender(<DataTable aria-label="Keys" data={named(1, 5)} columns={columns} getRowId={(r) => r.id} hasMore onLoadMore={() => {}} />);
    expect(status()).toHaveTextContent("");
  });

  it("says nothing when the next page fails", async () => {
    const user = userEvent.setup();
    const { rerender } = render(
      <DataTable aria-label="Keys" data={named(1, 3)} columns={columns} getRowId={(r) => r.id} hasMore loadingMore onLoadMore={() => {}} />,
    );
    rerender(<DataTable aria-label="Keys" data={named(1, 3)} columns={columns} getRowId={(r) => r.id} hasMore onLoadMore={() => {}} />);
    expect(status()).toHaveTextContent("");
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(screen.getByRole("button", { name: "Load more" })).toHaveFocus();
  });
});
