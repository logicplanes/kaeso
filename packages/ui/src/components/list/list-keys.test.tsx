import { describe, expect, it, vi } from "vitest";
import { createRef, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { List, type ListGroup, type ListItemData } from "./list.js";
import { Dialog } from "../dialog/dialog.js";
import { IconButton } from "../icon-button/icon-button.js";
import { icons } from "../../icons/vocabulary.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

await import("../checkbox/checkbox.js");

const items: ListItemData[] = [
  { id: "prod", title: "production", children: [{ id: "prod-ro", title: "read-only" }] },
  { id: "stage", title: "staging" },
  { id: "dev", title: "development" },
];
const withActions = (list: ListItemData[]) =>
  list.map((i) => ({
    ...i,
    actions: (
      <>
        <IconButton icon={icons.copy} aria-label={`Copy ${i.id}`} size="xs" tooltip={false} />
        <IconButton icon={icons.more} aria-label={`More for ${i.id}`} size="xs" tooltip={false} />
      </>
    ),
  }));
const cell = (name: string) => screen.getByText(name).closest<HTMLElement>("[role=gridcell]")!;

describe("List keyboard", () => {
  it("moves focus to the line that takes the place of one an action removed", async () => {
    const user = userEvent.setup();
    const flatItems = items.map(({ id, title }) => ({ id, title }));
    const { rerender } = render(<List aria-label="Keys" items={withActions(flatItems)} />);
    await act(() => lazyModulesSettled());
    await user.click(screen.getByRole("button", { name: "More for stage" }));
    expect(screen.getByRole("button", { name: "More for stage" })).toHaveFocus();
    rerender(<List aria-label="Keys" items={withActions(flatItems.filter((i) => i.id !== "stage"))} />);
    expect(cell("development")).toHaveFocus();
    rerender(<List aria-label="Keys" items={withActions(flatItems.filter((i) => i.id === "prod"))} />);
    expect(cell("production")).toHaveFocus();
  });

  it("also when the line goes while focus is in the row's menu, a portal outside the list", async () => {
    const user = userEvent.setup();
    const flatItems = withActions(items.map(({ id, title }) => ({ id, title })));
    const withMenu = flatItems.map((i) =>
      i.id === "stage"
        ? {
            ...i,
            actions: (
              <>
                {i.actions}
                {createPortal(<button type="button">Disable</button>, document.body)}
              </>
            ),
          }
        : i,
    );
    const { rerender } = render(<List aria-label="Keys" items={withMenu} />);
    await act(() => lazyModulesSettled());
    await user.click(screen.getByRole("button", { name: "More for stage" }));
    await user.click(screen.getByRole("button", { name: "Disable" }));
    act(() => screen.getByRole("button", { name: "Disable" }).blur());
    rerender(<List aria-label="Keys" items={flatItems.filter((i) => i.id !== "stage")} />);
    expect(cell("development")).toHaveFocus();
  });

  it("also when the line goes while its confirmation is still closing", async () => {
    const user = userEvent.setup();
    const flatItems = withActions(items.map(({ id, title }) => ({ id, title })));
    const view = (list: ListItemData[], confirming: boolean) => (
      <>
        <List aria-label="Keys" items={list} />
        {confirming && (
          <div role="alertdialog" aria-label="Archive stage?">
            <button type="button">Archive</button>
          </div>
        )}
      </>
    );
    const { rerender } = render(view(flatItems, true));
    await act(() => lazyModulesSettled());
    await user.click(screen.getByRole("button", { name: "More for stage" }));
    await user.click(screen.getByRole("button", { name: "Archive" }));
    rerender(
      view(
        flatItems.filter((i) => i.id !== "stage"),
        true,
      ),
    );
    expect(screen.getByRole("button", { name: "Archive" })).toHaveFocus();
    rerender(
      view(
        flatItems.filter((i) => i.id !== "stage"),
        false,
      ),
    );
    await waitFor(() => expect(cell("development")).toHaveFocus());
  });

  it("leaves focus alone when it had already left the list before the line went", async () => {
    const user = userEvent.setup();
    const flatItems = items.map(({ id, title }) => ({ id, title }));
    const { rerender } = render(
      <>
        <List aria-label="Keys" items={withActions(flatItems)} />
        <button type="button">Elsewhere</button>
      </>,
    );
    await act(() => lazyModulesSettled());
    await user.click(screen.getByRole("button", { name: "More for stage" }));
    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    rerender(
      <>
        <List aria-label="Keys" items={withActions(flatItems.filter((i) => i.id !== "stage"))} />
        <button type="button">Elsewhere</button>
      </>,
    );
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
    (document.activeElement as HTMLElement).blur();
    rerender(
      <>
        <List aria-label="Keys" items={withActions(flatItems.filter((i) => i.id === "prod"))} />
        <button type="button">Elsewhere</button>
      </>,
    );
    expect(document.body).toHaveFocus();
  });

  it("is one tab stop: Tab enters on a row and the next Tab leaves, past every checkbox and action", async () => {
    const user = userEvent.setup();
    render(
      <>
        <button type="button">Before</button>
        <List aria-label="Keys" items={withActions(items)} selectable />
        <button type="button">After</button>
      </>,
    );
    await act(() => lazyModulesSettled());
    screen.getByRole("button", { name: "Before" }).focus();
    await user.tab();
    expect(cell("production")).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    for (const checkbox of screen.getAllByRole("checkbox")) expect(checkbox).toHaveAttribute("tabindex", "-1");
    expect(screen.getByRole("button", { name: "Copy stage" })).toHaveAttribute("tabindex", "-1");
    expect(screen.getAllByRole("gridcell").filter((c) => c.tabIndex === 0)).toHaveLength(1);
  });

  it("moves with the arrows, j and k, Home and End, and Tab comes back to the last row", async () => {
    const user = userEvent.setup();
    render(
      <>
        <List aria-label="Keys" items={items} />
        <button type="button">After</button>
      </>,
    );
    await user.tab();
    await user.keyboard("{ArrowDown}");
    expect(cell("read-only")).toHaveFocus();
    await user.keyboard("j");
    expect(cell("staging")).toHaveFocus();
    await user.keyboard("{ArrowUp}k");
    expect(cell("production")).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(cell("production")).toHaveFocus();
    await user.keyboard("{End}");
    expect(cell("development")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(cell("development")).toHaveFocus();
    await user.tab();
    await user.tab({ shift: true });
    expect(cell("development")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(cell("production")).toHaveFocus();
    expect(cell("production")).toHaveAttribute("tabindex", "0");
    expect(cell("development")).toHaveAttribute("tabindex", "-1");
  });

  it("selects with Space (or x), opens with Enter, and selects ranges with Shift", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    const onSelected = vi.fn();
    render(<List aria-label="Keys" items={items} selectable selectionBar={false} onOpen={onOpen} onSelectedChange={onSelected} />);
    await user.tab();
    await user.keyboard(" ");
    expect(cell("production")).toHaveAttribute("aria-selected", "true");
    await user.keyboard(" ");
    expect(cell("production")).toHaveAttribute("aria-selected", "false");
    await user.keyboard("x");
    expect(cell("production")).toHaveAttribute("aria-selected", "true");
    expect(onOpen).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: "prod" }));
    await user.keyboard("{Shift>}{ArrowDown}{ArrowDown}{/Shift}");
    expect([...onSelected.mock.lastCall![0]]).toEqual(["prod", "prod-ro", "stage"]);
    expect(cell("staging")).toHaveFocus();
    await user.keyboard("{Control>}a{/Control}");
    expect(onSelected.mock.lastCall![0].size).toBe(4);
    await user.keyboard("{Escape}");
    expect(onSelected.mock.lastCall![0].size).toBe(0);
  });

  it("leaves Enter with a modifier to the surrounding form: the dialog submits, the row neither opens nor toggles", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    const onSubmit = vi.fn();
    render(
      <Dialog defaultOpen title="Add models" onSubmit={onSubmit}>
        <List aria-label="Models" items={items} selectable selectionBar={false} onOpen={onOpen} />
      </Dialog>,
    );
    (await screen.findByText("staging")).closest<HTMLElement>("[role=gridcell]")!.focus();
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(onSubmit).toHaveBeenCalledTimes(2);
    expect(onOpen).not.toHaveBeenCalled();
    expect(cell("staging")).toHaveAttribute("aria-selected", "false");
  });

  it("leaves Alt and the modifiers it doesn't use to the page", async () => {
    const user = userEvent.setup();
    const outside: string[] = [];
    render(
      <div onKeyDown={(e: KeyboardEvent) => outside.push(`${e.altKey ? "alt+" : ""}${e.metaKey ? "meta+" : ""}${e.key}:${e.defaultPrevented}`)}>
        <List aria-label="Keys" items={items} selectable />
      </div>,
    );
    await user.tab();
    await user.keyboard("{Alt>}{ArrowDown}{/Alt}{Meta>}{ArrowDown}{/Meta}");
    expect(cell("production")).toHaveFocus();
    expect(outside).toEqual(expect.arrayContaining(["alt+ArrowDown:false", "meta+ArrowDown:false"]));
  });

  it("reaches a row's actions with the right arrow and comes back with the left arrow or Escape", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    render(<List aria-label="Keys" items={withActions(items.slice(1))} selectable onOpen={onOpen} />);
    await user.tab();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Copy stage" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "More for stage" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "More for stage" })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: "Copy stage" })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(cell("staging")).toHaveFocus();
    await user.keyboard("{ArrowLeft}{ArrowRight}{Escape}");
    expect(cell("staging")).toHaveFocus();
    await user.keyboard("{ArrowRight}{ArrowDown}");
    expect(cell("development")).toHaveFocus();
    await user.keyboard("{ArrowRight} ");
    expect(cell("development")).toHaveAttribute("aria-selected", "false");
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("moves through group headers too, which collapse with Enter and reach their add button with the right arrow", async () => {
    const user = userEvent.setup();
    const onAdd = vi.fn();
    const groups: ListGroup[] = [
      { id: "active", title: "Active", items: [items[1]!], onAdd, addLabel: "Create key" },
      { id: "archived", title: "Archived", items: [items[2]!] },
    ];
    render(<List aria-label="Keys" groups={groups} />);
    await user.tab();
    const active = screen.getByRole("button", { name: /Active/ });
    expect(active).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Create key" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(onAdd).toHaveBeenCalledTimes(1);
    await user.tab();
    await user.tab({ shift: true });
    expect(active).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(cell("staging")).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("button", { name: /Archived/ })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.queryByText("development")).toBeNull();
    expect(screen.getByRole("button", { name: /Archived/ })).toHaveAttribute("aria-expanded", "false");
    await user.keyboard(" ");
    expect(screen.getByText("development")).toBeInTheDocument();
  });

  it("neither opens nor selects a disabled row, and skips it when selecting all or a range", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    const onSelected = vi.fn();
    render(
      <List
        aria-label="Models"
        items={[items[1]!, { id: "old", title: "legacy", disabled: true }, items[2]!]}
        selectable
        selectionBar={false}
        onOpen={onOpen}
        onSelectedChange={onSelected}
      />,
    );
    await user.tab();
    await user.keyboard("{ArrowDown} {Enter}");
    expect(onOpen).not.toHaveBeenCalled();
    expect(onSelected).not.toHaveBeenCalled();
    await user.keyboard("{ArrowUp}{Shift>}{ArrowDown}{ArrowDown}{/Shift}");
    expect([...onSelected.mock.lastCall![0]]).toEqual(["stage", "dev"]);
    await user.keyboard("{Meta>}a{/Meta}");
    expect([...onSelected.mock.lastCall![0]]).toEqual(["stage", "dev"]);
  });

  it("only marks rows selected or not when the list is selectable", () => {
    render(<List aria-label="Keys" items={items} />);
    for (const c of screen.getAllByRole("gridcell")) expect(c).not.toHaveAttribute("aria-selected");
  });

  it("toggles a row while selecting, clears from the bar, and forwards refs", async () => {
    const ref = createRef<HTMLDivElement>();
    const fnRef = vi.fn();
    const { rerender } = render(<List ref={ref} aria-label="Keys" items={items} selectable />);
    expect(ref.current).not.toBeNull();
    await userEvent.click(screen.getByRole("checkbox", { name: "Select production" }));
    expect(screen.getByRole("region", { name: "Selection" })).toHaveTextContent("1");
    await userEvent.click(screen.getByText("staging"));
    expect(screen.getByRole("region", { name: "Selection" })).toHaveTextContent("2");
    await userEvent.click(screen.getByRole("button", { name: /Clear/ }));
    expect(screen.queryByRole("region", { name: "Selection" })).toBeNull();
    rerender(<List ref={fnRef} aria-label="Keys" items={items} />);
    expect(fnRef).toHaveBeenCalled();
  });

  it("opens a row on click", async () => {
    const onOpen = vi.fn();
    render(<List aria-label="Keys" items={items} onOpen={onOpen} />);
    await userEvent.click(screen.getByText("development"));
    expect(onOpen).toHaveBeenLastCalledWith(expect.objectContaining({ id: "dev" }));
    expect(cell("development")).toHaveFocus();
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
