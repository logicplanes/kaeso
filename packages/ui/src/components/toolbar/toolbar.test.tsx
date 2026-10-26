import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Toolbar } from "./toolbar.js";
import { FilterChip } from "../filter-chip/filter-chip.js";

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
