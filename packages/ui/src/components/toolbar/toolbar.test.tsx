import { createRef, useState } from "react";
import { describe, expect, it } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Toolbar } from "./toolbar.js";
import { FilterChip } from "../filter-chip/filter-chip.js";
import { Segmented } from "../segmented/segmented.js";

describe("Toolbar.Spacer", () => {
  it("forwards its ref to the space it fills", () => {
    const ref = createRef<HTMLSpanElement>();
    render(
      <Toolbar aria-label="Filters">
        <button type="button">Status</button>
        <Toolbar.Spacer ref={ref} />
        <button type="button">Export CSV</button>
      </Toolbar>,
    );
    expect(screen.getByRole("toolbar", { name: "Filters" })).toContainElement(ref.current);
    expect(ref.current).toHaveClass("flex-1");
    expect(ref.current).toHaveAttribute("aria-hidden");
  });
});

describe("Toolbar keyboard", () => {
  const bar = (
    <>
      <button type="button">Before</button>
      <Toolbar aria-label="Filters">
        <FilterChip property="Status" operator="is" value="2xx" onRemove={() => undefined} />
        <button type="button">Add filter</button>
        <button type="button" disabled>
          Export
        </button>
        <button type="button">Display options</button>
      </Toolbar>
      <button type="button">After</button>
    </>
  );

  it("is one tab stop, and Tab comes back to the control that last had focus", async () => {
    render(bar);
    screen.getByRole("button", { name: "Before" }).focus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: /Remove/ })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Add filter" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Add filter" })).toHaveFocus();
  });

  it("walks past a chip's static parts and disabled controls, and wraps at the ends", async () => {
    render(bar);
    screen.getByRole("button", { name: /Remove/ }).focus();
    await userEvent.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.getByRole("button", { name: "Display options" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: /Remove/ })).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(screen.getByRole("button", { name: "Display options" })).toHaveFocus();
  });
});

describe("Toolbar with a radio group in it", () => {
  let choose: (v: string) => void = () => undefined;
  function Bar() {
    const [kind, setKind] = useState<string | undefined>(undefined);
    choose = setKind;
    return (
      <>
        <button type="button">Before</button>
        <Toolbar aria-label="Filter requests">
          <Segmented
            size="sm"
            aria-label="What to show"
            value={kind ?? ""}
            onValueChange={setKind}
            options={[
              { value: "models", label: "Models" },
              { value: "tools", label: "Tools" },
            ]}
          />
          <button type="button">Last 24 hours</button>
          <button type="button">Add filter</button>
        </Toolbar>
        <button type="button">After</button>
      </>
    );
  }

  it("leaves the group its own tab stop and arrow keys, and Tab reaches the toolbar's controls whenever the group was chosen", async () => {
    render(<Bar />);
    act(() => choose("models"));
    await act(() => new Promise((r) => setTimeout(r, 0)));
    screen.getByRole("button", { name: "Before" }).focus();
    await userEvent.tab();
    expect(screen.getByRole("radio", { name: "Models" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Last 24 hours" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Add filter" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Last 24 hours" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    await userEvent.tab({ shift: true });
    expect(screen.getByRole("radio", { name: "Models" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("radio", { name: "Tools" })).toHaveFocus();
    expect(screen.getByRole("radio", { name: "Tools" })).toBeChecked();
  });
});
