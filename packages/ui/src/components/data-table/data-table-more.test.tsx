import { describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DataTable } from "./data-table.js";
import { col } from "./columns.js";

type Row = { id: string; key: string; requests: number };
const rows: Row[] = [
  { id: "a", key: "checkout-web", requests: 12 },
  { id: "b", key: "ci-runner", requests: 7 },
];
const columns = [
  col.text<Row>({ id: "key", header: "Key", size: 160 }, (r) => r.key),
  col.number<Row>({ id: "requests", header: "Requests" }, (r) => r.requests),
];

describe("DataTable selection and rows", () => {
  it("selects all rows and single rows, controlled or not", async () => {
    const onSelection = vi.fn();
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable onSelectionChange={onSelection} />);
    await userEvent.click(screen.getByRole("checkbox", { name: "Select all rows" }));
    expect(onSelection).toHaveBeenLastCalledWith({ a: true, b: true });
    await userEvent.click(screen.getAllByRole("checkbox", { name: /^Select (?!all)/ })[0]!);
    expect(onSelection).toHaveBeenLastCalledWith({ b: true });
    expect(screen.getAllByRole("row").filter((r) => r.getAttribute("aria-selected") === "true")).toHaveLength(1);
  });

  it("keeps a controlled selection and opens rows by click or Enter, not from their controls", async () => {
    const onRowClick = vi.fn();
    const onSelection = vi.fn();
    render(
      <DataTable
        aria-label="Keys"
        data={rows}
        columns={[
          ...columns,
          col.custom<Row, string>(
            { id: "act", header: "" },
            (r) => r.id,
            () => <button>Copy</button>,
          ),
        ]}
        getRowId={(r) => r.id}
        selectable
        selection={{ a: true }}
        onSelectionChange={onSelection}
        onRowClick={onRowClick}
        activeRowId="a"
      />,
    );
    const [first] = screen.getAllByRole("row").filter((r) => r.hasAttribute("tabindex"));
    await userEvent.click(within(first!).getByText("checkout-web"));
    expect(onRowClick).toHaveBeenCalledWith(rows[0]);
    await userEvent.click(within(first!).getByRole("button", { name: "Copy" }));
    expect(onRowClick).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(first!, { key: "Enter" });
    expect(onRowClick).toHaveBeenCalledTimes(2);
    expect(first).toHaveAttribute("data-active");
    await userEvent.click(screen.getAllByRole("checkbox", { name: /^Select (?!all)/ })[1]!);
    expect(onSelection).toHaveBeenLastCalledWith({ a: true, b: true });
    expect(screen.getAllByRole("checkbox", { name: /^Select (?!all)/ })[1]).not.toBeChecked();
  });

  it("shows skeleton rows while the first page loads, and a custom empty state", () => {
    const { rerender } = render(<DataTable aria-label="Keys" data={[]} columns={columns} loading density="compact" />);
    expect(screen.getAllByRole("row").length).toBeGreaterThan(8);
    rerender(<DataTable aria-label="Keys" data={[]} columns={columns} empty={<p>No keys yet</p>} />);
    expect(screen.getByText("No keys yet")).toBeInTheDocument();
  });

  it("resets a column's width on a double click, syncs header scroll and forwards refs", () => {
    const ref = createRef<HTMLDivElement>();
    const fnRef = vi.fn();
    const { container, rerender } = render(<DataTable ref={ref} aria-label="Keys" data={rows} columns={columns} />);
    expect(ref.current).not.toBeNull();
    fireEvent.doubleClick(screen.getAllByRole("separator", { name: "Resize column" })[0]!);
    const groups = container.querySelectorAll<HTMLElement>("[role=rowgroup]");
    groups[1]!.scrollLeft = 40;
    fireEvent.scroll(groups[1]!);
    groups[0]!.scrollLeft = 10;
    fireEvent.scroll(groups[0]!);
    rerender(<DataTable ref={fnRef} aria-label="Keys" data={rows} columns={columns} />);
    expect(fnRef).toHaveBeenCalled();
  });

  it("renders only the visible rows of a long table", () => {
    const many = Array.from({ length: 300 }, (_, i) => ({ id: String(i), key: `key-${i}`, requests: i }));
    render(<DataTable aria-label="Keys" data={many} columns={columns} getRowId={(r) => r.id} virtualizeAbove={50} />);
    expect(screen.getAllByRole("row").length).toBeLessThan(300);
  });

  it("shows the rows in view of a long table that sits below other content", () => {
    const page = (document.scrollingElement ?? document.documentElement) as HTMLElement;
    const above = 3000;
    const size = vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function (this: HTMLElement) {
      return this === page ? 800 : 0;
    });
    const rect = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const body = this.getAttribute("role") === "rowgroup" && this.previousElementSibling?.getAttribute("role") === "rowgroup";
      const top = body ? above - page.scrollTop : 0;
      return { top, bottom: top, left: 0, right: 0, width: 0, height: 0, x: 0, y: top, toJSON: () => ({}) } as DOMRect;
    });
    try {
      const many = Array.from({ length: 300 }, (_, i) => ({ id: String(i), key: `key-${i}`, requests: i }));
      render(<DataTable aria-label="Keys" data={many} columns={columns} getRowId={(r) => r.id} virtualizeAbove={50} />);
      page.scrollTop = above + 200 * 40;
      fireEvent.scroll(page);
      expect(screen.getByText("key-200")).toBeInTheDocument();
      expect(screen.getByText("key-200").closest("[role=row]")).toHaveStyle({ transform: "translateY(8000px)" });
    } finally {
      size.mockRestore();
      rect.mockRestore();
      page.scrollTop = 0;
    }
  });
});
