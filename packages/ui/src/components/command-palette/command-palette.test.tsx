import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandPalette, type CommandGroup } from "./command-palette.js";

function groups(onSelect = vi.fn()): CommandGroup[] {
  return [
    {
      label: "Go to",
      items: [
        { id: "logs", label: "Request logs", hint: "Every call", keywords: ["history"], shortcut: ["g", "l"], onSelect },
        { id: "keys", label: "API keys", onSelect },
      ],
    },
    {
      label: "Projects",
      items: [
        {
          id: "switch",
          label: "Switch project",
          children: [{ items: [{ id: "checkout", label: "Checkout", onSelect }] }],
        },
      ],
    },
  ];
}

describe("CommandPalette", () => {
  it("filters by every word of the query, across label, hint and keywords", async () => {
    render(<CommandPalette inline groups={groups()} />);
    await userEvent.type(screen.getByRole("combobox"), "history every");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual([expect.stringContaining("Request logs")]);
  });

  it("moves with the arrows, wraps around, and runs the active command with Enter", async () => {
    const onSelect = vi.fn();
    render(<CommandPalette inline groups={groups(onSelect)} />);
    const input = screen.getByRole("combobox");
    await userEvent.type(input, "{ArrowUp}");
    expect(screen.getByRole("option", { name: /Switch project/ })).toHaveAttribute("aria-selected", "true");
    await userEvent.type(input, "{ArrowDown}{ArrowDown}");
    expect(screen.getByRole("option", { name: /API keys/ })).toHaveAttribute("aria-selected", "true");
    await userEvent.type(input, "{End}");
    expect(screen.getByRole("option", { name: /Switch project/ })).toHaveAttribute("aria-selected", "true");
    await userEvent.type(input, "{Home}{Enter}");
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("starts again at the first result when the query or the page changes", async () => {
    render(<CommandPalette inline groups={groups()} />);
    const input = screen.getByRole("combobox");
    await userEvent.type(input, "{ArrowUp}");
    await userEvent.type(input, "s");
    expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    await userEvent.type(input, "{Backspace}{ArrowUp}{Enter}");
    expect(screen.getByRole("option", { name: "Checkout" })).toHaveAttribute("aria-selected", "true");
    await userEvent.type(input, "{Backspace}");
    expect(screen.getByRole("option", { name: /Request logs/ })).toHaveAttribute("aria-selected", "true");
  });

  it("opens a nested page and goes back with Backspace on an empty query", async () => {
    render(<CommandPalette inline keyHints groups={groups()} />);
    await userEvent.click(screen.getByRole("option", { name: /Switch project/ }));
    expect(screen.getByRole("option", { name: "Checkout" })).toBeInTheDocument();
    expect(screen.getByText("to go back")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("combobox"), "{Backspace}");
    expect(screen.getByRole("option", { name: /Request logs/ })).toBeInTheDocument();
  });

  it("says when nothing matches, and the arrows do nothing then", async () => {
    render(<CommandPalette inline groups={groups()} emptyText="Nothing here" defaultQuery="zzz" />);
    const input = screen.getByRole("combobox");
    await userEvent.type(input, "{ArrowDown}{ArrowUp}{Enter}");
    expect(screen.getByRole("status")).toHaveTextContent("Nothing here");
  });

  it("shows a loading bar for a loading group and reports the query", async () => {
    const onQueryChange = vi.fn();
    const { container } = render(<CommandPalette inline groups={[{ label: "Search", items: [], loading: true }]} onQueryChange={onQueryChange} />);
    expect(container.querySelector(".animate-indeterminate")).not.toBeNull();
    await userEvent.type(screen.getByRole("combobox"), "a");
    expect(onQueryChange).toHaveBeenLastCalledWith("a");
    expect(screen.queryByText("No results")).toBeNull();
  });

  it("opens and closes with ⌘K and closes after running a command", async () => {
    const onSelect = vi.fn();
    const onOpenChange = vi.fn();
    render(<CommandPalette groups={groups(onSelect)} onOpenChange={onOpenChange} />);
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("option", { name: /API keys/ }));
    expect(onSelect).toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    fireEvent.keyDown(window, { key: "K", ctrlKey: true });
    expect(onOpenChange).toHaveBeenLastCalledWith(true);
  });

  it("follows a controlled open and ignores the hotkey when it is turned off", () => {
    const onOpenChange = vi.fn();
    render(<CommandPalette open hotkey={false} groups={groups()} onOpenChange={onOpenChange} />);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("makes the hovered row active", () => {
    render(<CommandPalette inline groups={groups()} />);
    fireEvent.mouseMove(screen.getByRole("option", { name: /API keys/ }));
    expect(screen.getByRole("option", { name: /API keys/ })).toHaveAttribute("aria-selected", "true");
  });
});
