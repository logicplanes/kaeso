import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandList, type CommandListGroup } from "./command-list.js";
import { PickerPopover, PickerTrigger } from "./picker-popover.js";
import { commandMatch } from "./match.js";
import { Dialog } from "../dialog/dialog.js";
import { renderToString } from "react-dom/server";

const groups: CommandListGroup[] = [
  {
    id: "openai",
    label: "OpenAI",
    count: 3,
    items: [
      { value: "gpt-5", label: "GPT-5", keywords: ["gpt-5", "OpenAI"] },
      { value: "gpt-5-mini", label: "GPT-5 mini", keywords: ["gpt-5-mini", "OpenAI"] },
      { value: "gpt-4.1", label: "GPT-4.1", keywords: ["gpt-4.1", "OpenAI"], disabled: true },
    ],
  },
  {
    id: "anthropic",
    label: "Anthropic",
    count: 1,
    items: [{ value: "claude-sonnet-4-5", label: "Claude Sonnet 4.5", keywords: ["claude-sonnet-4-5", "Anthropic"], meta: "200K" }],
  },
];

const many = (count: number): CommandListGroup[] =>
  Array.from({ length: 10 }, (_, g) => ({
    id: `p${g}`,
    label: `Provider ${g}`,
    items: Array.from({ length: count / 10 }, (_, i) => ({ value: `p${g}-m${i}`, label: `Model ${g}-${i}` })),
  }));

function Harness({ onSelect = () => {}, multiple = false }: { onSelect?: (v: string) => void; multiple?: boolean }) {
  const [value, setValue] = useState<string[]>([]);
  return (
    <CommandList
      groups={groups}
      aria-label="Models"
      multiple={multiple}
      value={value}
      autoFocus
      onSelect={(v) => {
        onSelect(v);
        setValue((cur) => (multiple ? (cur.includes(v) ? cur.filter((x) => x !== v) : [...cur, v]) : [v]));
      }}
    />
  );
}

const active = () => {
  const id = screen.getByRole("combobox").getAttribute("aria-activedescendant");
  return id ? document.getElementById(id) : null;
};

describe("commandMatch", () => {
  it("ranks exact, prefix, word and loose matches, and needs every word", () => {
    expect(commandMatch("gpt-5", ["gpt-5"])).toBeGreaterThan(commandMatch("gpt", ["gpt-5"]));
    expect(commandMatch("mini", ["GPT-5 mini"])).toBeGreaterThan(commandMatch("ini", ["GPT-5 mini"]));
    expect(commandMatch("gpt5mini", ["gpt-5-mini"])).toBeGreaterThan(0);
    expect(commandMatch("snt", ["claude-sonnet"])).toBeGreaterThan(0);
    expect(commandMatch("sonnet anthropic", ["Claude Sonnet 4.5", "Anthropic"])).toBeGreaterThan(0);
    expect(commandMatch("sonnet openai", ["Claude Sonnet 4.5", "Anthropic"])).toBe(0);
    expect(commandMatch("zq", ["gpt-5"])).toBe(0);
    expect(commandMatch("  ", ["gpt-5"])).toBe(1);
  });
});

