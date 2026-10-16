import { describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { List } from "./list";

const items = [
  { id: "prod", title: "production", children: [{ id: "prod-ro", title: "read-only" }] },
  { id: "stage", title: "staging" },
  { id: "dev", title: "development" },
];

describe("List keyboard", () => {
  it("moves with j and k, opens with Enter, and selects ranges with Shift", async () => {
    const onOpen = vi.fn();
    const onSelected = vi.fn();
    render(<List aria-label="Keys" items={items} selectable onOpen={onOpen} onSelectedChange={onSelected} />);
    const grid = screen.getByRole("grid");
    const rows = screen.getAllByRole("row");
    rows[0]!.focus();
    fireEvent.keyDown(rows[0]!, { key: "j" });
    fireEvent.keyDown(document.activeElement!, { key: "k" });
    fireEvent.keyDown(document.activeElement!, { key: "Enter" });
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: "prod" }));
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown", shiftKey: true });
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown", shiftKey: true });
    expect([...onSelected.mock.lastCall![0]]).toEqual(["prod", "prod-ro", "stage"]);
    fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    fireEvent.keyDown(document.activeElement!, { key: "a", metaKey: true });
    expect(onSelected.mock.lastCall![0].size).toBe(4);
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(onSelected.mock.lastCall![0].size).toBe(0);
    await userEvent.click(screen.getByText("development"));
    expect(onOpen).toHaveBeenLastCalledWith(expect.objectContaining({ id: "dev" }));
    expect(grid).toBeInTheDocument();
  });

  it("toggles a row while selecting, clears from the bar, and forwards refs", async () => {
    const ref = createRef<HTMLDivElement>();
    const fnRef = vi.fn();
    const { rerender } = render(<List ref={ref} aria-label="Keys" items={items} selectable />);
    expect(ref.current).not.toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Select production" }));
    expect(screen.getByRole("region", { name: "Selection" })).toHaveTextContent("1");
    await userEvent.click(screen.getByText("staging"));
    expect(screen.getByRole("region", { name: "Selection" })).toHaveTextContent("2");
    await userEvent.click(screen.getByRole("button", { name: /Clear/ }));
    expect(screen.queryByRole("region", { name: "Selection" })).toBeNull();
    rerender(<List ref={fnRef} aria-label="Keys" items={items} />);
    expect(fnRef).toHaveBeenCalled();
  });

  it("starts with collapsed groups expanded on request", async () => {
    render(
      <List
        aria-label="Keys"
        groups={[
          { id: "active", title: "Active", items: [items[1]!] },
          { id: "archived", title: "Archived", items: [items[2]!], defaultCollapsed: true },
        ]}
      />,
    );
    expect(screen.queryByText("development")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: /Archived/ }));
    expect(screen.getByText("development")).toBeInTheDocument();
  });
});
