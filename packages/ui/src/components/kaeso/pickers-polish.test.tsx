import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderPicker } from "./provider-picker.js";
import { RolePicker } from "./role-picker.js";
import { Select } from "../select/select.js";
import { Combobox, Autocomplete } from "../combobox/combobox.js";
import { FilterChip } from "../filter-chip/filter-chip.js";
import { FilterRow } from "../filter-row/filter-row.js";
import { ThemeMenu } from "../theme-menu/theme-menu.js";
import { ThemeProvider } from "../../theme/theme-provider.js";
import { DisplayOptions } from "../display-options/display-options.js";
import { Segmented } from "../segmented/segmented.js";
import { Slider } from "../slider/slider.js";
import { PickerTrigger } from "../command-list/picker-popover.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";
import { RaisedContent } from "../../utils/inset.js";
import { icons } from "../../icons/vocabulary.js";

const providers = [
  { id: "openai", name: "OpenAI", connected: true },
  { id: "anthropic", name: "Anthropic" },
  { id: "mistral", name: "Mistral" },
];

describe("popups", () => {
  it("measure the window, not the page, so a popup at 200% zoom on a scrolled page stays on screen", async () => {
    const rect = (x: number, y: number, width: number, height: number) =>
      ({ x, y, left: x, top: y, width, height, right: x + width, bottom: y + height, toJSON: () => ({}) }) as DOMRect;
    const isPopup = (el: HTMLElement) => el.matches("[role=listbox]") || !!el.querySelector("[role=listbox]");
    const mocks = [
      vi.spyOn(document.documentElement, "clientWidth", "get").mockReturnValue(640),
      vi.spyOn(document.documentElement, "clientHeight", "get").mockReturnValue(400),
      vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
        return this === document.documentElement ? 640 : isPopup(this) ? 240 : 120;
      }),
      vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
        return this === document.documentElement ? 3000 : isPopup(this) ? 200 : 36;
      }),
      vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
        if (this === document.documentElement) return rect(0, -1200, this.offsetWidth, this.offsetHeight);
        if (this.getAttribute("role") === "combobox") return rect(560, 330, 72, 36);
        return rect(0, 0, this.offsetWidth, this.offsetHeight);
      }),
    ];
    try {
      render(
        <Select
          aria-label="Retention"
          options={[
            { value: "7", label: "7 days" },
            { value: "30", label: "30 days" },
          ]}
        />,
      );
      await userEvent.click(screen.getByRole("combobox", { name: "Retention" }));
      await act(() => lazyModulesSettled());
      const positioner = (await screen.findByRole("listbox")).closest<HTMLElement>('[data-side][style*="position: fixed"]')!;
      const place = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/;
      await waitFor(() => expect(positioner.getAttribute("style")).toMatch(place));
      const [, x, y] = place.exec(positioner.getAttribute("style")!)!.map(Number);
      expect(positioner).toHaveAttribute("data-side", "top");
      expect({ left: x, top: y, right: x! + 240, bottom: y! + 200 }).toEqual({ left: 392, top: 124, right: 632, bottom: 324 });
      expect(positioner.getAttribute("style")).toContain("--available-height: 316px");
    } finally {
      for (const mock of mocks) mock.mockRestore();
    }
  });
});

