import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DataTable } from "./data-table.js";
import { col } from "./columns.js";
import { InsetContent } from "../../utils/inset.js";

type Row = { id: string; key: string; at: string; cost: string };
const rows: Row[] = [
  { id: "a", key: "checkout-web", at: "2026-10-12T14:13:25.423Z", cost: "0.0320" },
  { id: "b", key: "ci-runner", at: "2026-10-12T13:01:00.000Z", cost: "0.0007" },
  { id: "c", key: "staging", at: "2026-10-11T09:40:00.000Z", cost: "1.2000" },
];
const columns = [
  col.date<Row>({ id: "at", header: "Time", size: 160 }, (r) => r.at),
  col.text<Row>({ id: "key", header: "Key", size: 160 }, (r) => r.key),
  col.custom<Row, string>(
    { id: "open", header: "Open", size: 120 },
    (r) => r.id,
    (v) => (
      <>
        <button type="button">Copy {v}</button>
        <button type="button">Share {v}</button>
      </>
    ),
  ),
  col.money<Row>({ id: "cost", header: "Cost", size: 120 }, (r) => r.cost, { decimals: 4 }),
];
const row = (name: string) => screen.getByText(name).closest<HTMLElement>("[role=row]")!;

describe("DataTable keyboard model", () => {
  it("is a grid with one tab stop: header buttons and row controls stay out of the tab order", async () => {
    const user = userEvent.setup();
    render(
      <>
        <DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable onRowClick={() => undefined} />
        <button type="button">After</button>
      </>,
    );
    const grid = screen.getByRole("table", { name: "Keys" });
    expect(grid).toHaveAttribute("aria-multiselectable", "true");
    expect(within(grid).getAllByRole("cell").length).toBeGreaterThan(0);
    await user.tab();
    expect(row("checkout-web")).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    for (const el of grid.querySelectorAll<HTMLElement>("button, [role=checkbox], [role=separator]")) expect(el).toHaveAttribute("tabindex", "-1");
  });

  it("names row checkboxes after the first text column, not the timestamp before it", () => {
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable />);
    expect(screen.getByRole("checkbox", { name: "Select ci-runner" })).toBeInTheDocument();
  });

  it("reaches the header from the first row, moves along it, resizes with Shift, and comes back down", async () => {
    const user = userEvent.setup();
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable onRowClick={() => undefined} />);
    row("checkout-web").focus();
    await user.keyboard("{ArrowUp}");
    const selectAll = screen.getByRole("checkbox", { name: "Select all rows" });
    expect(selectAll).toHaveFocus();
    expect(selectAll).toHaveAttribute("tabindex", "0");
    expect(row("checkout-web")).toHaveAttribute("tabindex", "-1");
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Time" })).toHaveFocus();
    await user.keyboard("{End}");
    expect(screen.getByRole("button", { name: "Cost" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Cost" })).toHaveFocus();
    await user.keyboard("{Home}");
    expect(selectAll).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(selectAll).toHaveFocus();
    await user.keyboard("{Shift>}{ArrowRight}{/Shift}");
    expect(selectAll).toHaveFocus();
    await user.keyboard("{ArrowRight}{Alt>}{ArrowRight}{/Alt}");
    expect(screen.getByRole("button", { name: "Time" })).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(row("checkout-web")).toHaveFocus();
  });

  it("selects ranges with Shift and the arrows, everything with mod A, and clears with Escape", async () => {
    const user = userEvent.setup();
    const onSelectionChange = vi.fn();
    render(
      <DataTable
        aria-label="Keys"
        data={rows}
        columns={columns}
        getRowId={(r) => r.id}
        selectable
        onSelectionChange={onSelectionChange}
        onRowClick={() => undefined}
      />,
    );
    row("checkout-web").focus();
    await user.keyboard("{Shift>}{ArrowDown}{ArrowDown}{/Shift}");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ a: true, b: true, c: true });
    expect(row("staging")).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(onSelectionChange).toHaveBeenLastCalledWith({});
    await user.keyboard("{Control>}a{/Control}");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ a: true, b: true, c: true });
    await user.keyboard("{Escape}x");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ c: true });
    await user.keyboard("{PageUp}{PageUp}");
    expect(row("checkout-web")).toHaveFocus();
    await user.keyboard("{Shift>}x{/Shift}");
    expect(onSelectionChange).toHaveBeenLastCalledWith({ c: true });
  });

  it("enters a row's controls with the right arrow and comes back with the left arrow or Escape", async () => {
    const user = userEvent.setup();
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} onRowClick={() => undefined} />);
    row("ci-runner").focus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Copy b" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Share b" })).toHaveFocus();
    await user.keyboard("{ArrowRight}{ArrowLeft}");
    expect(screen.getByRole("button", { name: "Copy b" })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(row("ci-runner")).toHaveFocus();
    await user.keyboard("{ArrowRight}{Escape}");
    expect(row("ci-runner")).toHaveFocus();
    await user.keyboard("{ArrowRight}{Control>}{ArrowLeft}{/Control}");
    expect(screen.getByRole("button", { name: "Copy b" })).toHaveFocus();
  });

  it("marks the open row as current, and stays a plain table when rows don't act", () => {
    const { rerender } = render(
      <DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} activeRowId="b" onRowClick={() => undefined} />,
    );
    expect(row("ci-runner")).toHaveAttribute("aria-current", "true");
    expect(row("ci-runner")).toHaveClass("bg-fill-2");
    rerender(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} />);
    expect(screen.getByRole("table", { name: "Keys" })).toBeInTheDocument();
    expect(row("ci-runner")).not.toHaveAttribute("tabindex");
  });

  it("hovers a step past the selected fill on the panel and in a card", () => {
    const { rerender } = render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable selection={{ b: true }} />);
    expect(row("ci-runner")).toHaveClass("bg-fill-2", "hover:bg-fill-3");
    expect(row("staging")).toHaveClass("hover:bg-fill-1");
    rerender(
      <InsetContent>
        <DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} selectable selection={{ b: true }} />
      </InsetContent>,
    );
    expect(row("ci-runner")).toHaveClass("bg-raised-pressed", "hover:raised-selected-hover");
    expect(row("staging")).toHaveClass("hover:bg-raised-hover");
  });

  it("changes a column's width from its header's context menu, without dragging", async () => {
    const user = userEvent.setup();
    render(<DataTable aria-label="Keys" data={rows} columns={columns} getRowId={(r) => r.id} />);
    const handle = () => screen.getAllByRole("separator", { name: "Resize column" })[1]!;
    expect(handle()).toHaveAttribute("aria-valuenow", "160");
    await user.pointer({ keys: "[MouseRight]", target: screen.getByRole("button", { name: "Key" }) });
    await user.click(await screen.findByRole("menuitem", { name: "Widen column" }));
    expect(handle()).toHaveAttribute("aria-valuenow", "192");
    await user.pointer({ keys: "[MouseRight]", target: screen.getByRole("button", { name: "Key" }) });
    await user.click(await screen.findByRole("menuitem", { name: "Narrow column" }));
    await user.pointer({ keys: "[MouseRight]", target: screen.getByRole("button", { name: "Key" }) });
    await user.click(await screen.findByRole("menuitem", { name: "Narrow column" }));
    expect(handle()).toHaveAttribute("aria-valuenow", "128");
    await user.pointer({ keys: "[MouseRight]", target: screen.getByRole("button", { name: "Key" }) });
    await user.click(await screen.findByRole("menuitem", { name: "Reset width" }));
    expect(handle()).toHaveAttribute("aria-valuenow", "160");
  });

  it("says how to recover when loading failed and there is no retry", () => {
    render(<DataTable aria-label="Keys" data={[]} columns={columns} error="503" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Reload the page to try again.");
  });
});
