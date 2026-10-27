import { createRef } from "react";
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
    expect(screen.getByRole("listbox")).toHaveTextContent("Nothing here");
  });

  it("shows a loading bar for a loading group and reports the query", async () => {
    const onQueryChange = vi.fn();
    const { container } = render(<CommandPalette inline groups={[{ label: "Search", items: [], loading: true }]} onQueryChange={onQueryChange} />);
    expect(container.querySelector(".animate-indeterminate")).not.toBeNull();
    await userEvent.type(screen.getByRole("combobox"), "a");
    expect(onQueryChange).toHaveBeenLastCalledWith("a");
    expect(screen.queryByText("No results")).toBeNull();
  });

  it("shows the query it is given and reports what is typed on top of it", async () => {
    const onQueryChange = vi.fn();
    const { rerender } = render(<CommandPalette inline groups={groups()} query="req" onQueryChange={onQueryChange} />);
    const input = screen.getByRole("combobox");
    expect(input).toHaveValue("req");
    await userEvent.type(input, "u");
    expect(onQueryChange).toHaveBeenLastCalledWith("requ");
    rerender(<CommandPalette inline groups={groups()} query="keys" onQueryChange={onQueryChange} />);
    expect(input).toHaveValue("keys");
    expect(screen.getByRole("option", { name: /API keys/ })).toBeInTheDocument();
  });

  it("tells the host when entering a page clears the query", async () => {
    const onQueryChange = vi.fn();
    render(<CommandPalette inline groups={groups()} defaultQuery="swi" onQueryChange={onQueryChange} />);
    await userEvent.click(screen.getByRole("option", { name: /Switch project/ }));
    expect(onQueryChange).toHaveBeenLastCalledWith("");
    expect(screen.getByRole("combobox")).toHaveValue("");
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

  it("keeps its top while the results change, and watches its list so its height follows them smoothly", async () => {
    const watched: Element[] = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe(el: Element) {
          watched.push(el);
        }
        unobserve(el: Element) {
          watched.splice(watched.indexOf(el), 1);
        }
        disconnect() {
          watched.length = 0;
        }
      },
    );
    render(<CommandPalette open groups={groups()} />);
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    expect(palette.parentElement).toHaveClass("items-start", "pt-palette");
    expect(palette).toHaveClass("flex", "flex-col");
    const rows = screen.getByRole("listbox").firstElementChild;
    expect(watched).toContain(rows);
    await userEvent.type(screen.getByRole("combobox"), "keys");
    expect(screen.getAllByRole("option").map((el) => el.textContent)).toEqual(["API keys"]);
    expect(watched).toContain(screen.getByRole("listbox").firstElementChild);
    vi.unstubAllGlobals();
  });
  it("follows a controlled open and ignores the hotkey when it is turned off", () => {
    const onOpenChange = vi.fn();
    render(<CommandPalette open hotkey={false} groups={groups()} onOpenChange={onOpenChange} />);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("keeps its groups' order while filtering, marks its field and captions a group that is still loading", async () => {
    render(
      <CommandPalette
        inline
        groups={[
          { label: "Pages", items: [{ id: "models", label: "Models", keywords: ["alias"] }] },
          { label: "Headings", items: [{ id: "aliases", label: "Aliases" }] },
          { label: "Find", loading: true, items: [] },
        ]}
      />,
    );
    const input = screen.getByRole("combobox");
    expect(input).toHaveAttribute("data-palette-input");
    expect(screen.getByText("Find")).toBeInTheDocument();
    await userEvent.type(input, "alias");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Models", "Aliases"]);
    expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Find")).toBeInTheDocument();
  });

  it("makes the hovered row active", () => {
    render(<CommandPalette inline groups={groups()} />);
    fireEvent.mouseMove(screen.getByRole("option", { name: /API keys/ }));
    expect(screen.getByRole("option", { name: /API keys/ })).toHaveAttribute("aria-selected", "true");
  });

  it("forwards its ref to the dialog when open, and to the panel when inline", () => {
    const ref = createRef<HTMLDivElement>();
    const { unmount } = render(<CommandPalette ref={ref} open groups={groups()} />);
    expect(ref.current).toBe(screen.getByRole("dialog", { name: "Command palette" }));
    unmount();
    render(<CommandPalette ref={ref} inline groups={groups()} />);
    expect(ref.current).toHaveClass("rounded-palette");
    expect(ref.current).toContainElement(screen.getByRole("combobox"));
  });
});

describe("CommandPalette pages", () => {
  it("goes back from a nested page with its chip, with Escape on an empty query, and clears a query first", async () => {
    const onOpenChange = vi.fn();
    render(<CommandPalette open onOpenChange={onOpenChange} hotkey={false} groups={groups()} />);
    await userEvent.click(await screen.findByRole("option", { name: /Switch project/ }));
    const back = screen.getByRole("button", { name: "Switch project, go back" });
    expect(screen.getByRole("combobox")).toHaveAttribute("placeholder", "Search");
    await userEvent.click(back);
    expect(screen.getByRole("option", { name: /Request logs/ })).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveFocus();
    await userEvent.click(screen.getByRole("option", { name: /Switch project/ }));
    await userEvent.type(screen.getByRole("combobox"), "chec");
    await userEvent.keyboard("{Escape}");
    expect(screen.getByRole("combobox")).toHaveValue("");
    expect(screen.getByRole("option", { name: "Checkout" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(screen.getByRole("option", { name: /Request logs/ })).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalled();
    await userEvent.keyboard("{Escape}");
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
  });

  it("closes on Escape from the first page, even with a search typed", async () => {
    const onOpenChange = vi.fn();
    render(<CommandPalette open onOpenChange={onOpenChange} hotkey={false} groups={groups()} />);
    await userEvent.type(await screen.findByRole("combobox"), "logs");
    await userEvent.keyboard("{Escape}");
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
  });

  it("names a result by its label first, with its hint after a pause", () => {
    render(<CommandPalette inline groups={groups()} />);
    expect(screen.getByRole("option", { name: /^Request logs\s*,\s*Every call\s*,\s*G\s*L$/i })).toBeInTheDocument();
  });

  it("keeps ⌘K from opening a second dialog over another one", () => {
    const onOpenChange = vi.fn();
    render(
      <>
        <div role="dialog" aria-label="Create key" data-open="" />
        <CommandPalette onOpenChange={onOpenChange} groups={groups()} />
      </>,
    );
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("captions a group that is still loading without a count, with a loading row under it", () => {
    render(<CommandPalette inline groups={[{ label: "Logs", items: [], loading: true }, ...groups()]} />);
    const caption = screen.getByText("Logs");
    expect(caption.parentElement).toHaveTextContent(/^Logs$/);
    expect(screen.getByText("Loading…")).toBeInTheDocument();
  });
});
