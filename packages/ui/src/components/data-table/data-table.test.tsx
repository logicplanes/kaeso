import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DATA_TABLE_GROW_MIN, DataTable, fitColumns } from "./data-table.js";
import { col } from "./columns.js";
import { GutterContent } from "../../utils/inset.js";

type Row = { id: string; key: string; cost: string };
const rows: Row[] = [
  { id: "a", key: "checkout-web", cost: "0.0320" },
  { id: "b", key: "ci-runner", cost: "0.0007" },
  { id: "c", key: "staging", cost: "1.2000" },
];
const columns = [
  col.text<Row>({ id: "key", header: "Key", size: 160 }, (r) => r.key),
  col.money<Row>({ id: "cost", header: "Cost", size: 120 }, (r) => r.cost, { decimals: 4 }),
];

describe("DataTable", () => {
  it("renders a table with headers and aligned money", () => {
    render(<DataTable aria-label="Costs" data={rows} columns={columns} getRowId={(r) => r.id} />);
    const table = screen.getByRole("table", { name: "Costs" });
    expect(within(table).getByRole("columnheader", { name: /Key/ })).toBeInTheDocument();
    expect(within(table).getByText("$0.0007")).toBeInTheDocument();
    expect(within(table).getByText("$1.2000")).toBeInTheDocument();
  });
  it("sorts money numerically from its header", async () => {
    render(<DataTable aria-label="Costs" data={rows} columns={columns} getRowId={(r) => r.id} />);
    await userEvent.click(screen.getByRole("button", { name: /Cost/ }));
    const header = screen.getByRole("columnheader", { name: /Cost/ });
    expect(["ascending", "descending"]).toContain(header.getAttribute("aria-sort"));
    const cells = screen
      .getAllByRole("row")
      .slice(1)
      .map((r) => within(r).getAllByRole("cell").at(-1)!.textContent);
    const values = cells.map((c) => Number(c!.replace(/[$,]/g, "")));
    const sorted = [...values].sort((x, y) => (header.getAttribute("aria-sort") === "ascending" ? x - y : y - x));
    expect(values).toEqual(sorted);
  });
  it("says so when there is nothing to show", () => {
    render(<DataTable aria-label="Costs" data={[]} columns={columns} />);
    expect(screen.getByRole("table", { name: "Costs" }).parentElement?.textContent ?? document.body.textContent).toMatch(/No|nothing/i);
  });
  it("says it couldn't load the rows and offers Try again, without drawing the error", async () => {
    let retried = 0;
    render(<DataTable aria-label="Costs" data={[]} columns={columns} error="The gateway answered 503." onRetry={() => retried++} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load these rows");
    expect(screen.queryByText("The gateway answered 503.")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(retried).toBe(1);
  });
  it("colors a cell's text with the column's tone for that row", () => {
    const toned = [col.text<Row>({ id: "key", header: "Key", size: 160, tone: (r) => (r.id === "c" ? "danger" : undefined) }, (r) => r.key)];
    render(<DataTable aria-label="Keys" data={rows} columns={toned} getRowId={(r) => r.id} />);
    expect(screen.getByText("staging").closest("[role=cell]")).toHaveClass("text-danger-text");
    expect(screen.getByText("ci-runner").closest("[role=cell]")).not.toHaveClass("text-danger-text");
  });
  it("marks rows that just arrived so they fade in", () => {
    render(<DataTable aria-label="Costs" data={rows} columns={columns} getRowId={(r) => r.id} freshRowIds={new Set(["b"])} />);
    const fresh = screen.getByText("ci-runner").closest("[role=row]");
    expect(fresh).toHaveAttribute("data-fresh", "true");
    expect(screen.getByText("staging").closest("[role=row]")).not.toHaveAttribute("data-fresh");
  });
  describe("on a narrow screen", () => {
    afterEach(() => vi.restoreAllMocks());
    const narrow = (width: number) =>
      vi
        .spyOn(HTMLElement.prototype, "getBoundingClientRect")
        .mockReturnValue({ width, height: 0, top: 0, left: 0, right: width, bottom: 0, x: 0, y: 0, toJSON: () => ({}) });
    const growing = [
      col.text<Row>({ id: "key", header: "Key", size: 240, priority: 1, grow: true }, (r) => r.key),
      col.money<Row>({ id: "cost", header: "Cost", size: 120, priority: 1 }, (r) => r.cost, { decimals: 4 }),
    ];
    it("keeps the growing column's width when the columns fit", () => {
      narrow(800);
      render(<DataTable aria-label="Costs" data={rows} columns={growing} getRowId={(r) => r.id} />);
      expect(screen.getByText("staging").closest<HTMLElement>("[role=row]")?.style.gridTemplateColumns).toBe("minmax(240px, 1fr) 120px");
    });
    it("lets the growing column shrink to its readable minimum, not to its icon, when the columns that always stay overflow", () => {
      narrow(320);
      render(<DataTable aria-label="Costs" data={rows} columns={growing} getRowId={(r) => r.id} />);
      expect(screen.getByText("staging").closest<HTMLElement>("[role=row]")?.style.gridTemplateColumns).toBe(`minmax(${DATA_TABLE_GROW_MIN}px, 1fr) 120px`);
    });
    it("takes a column's own readable minimum", () => {
      narrow(320);
      const own = [col.text<Row>({ id: "key", header: "Key", size: 240, priority: 1, grow: true, minWidth: 160 }, (r) => r.key), growing[1]!];
      render(<DataTable aria-label="Costs" data={rows} columns={own} getRowId={(r) => r.id} />);
      expect(screen.getByText("staging").closest<HTMLElement>("[role=row]")?.style.gridTemplateColumns).toBe("minmax(160px, 1fr) 120px");
    });

    type Log = { id: string; time: string; status: number; model: string; key: string; cost: string };
    const logs: Log[] = [{ id: "r1", time: "2 min ago", status: 200, model: "claude-sonnet-4-5", key: "checkout-web", cost: "0.0108" }];
    const logColumns = [
      col.text<Log>({ id: "time", header: "Time", size: 104, priority: 1 }, (r) => r.time),
      col.custom<Log, number>(
        { id: "status", header: "Status", size: 80, priority: 1 },
        (r) => r.status,
        (v) => String(v),
      ),
      col.custom<Log, string>(
        { id: "model", header: "Model", size: 240, priority: 1, grow: true },
        (r) => r.model,
        (v) => (
          <span className="inline-flex min-w-0 items-center gap-8">
            <span aria-hidden>A</span>
            <span className="min-w-0 truncate">{v}</span>
          </span>
        ),
      ),
      col.text<Log>({ id: "key", header: "Key", size: 140, priority: 3 }, (r) => r.key),
      col.money<Log>({ id: "cost", header: "Cost", size: 112, priority: 2 }, (r) => r.cost, { decimals: 4 }),
      col.custom<Log, Log>(
        { id: "actions", header: "Actions", size: 64, priority: 1 },
        (r) => r,
        (r) => <button type="button">{`Actions for ${r.id}`}</button>,
      ),
    ];
    const headers = () => screen.getAllByRole("columnheader").map((h) => h.textContent);
    it("drops the lower-priority columns first and keeps the row's actions at a phone's width", () => {
      narrow(390);
      render(<DataTable aria-label="Request logs" data={logs} columns={logColumns} getRowId={(r) => r.id} />);
      expect(headers()).toEqual(["Time", "Status", "Model", "Actions"]);
      const row = screen.getByText("claude-sonnet-4-5").closest<HTMLElement>("[role=row]")!;
      expect(row.style.gridTemplateColumns).toBe(`104px 80px minmax(${DATA_TABLE_GROW_MIN}px, 1fr) 64px`);
      expect(screen.getByRole("button", { name: "Actions for r1" })).toBeInTheDocument();
    });
    it("lets the other priority 1 columns give way from the row's end, never the model or the actions, on a narrower screen", () => {
      narrow(320);
      render(<DataTable aria-label="Request logs" data={logs} columns={logColumns} getRowId={(r) => r.id} />);
      expect(headers()).toEqual(["Time", "Model", "Actions"]);
      expect(screen.getByText("claude-sonnet-4-5").closest<HTMLElement>("[role=row]")!.style.gridTemplateColumns).toBe(
        `104px minmax(${DATA_TABLE_GROW_MIN}px, 1fr) 64px`,
      );
    });
    it("tells the page which columns it left out, so a kept column can show what they held", () => {
      narrow(390);
      const onHidden = vi.fn();
      render(<DataTable aria-label="Request logs" data={logs} columns={logColumns} getRowId={(r) => r.id} onHiddenColumnsChange={onHidden} />);
      expect(onHidden).toHaveBeenLastCalledWith(expect.arrayContaining(["key", "cost"]));
      expect(onHidden.mock.lastCall?.[0]).not.toContain("model");
    });
    it("lets the growing column's cell fill its track, for content that adapts to its width", () => {
      narrow(800);
      render(<DataTable aria-label="Costs" data={rows} columns={growing} getRowId={(r) => r.id} />);
      expect(screen.getByText("staging").closest("[role=cell] > span")).toHaveClass("flex-1");
    });
    it("keeps the selection boxes when columns give way", () => {
      narrow(300);
      render(<DataTable aria-label="Request logs" data={logs} columns={logColumns} getRowId={(r) => r.id} selectable />);
      expect(screen.getByRole("checkbox", { name: "Select all rows" })).toBeInTheDocument();
      expect(headers().slice(1)).toEqual(["Model", "Actions"]);
    });
    it("pins the last column when a row without a growing column has to scroll", () => {
      narrow(200);
      render(<DataTable aria-label="Costs" data={rows} columns={columns.map((c) => ({ ...c, meta: { ...c.meta, priority: 1 } }))} getRowId={(r) => r.id} />);
      const cells = within(screen.getByText("staging").closest<HTMLElement>("[role=row]")!).getAllByRole("cell");
      expect(cells.at(-1)).toHaveClass("sticky", "right-0");
      expect(cells[0]).not.toHaveClass("sticky");
      expect(screen.getByText("staging").closest<HTMLElement>("[role=row]")!.style.minWidth).toBe("280px");
    });
    it("rounds a pinned column with its row's hover fill, but keeps the header's line straight to the edge", () => {
      narrow(200);
      render(<DataTable aria-label="Costs" data={rows} columns={columns.map((c) => ({ ...c, meta: { ...c.meta, priority: 1 } }))} getRowId={(r) => r.id} />);
      const cell = within(screen.getByText("staging").closest<HTMLElement>("[role=row]")!).getAllByRole("cell").at(-1)!;
      const header = screen.getAllByRole("columnheader").at(-1)!;
      expect(cell).toHaveClass("sticky", "rounded-r-item");
      expect(header).toHaveClass("sticky", "right-0", "bg-panel", "hairline-b-strong");
      expect(header.className).not.toMatch(/rounded/);
    });
  });
});

describe("fitColumns", () => {
  const c = (id: string, size: number, priority: number, grow = false) => ({ id, size, priority, grow, minWidth: undefined });
  const logs = [
    c("time", 104, 1),
    c("status", 80, 1),
    c("model", 240, 1, true),
    c("key", 140, 3),
    c("latency", 104, 2),
    c("cost", 112, 2),
    c("actions", 64, 1),
  ];
  it("keeps everything with room to spare", () => {
    expect(fitColumns(logs, 1200)).toEqual({ hidden: [], tight: false, overflow: false });
  });
  it("drops the highest priority numbers first and keeps the growing column at its size while it can", () => {
    expect(fitColumns(logs, 700)).toEqual({ hidden: ["key", "cost"], tight: false, overflow: false });
  });
  it("shrinks the growing column to its readable minimum before any priority 1 column gives way", () => {
    expect(fitColumns(logs, 390)).toEqual({ hidden: ["key", "latency", "cost"], tight: true, overflow: false });
  });
  it("gives way from the row's end backwards, keeping the last column", () => {
    expect(fitColumns(logs, 320).hidden).toEqual(["status", "key", "latency", "cost"]);
    expect(fitColumns(logs, 240)).toEqual({ hidden: ["time", "status", "key", "latency", "cost"], tight: true, overflow: false });
  });
  it("scrolls only when nothing is left to give way", () => {
    expect(fitColumns(logs, 160)).toEqual({ hidden: ["time", "status", "key", "latency", "cost"], tight: true, overflow: true });
  });
});

describe("DataTable on the gutter", () => {
  it("insets the table to the 24px gutter and pads every cell 12px, so row fills never touch the panel's edge", () => {
    const { container } = render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} />);
    expect(container.firstElementChild).toHaveClass("px-24");
    const cells = within(screen.getByText("checkout-web").closest<HTMLElement>("[role=row]")!).getAllByRole("cell");
    expect(cells[0]).toHaveClass("px-12");
    expect(cells[0]).not.toHaveClass("pl-24");
    expect(cells.at(-1)).not.toHaveClass("pr-24");
    expect(screen.getByText("checkout-web").closest("[role=row]")).toHaveClass("rounded-item");
  });
  it("takes no gutter of its own inside a padded page body", () => {
    const { container } = render(
      <GutterContent>
        <DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} />
      </GutterContent>,
    );
    expect(container.firstElementChild).not.toHaveClass("px-24");
  });
  it("fits columns to the width inside the gutter without reserving it twice", () => {
    const c = (id: string, size: number, priority: number) => ({ id, size, priority, grow: false, minWidth: undefined });
    const cols = [c("time", 160, 1), c("status", 80, 1), c("provider", 144, 3)];
    expect(fitColumns(cols, 384, 0).hidden).toEqual([]);
    expect(fitColumns(cols, 384).hidden).toEqual(["provider"]);
  });
});

