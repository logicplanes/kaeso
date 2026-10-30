import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FilterToggle } from "./filter-toggle.js";

describe("FilterToggle", () => {
  it("is a toggle button that reports the new state", async () => {
    const onChange = vi.fn();
    const { rerender } = render(<FilterToggle label="Agents" pressed={false} onPressedChange={onChange} />);
    const b = screen.getByRole("button", { name: "Agents" });
    expect(b).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(b);
    expect(onChange).toHaveBeenLastCalledWith(true);
    rerender(<FilterToggle label="Agents" pressed onPressedChange={onChange} />);
    expect(b).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(b);
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it("does nothing while disabled", async () => {
    const onChange = vi.fn();
    render(<FilterToggle label="Only overridden" pressed={false} onPressedChange={onChange} disabled />);
    await userEvent.click(screen.getByRole("button", { name: "Only overridden" }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