describe("CommandList", () => {
  it("lists groups under captions with counts and names each option by its group", () => {
    render(<Harness />);
    const list = screen.getByRole("listbox", { name: "Models" });
    expect(
      within(list)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["GPT-5", "GPT-5 mini", "GPT-4.1", "Claude Sonnet 4.5200K"]);
    expect(list).toHaveTextContent("OpenAI3");
    expect(within(screen.getByRole("group", { name: "Anthropic" })).getByRole("option")).toHaveTextContent("Claude Sonnet 4.5");
    expect(screen.getByRole("option", { name: "GPT-4.1" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("combobox", { name: "Search" })).toHaveFocus();
  });

  it("leaves the group counts out when asked, with or without a query", async () => {
    const user = userEvent.setup();
    render(<CommandList groups={groups} aria-label="Models" counts={false} autoFocus />);
    const list = screen.getByRole("listbox", { name: "Models" });
    expect(list).not.toHaveTextContent("OpenAI3");
    await user.type(screen.getByRole("combobox"), "gpt");
    expect(list).toHaveTextContent("OpenAI");
    expect(list).not.toHaveTextContent(/OpenAI\d/);
  });

  it("moves with the arrow, Home and End keys, skips disabled rows and chooses with Enter", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<Harness onSelect={onSelect} />);
    expect(active()).toHaveTextContent("GPT-5");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(active()).toHaveTextContent("Claude Sonnet 4.5");
    await user.keyboard("{ArrowDown}");
    expect(active()).toHaveTextContent("GPT-5");
    await user.keyboard("{ArrowUp}");
    expect(active()).toHaveTextContent("Claude Sonnet 4.5");
    await user.keyboard("{Home}");
    expect(active()).toHaveTextContent("GPT-5");
    await user.keyboard("{End}{PageUp}{PageDown}{Enter}");
    expect(onSelect).toHaveBeenCalledWith("claude-sonnet-4-5");
    expect(screen.getByRole("option", { name: /Claude Sonnet/ })).toHaveAttribute("aria-selected", "true");
  });

  it("filters fuzzily, puts the best group first and says when nothing matches", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.type(screen.getByRole("combobox"), "gpt-5");
    expect(screen.getByRole("group", { name: "OpenAI" })).toHaveTextContent(/^OpenAI2GPT/);
    await user.clear(screen.getByRole("combobox"));
    await user.type(screen.getByRole("combobox"), "sonnet");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Claude Sonnet 4.5200K"]);
    expect(active()).toHaveTextContent("Claude Sonnet 4.5");
    await user.clear(screen.getByRole("combobox"));
    await user.type(screen.getByRole("combobox"), "mini");
    expect(screen.getAllByRole("option")[0]).toHaveTextContent("GPT-5 mini");
    await user.type(screen.getByRole("combobox"), "zzz");
    expect(screen.getByRole("status")).toHaveTextContent("No results");
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("combobox")).toHaveValue("");
    expect(screen.getAllByRole("option")).toHaveLength(4);
  });

  it("chooses the best match when Enter follows the typing at once", async () => {
    const onSelect = vi.fn();
    render(<CommandList groups={many(5000)} aria-label="Models" autoFocus onSelect={onSelect} />);
    const search = screen.getByRole("combobox");
    fireEvent.change(search, { target: { value: "Model 7-42" } });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledWith("p7-m42");
  });

  it("toggles several rows with checkboxes", async () => {
    const user = userEvent.setup();
    render(<Harness multiple />);
    expect(screen.getByRole("listbox")).toHaveAttribute("aria-multiselectable", "true");
    await user.click(screen.getByRole("option", { name: "GPT-5" }));
    await user.click(screen.getByRole("option", { name: "GPT-5 mini" }));
    await user.click(screen.getByRole("option", { name: "GPT-4.1" }));
    expect(screen.getAllByRole("option", { selected: true }).map((o) => o.textContent)).toEqual(["GPT-5", "GPT-5 mini"]);
    await user.click(screen.getByRole("option", { name: "GPT-5" }));
    expect(screen.getAllByRole("option", { selected: true }).map((o) => o.textContent)).toEqual(["GPT-5 mini"]);
  });

  it("renders only the rows in view for thousands of models and keeps the active one", async () => {
    const user = userEvent.setup();
    render(<CommandList groups={many(5000)} aria-label="Models" autoFocus />);
    const rendered = screen.getAllByRole("option");
    expect(rendered.length).toBeLessThan(60);
    expect(rendered[0]).toHaveAttribute("aria-setsize", "5000");
    await user.keyboard("{End}");
    expect(active()).toHaveTextContent("Model 9-499");
    expect(active()).toHaveAttribute("aria-posinset", "5000");
    expect(screen.getAllByRole("option").length).toBeLessThan(80);
  });

  it("starts on the chosen row and calls onEscape once the search is empty", async () => {
    const user = userEvent.setup();
    const onEscape = vi.fn();
    render(<CommandList groups={groups} aria-label="Models" value="claude-sonnet-4-5" onEscape={onEscape} placeholder="Search models" autoFocus />);
    expect(active()).toHaveTextContent("Claude Sonnet 4.5");
    expect(screen.getByRole("combobox", { name: "Search models" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(onEscape).toHaveBeenCalledTimes(1);
  });
});

describe("PickerPopover", () => {
  it("opens a searchable list from its trigger and closes after a choice", async () => {
    const user = userEvent.setup();
    function Picker() {
      const [open, setOpen] = useState(false);
      const [value, setValue] = useState<string | null>(null);
      return (
        <PickerPopover
          title="Model"
          open={open}
          onOpenChange={setOpen}
          trigger={
            <PickerTrigger aria-label="Model" placeholder="Choose a model">
              {value ?? undefined}
            </PickerTrigger>
          }
        >
          <CommandList
            groups={groups}
            aria-label="Models"
            value={value}
            autoFocus
            onSelect={(v) => {
              setValue(v);
              setOpen(false);
            }}
          />
        </PickerPopover>
      );
    }
    render(<Picker />);
    const trigger = screen.getByRole("button", { description: "Model" });
    expect(trigger).toHaveAccessibleName("Choose a model");
    await user.click(trigger);
    expect(await screen.findByRole("listbox", { name: "Models" })).toBeInTheDocument();
    await user.keyboard("mini{Enter}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(trigger).toHaveAccessibleName("gpt-5-mini");
    expect(trigger).not.toHaveAttribute("aria-label");
  });

  it("clears the search with Escape inside a dialog, then closes only the picker and returns focus", async () => {
    const user = userEvent.setup();
    const onDialog = vi.fn();
    render(
      <Dialog open onOpenChange={onDialog} title="Add models">
        <PickerPopover title="Provider" trigger={<PickerTrigger aria-label="Provider" placeholder="Choose a provider" />}>
          <CommandList groups={groups} aria-label="Providers" autoFocus />
        </PickerPopover>
      </Dialog>,
    );
    const trigger = screen.getByRole("button", { description: "Provider" });
    await user.click(trigger);
    const search = await screen.findByRole("combobox", { name: "Search" });
    await user.type(search, "gpt");
    await user.keyboard("{Escape}");
    expect(search).toHaveValue("");
    expect(screen.getByRole("listbox", { name: "Providers" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox", { name: "Providers" })).not.toBeInTheDocument());
    expect(screen.getByRole("dialog", { name: "Add models" })).toBeInTheDocument();
    expect(onDialog).not.toHaveBeenCalled();
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("is the field a dialog step focuses first", async () => {
    render(
      <Dialog open title="Create key">
        <PickerPopover title="Models" trigger={<PickerTrigger aria-label="Models" placeholder="All models" />}>
          <CommandList groups={groups} aria-label="Models" />
        </PickerPopover>
      </Dialog>,
    );
    await waitFor(() => expect(screen.getByRole("button", { description: "Models" })).toHaveFocus());
  });

  it("opens as a full-screen sheet with a title on phones", async () => {
    const user = userEvent.setup();
    const original = window.matchMedia;
    window.matchMedia = ((q: string) =>
      ({
        matches: q.includes("max-width"),
        media: q,
        addEventListener() {},
        removeEventListener() {},
      }) as unknown as MediaQueryList) as typeof window.matchMedia;
    render(
      <PickerPopover title="Provider" trigger={<PickerTrigger aria-label="Provider" placeholder="Choose a provider" />}>
        <CommandList groups={groups} aria-label="Providers" />
      </PickerPopover>,
    );
    await user.click(screen.getByRole("button", { description: "Provider" }));
    const sheet = await screen.findByRole("dialog", { name: "Provider" });
    expect(within(sheet).getByRole("listbox", { name: "Providers" })).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    window.matchMedia = original;
  });
});

describe("CommandList outside a browser", () => {
  it("renders on the server and without matchMedia", () => {
    const html = renderToString(
      <PickerPopover title="Model" trigger={<PickerTrigger aria-label="Model" placeholder="Choose a model" />}>
        <CommandList groups={groups} aria-label="Models" />
      </PickerPopover>,
    );
    expect(html).toContain("Choose a model");
    expect(renderToString(<CommandList groups={groups} aria-label="Models" />)).toContain("GPT-5");
    const original = window.matchMedia;
    Object.defineProperty(window, "matchMedia", { value: undefined, configurable: true });
    try {
      render(<CommandList groups={groups} aria-label="Models" />);
      expect(screen.getAllByRole("option")).toHaveLength(4);
      render(
        <PickerPopover title="Model" trigger={<PickerTrigger aria-label="Model" />}>
          {null}
        </PickerPopover>,
      );
      expect(screen.getByRole("button", { name: "Model" })).not.toHaveAttribute("aria-describedby");
    } finally {
      Object.defineProperty(window, "matchMedia", { value: original, configurable: true });
    }
  });
});

describe("CommandList rows", () => {
  it("keep the same 6px edge below the last row as above the first", () => {
    render(<CommandList aria-label="Models" groups={groups} value={null} onSelect={() => {}} />);
    const rows = screen.getByRole("listbox", { name: "Models" }).querySelector<HTMLElement>(":scope > [role=presentation]");
    expect(rows?.style.height).toBe(`${2 * 6 + 2 * 28 + 4 * 34}px`);
  });
});

describe("CommandList inline in a dialog step", () => {
  it("is as tall as its rows, with no room left below the last one, and keeps that height while searching", async () => {
    const { container } = render(<CommandList aria-label="Models" groups={groups} value={null} onSelect={() => {}} height="inline" />);
    const frame = container.firstElementChild as HTMLElement;
    expect(frame.style.height).toBe(`${44 + 2 * 6 + 2 * 28 + 4 * 34}px`);
    await userEvent.type(screen.getByRole("combobox"), "claude");
    expect(frame.style.height).toBe(`${44 + 2 * 6 + 2 * 28 + 4 * 34}px`);
  });

  it("leaves no room for a labelled group with no rows", () => {
    const withEmpty = [...groups, { id: "mistral", label: "Mistral", items: [] }];
    const { container } = render(<CommandList aria-label="Models" groups={withEmpty} value={null} onSelect={() => {}} height="inline" />);
    expect((container.firstElementChild as HTMLElement).style.height).toBe(`${44 + 2 * 6 + 2 * 28 + 4 * 34}px`);
  });

  it("stops at 360px and scrolls when the rows are taller", () => {
    const { container } = render(<CommandList aria-label="Models" groups={many(40)} value={null} onSelect={() => {}} height="inline" />);
    expect(container.firstElementChild).toHaveClass("max-h-360");
  });

  it("ends a group's count on the same edge as its options' meta, past the check column a single choice keeps", () => {
    render(<Harness />);
    const header = screen.getByText("Anthropic", { selector: "span" }).parentElement!;
    const option = screen.getByRole("option", { name: /Claude Sonnet 4.5/ });
    expect(header).toHaveClass("gap-10");
    expect(option).toHaveClass("gap-10");
    for (const [row, text] of [
      [header, "1"],
      [option, "200K"],
    ] as const) {
      expect(row.lastElementChild).toHaveClass("size-16", "shrink-0");
      expect(row.lastElementChild!.previousElementSibling).toHaveTextContent(text);
    }
  });

  it("ends a group's count at the row's end when there is no check column", () => {
    render(<CommandList aria-label="Commands" groups={groups} onSelect={() => {}} />);
    expect(screen.getByText("Anthropic", { selector: "span" }).parentElement!.lastElementChild).toHaveTextContent("1");
  });

  it("draws a picker's trigger like a text input", () => {
    render(<PickerTrigger aria-label="Model" placeholder="Choose a model" />);
    expect(screen.getByRole("button", { name: /Choose a model/ })).toHaveClass("material-selector");
  });
});