describe("DataTable from the keyboard", () => {
  it("is one tab stop that the arrows, Home and End move between rows, Enter opens and Space selects", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} onRowClick={open} selectable />);
    const row = (name: string) => screen.getByText(name).closest<HTMLElement>("[role=row]")!;
    expect(row("checkout-web")).toHaveAttribute("tabindex", "0");
    expect(row("ci-runner")).toHaveAttribute("tabindex", "-1");
    row("checkout-web").focus();
    await user.keyboard("{ArrowDown}");
    expect(row("ci-runner")).toHaveFocus();
    expect(row("ci-runner")).toHaveAttribute("tabindex", "0");
    expect(row("checkout-web")).toHaveAttribute("tabindex", "-1");
    await user.keyboard("{End}");
    expect(row("staging")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(row("checkout-web")).toHaveFocus();
    await user.keyboard(" ");
    expect(row("checkout-web")).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Enter}");
    expect(open).toHaveBeenCalledWith(rows[0]);
  });
  it("numbers its rows for assistive technology, the header first", () => {
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} />);
    const table = screen.getByRole("table", { name: "Keys" });
    expect(table).toHaveAttribute("aria-rowcount", "4");
    expect(screen.getAllByRole("columnheader")[0]!.closest("[role=row]")).toHaveAttribute("aria-rowindex", "1");
    expect(screen.getByText("staging").closest("[role=row]")).toHaveAttribute("aria-rowindex", "4");
  });
  it("names each row's checkbox after the row", () => {
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable />);
    expect(screen.getByRole("checkbox", { name: "Select ci-runner" })).toBeInTheDocument();
  });
  it("resizes a column with the arrow keys on its handle", async () => {
    const user = userEvent.setup();
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} />);
    const handle = screen.getAllByRole("separator", { name: "Resize column" })[0]!;
    expect(handle).toHaveAttribute("aria-valuenow", "160");
    handle.focus();
    await user.keyboard("{ArrowRight}");
    expect(handle).toHaveAttribute("aria-valuenow", "176");
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(handle).toHaveAttribute("aria-valuenow", "144");
  });

  it("keeps rows that can't be selected out of the selection, select all included", async () => {
    const user = userEvent.setup();
    const onSelectionChange = vi.fn();
    const rows = [
      { id: "a", name: "Alpha", done: false },
      { id: "b", name: "Beta", done: true },
      { id: "c", name: "Gamma", done: false },
    ];
    render(
      <DataTable
        aria-label="Models"
        data={rows}
        getRowId={(r) => r.id}
        columns={[col.text({ id: "name", header: "Name" }, (r: (typeof rows)[number]) => r.name)]}
        selectable
        canSelectRow={(r) => !r.done}
        onSelectionChange={onSelectionChange}
      />,
    );
    expect(screen.getByRole("checkbox", { name: "Select Beta" })).toHaveAttribute("aria-disabled", "true");
    await user.click(screen.getByRole("checkbox", { name: "Select all rows" }));
    expect(onSelectionChange).toHaveBeenLastCalledWith({ a: true, c: true });
    expect(screen.getByRole("checkbox", { name: "Select all rows" })).toHaveAttribute("aria-checked", "true");
  });
});