describe("Select", () => {
  it("keeps the count of more values whole while the first value truncates", () => {
    render(
      <Select
        multiple
        aria-label="Providers"
        options={providers.map((p) => ({ value: p.id, label: p.name }))}
        defaultValue={["anthropic", "openai", "mistral"]}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Providers" });
    expect(trigger).toHaveTextContent("Anthropic +2");
    expect(within(trigger).getByText("+2")).toHaveClass("shrink-0");
    expect(within(trigger).getByText("Anthropic")).toHaveClass("truncate");
  });
  it("says Not set while nothing is chosen", () => {
    render(<Select aria-label="Retention" options={[{ value: "7", label: "7 days" }]} />);
    expect(screen.getByRole("combobox", { name: "Retention" })).toHaveTextContent("Not set");
  });
});

describe("Combobox", () => {
  it("names the query when nothing matches", async () => {
    render(<Combobox aria-label="Model" options={[{ value: "gpt-5", label: "gpt-5" }]} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Model" }), "sonnet");
    expect(await screen.findByText("No matches for “sonnet”")).toBeInTheDocument();
  });
  it("offers its options from a button with chips too, and sizes the chip field", () => {
    render(<Combobox multiple size="sm" aria-label="Models" options={[{ value: "gpt-5", label: "gpt-5" }]} defaultValue={["gpt-5"]} />);
    expect(screen.getByRole("button", { name: "Show options" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Models" }).closest(".material-field")).toHaveClass("min-h-32");
  });
  it("pads a leading icon by the field's size", () => {
    const { container } = render(<Autocomplete aria-label="Path" size="sm" leadingIcon={icons.search} suggestions={[]} />);
    expect(container.querySelector(".material-field")).toHaveClass("pl-10");
  });
});

describe("ProviderPicker", () => {
  it("turns every tile off when disabled", async () => {
    const onChoose = vi.fn();
    render(<ProviderPicker appearance="grid" aria-label="Provider" providers={providers} onChoose={onChoose} disabled />);
    for (const radio of screen.getAllByRole("radio")) expect(radio).toHaveAttribute("data-disabled");
    await userEvent.click(screen.getByRole("radio", { name: /Mistral/ }));
    expect(onChoose).not.toHaveBeenCalled();
  });
  it("turns every row off in the list when disabled", () => {
    render(<ProviderPicker aria-label="Provider" providers={providers} disabled />);
    for (const option of screen.getAllByRole("option")) expect(option).toHaveAttribute("aria-disabled", "true");
  });
  it("keeps the chosen tile's ink ring under the pointer", () => {
    render(<ProviderPicker appearance="grid" aria-label="Provider" providers={providers} value="mistral" />);
    const chosen = screen.getByRole("radio", { name: /Mistral/ });
    expect(chosen).toHaveClass("data-checked:ring-step", "hover:not-data-checked:shadow-control-hover", "forced-checked");
  });
});

describe("RolePicker", () => {
  it("reads an unavailable role's reason with it, with no tooltip to hunt for", async () => {
    render(
      <RolePicker
        aria-label="Role"
        roles={[
          { value: "admin", label: "Admin", description: "Everything" },
          { value: "owner", label: "Owner", description: "Billing", disabled: true, disabledReason: "Only an owner can make someone an owner." },
        ]}
      />,
    );
    const owner = screen.getByRole("radio", { name: "Owner" });
    expect(owner).toHaveAccessibleDescription("Billing Only an owner can make someone an owner.");
    await userEvent.hover(screen.getByText("Owner"));
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    expect(screen.getByText("Owner").closest("label")).toHaveClass("px-20");
  });
});

describe("FilterChip", () => {
  it("shows an operator with nothing to choose as text, not a button", () => {
    render(<FilterChip property="Model" operator="is" value="gpt-5" />);
    const group = screen.getByRole("group", { name: "Filter: Model" });
    expect(within(group).queryByRole("button", { name: "is" })).not.toBeInTheDocument();
    expect(within(group).getByText("is")).toBeInTheDocument();
  });
  it("keeps a long value on one line and truncates it, with the whole value as the segment's name", () => {
    const value = "claude-sonnet-4-5-20250929-with-extended-thinking";
    render(<FilterChip property="Model" operator="is" value={value} values={[{ value, label: value }]} data-force="hover" />);
    const segment = screen.getByRole("button", { name: value });
    expect(segment).toHaveClass("whitespace-nowrap");
    expect(segment).toHaveAttribute("data-force", "hover");
    expect(within(segment).getByText(value)).toHaveClass("truncate");
  });
});

describe("FilterRow", () => {
  it("moves from filter to filter with the arrows, not through each part of a chip", async () => {
    render(
      <FilterRow>
        <FilterChip
          property="Model"
          operator="is"
          operators={["is", "is not"]}
          value="gpt-5"
          values={[{ value: "gpt-5", label: "gpt-5" }]}
          onRemove={() => {}}
        />
        <FilterChip property="Key" operator="is" value="checkout" values={[{ value: "checkout", label: "checkout" }]} />
      </FilterRow>,
    );
    screen.getByRole("button", { name: "is" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "checkout" })).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: "is" })).toHaveFocus();
  });
});

describe("ThemeMenu", () => {
  it("closes when a theme is chosen with Enter", async () => {
    render(
      <ThemeProvider>
        <ThemeMenu />
      </ThemeProvider>,
    );
    screen.getByRole("button", { name: "Theme" }).focus();
    await userEvent.keyboard("{Enter}");
    await screen.findByRole("menu");
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});

describe("DisplayOptions", () => {
  it("draws its view switch for a raised surface and no line above a first section", () => {
    const { container } = render(
      <DisplayOptions
        inline
        view={{
          value: "list",
          onChange: () => {},
          options: [
            { value: "list", label: "List" },
            { value: "board", label: "Board" },
          ],
        }}
      />,
    );
    expect(container.querySelector(".segment-indicator-raised")).not.toBeNull();
    expect(screen.getByRole("radiogroup", { name: "View" })).toHaveClass("bg-fill-2");
    const toggles = render(<DisplayOptions inline toggles={[{ label: "Show empty groups", checked: false, onChange: () => {} }]} />);
    expect(toggles.getByRole("switch", { name: "Show empty groups" }).closest(".flex-col.px-12")).toHaveClass("not-first:hairline-t");
  });
});

describe("Segmented", () => {
  it("keeps the panel's track and indicator outside raised surfaces, and forces a single option's state", () => {
    render(
      <>
        <Segmented
          aria-label="On the panel"
          options={[
            { value: "a", label: "A" },
            { value: "b", label: "B", "data-force": "pressed" },
          ]}
        />
        <RaisedContent>
          <Segmented aria-label="In a dialog" options={[{ value: "a", label: "A" }]} />
        </RaisedContent>
      </>,
    );
    expect(screen.getByRole("radiogroup", { name: "On the panel" })).toHaveClass("bg-fill-1");
    expect(screen.getByRole("radiogroup", { name: "In a dialog" })).toHaveClass("bg-fill-2");
    expect(screen.getByRole("radio", { name: "B" })).toHaveAttribute("data-force", "pressed");
  });
});

describe("Slider", () => {
  it("reads a range's ends as plain sentences", () => {
    render(<Slider getAriaLabel={(i) => (i === 0 ? "Minimum" : "Maximum")} defaultValue={[20, 60]} />);
    expect(screen.getByRole("slider", { name: "Minimum" })).toHaveAttribute("aria-valuetext", "Start of range: 20");
    expect(screen.getByRole("slider", { name: "Maximum" })).toHaveAttribute("aria-valuetext", "End of range: 60");
  });
});

describe("PickerTrigger", () => {
  it("keeps focus when it is disabled while focused, and ignores clicks until it blurs", async () => {
    const onClick = vi.fn();
    function Form() {
      const [busy, setBusy] = useState(false);
      return (
        <>
          <PickerTrigger aria-label="Model" disabled={busy} onClick={onClick}>
            GPT-5
          </PickerTrigger>
          <button type="button" onClick={() => setBusy(true)}>
            Save
          </button>
        </>
      );
    }
    render(<Form />);
    const trigger = screen.getByRole("button", { name: "GPT-5" });
    act(() => trigger.focus());
    act(() => screen.getByRole("button", { name: "Save" }).click());
    expect(trigger).toHaveFocus();
    expect(trigger).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Enter}");
    expect(onClick).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(trigger).toBeDisabled();
  });
});
