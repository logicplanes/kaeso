import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DatePicker, DateRangePicker } from "./date-picker.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const gates = vi.hoisted(() => {
  const gate = () => {
    let open!: () => void;
    const promise = new Promise<void>((resolve) => (open = resolve));
    return { promise, open };
  };
  return { date: gate(), range: gate() };
});
vi.mock("./date-popup.js", async (importOriginal) => {
  await gates.date.promise;
  return importOriginal();
});
vi.mock("./range-popup.js", async (importOriginal) => {
  await gates.range.promise;
  return importOriginal();
});

await Promise.all([vi.importActual("./date-popup.js"), vi.importActual("./range-popup.js")]);

async function arrive(gate: { open: () => void }) {
  gate.open();
  await act(() => lazyModulesSettled());
}

const isOpen = (trigger: HTMLElement) => {
  expect(trigger).toHaveAttribute("aria-expanded", "true");
  expect(trigger).toHaveAttribute("data-popup-open");
};
const isClosed = (trigger: HTMLElement) => {
  expect(trigger).toHaveAttribute("aria-expanded", "false");
  expect(trigger).not.toHaveAttribute("data-popup-open");
};

describe("a picker clicked before its popover has loaded", () => {
  it("shows open at once, cancels on Escape, a press elsewhere or a second click, and opens when the popover arrives", async () => {
    const user = userEvent.setup();
    render(
      <>
        <DatePicker aria-label="Expires" />
        <button type="button">Elsewhere</button>
      </>,
    );
    const trigger = screen.getByRole("button", { description: "Expires" });
    isClosed(trigger);

    await user.click(trigger);
    isOpen(trigger);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    isClosed(trigger);

    await user.click(trigger);
    isOpen(trigger);
    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    isClosed(trigger);

    await user.click(trigger);
    isOpen(trigger);
    await user.click(trigger);
    isClosed(trigger);

    await user.click(trigger);
    isOpen(trigger);
    await user.tab();
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
    isClosed(trigger);

    await user.click(trigger);
    isOpen(trigger);
    await arrive(gates.date);
    expect(screen.getByRole("dialog", { name: "Choose a date" })).toBeInTheDocument();
    isOpen(trigger);
    expect(screen.getByRole("textbox", { name: "Date" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    isClosed(trigger);
  });

  it("stays closed when the popover arrives after a cancel, and opens at once afterwards", async () => {
    const user = userEvent.setup();
    render(<DateRangePicker aria-label="Period" />);
    const trigger = screen.getByRole("button", { description: "Period" });
    await user.click(trigger);
    isOpen(trigger);
    await user.keyboard("{Escape}");
    isClosed(trigger);
    await arrive(gates.range);
    isClosed(trigger);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await user.click(trigger);
    isOpen(trigger);
    expect(screen.getByRole("dialog", { name: "Choose a date range" })).toBeInTheDocument();
    expect(screen.getAllByRole("grid")).toHaveLength(2);
  });
});
