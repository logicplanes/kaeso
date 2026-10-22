import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DatePicker, DateRangePicker } from "./date-picker.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const gates = vi.hoisted(() => {
  const gate = () => {
    let open!: () => void;
    const promise = new Promise<void>((resolve) => (open = resolve));
    return { promise, open, asked: false };
  };
  return { date: gate(), range: gate() };
});
vi.mock("./date-popup.js", async (importOriginal) => {
  gates.date.asked = true;
  await gates.date.promise;
  return importOriginal();
});
vi.mock("./range-popup.js", async (importOriginal) => {
  gates.range.asked = true;
  await gates.range.promise;
  return importOriginal();
});

await Promise.all([vi.importActual("./date-popup.js"), vi.importActual("./range-popup.js")]);

const idle: (() => void)[] = [];
const cancelled = vi.fn();
vi.stubGlobal("requestIdleCallback", (run: () => void) => idle.push(run));
vi.stubGlobal("cancelIdleCallback", cancelled);
afterEach(() => {
  idle.length = 0;
  cancelled.mockClear();
});

describe("date pickers on a slow machine", () => {
  it("stop waiting for idle time when they leave the page", () => {
    const { unmount } = render(
      <>
        <DatePicker aria-label="Expires" />
        <DateRangePicker aria-label="Period" />
      </>,
    );
    expect(idle).toHaveLength(2);
    unmount();
    expect(cancelled).toHaveBeenCalledTimes(2);
  });

  it("load their popovers once the page is idle, without a press, and then open on the press itself", async () => {
    const user = userEvent.setup();
    render(
      <>
        <DatePicker aria-label="Expires" />
        <DateRangePicker aria-label="Period" />
      </>,
    );
    await act(() => lazyModulesSettled());
    expect(gates.date.asked).toBe(false);
    expect(gates.range.asked).toBe(false);
    expect(idle).toHaveLength(2);

    await act(async () => idle.forEach((run) => run()));
    await vi.waitFor(() => expect(gates.date.asked && gates.range.asked).toBe(true));
    gates.date.open();
    gates.range.open();
    await act(() => lazyModulesSettled());

    const date = screen.getByRole("button", { description: "Expires" });
    await user.click(date);
    expect(screen.getByRole("dialog", { name: "Choose a date" })).toBeInTheDocument();
    expect(date).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");

    const range = screen.getByRole("button", { description: "Period" });
    await user.click(range);
    expect(screen.getByRole("dialog", { name: "Choose a date range" })).toBeInTheDocument();
    expect(range).toHaveAttribute("aria-expanded", "true");
  });
});
