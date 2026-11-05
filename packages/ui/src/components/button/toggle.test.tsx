import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./button.js";

describe("a toggle button", () => {
  it("is pressed or not, and reports the new state", async () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <Button variant="selector" size="sm" pressed={false} onPressedChange={onChange}>
        Agents
      </Button>,
    );
    const b = screen.getByRole("button", { name: "Agents" });
    expect(b).toHaveAttribute("aria-pressed", "false");
    expect(b.querySelector("svg")).toBeNull();
    await userEvent.click(b);
    expect(onChange).toHaveBeenLastCalledWith(true);
    rerender(
      <Button variant="selector" size="sm" pressed onPressedChange={onChange}>
        Agents
      </Button>,
    );
    expect(b).toHaveAttribute("aria-pressed", "true");
    expect(b.querySelector("svg")).not.toBeNull();
    await userEvent.click(b);
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it("does nothing while disabled, and leaves a plain button without aria-pressed", async () => {
    const onChange = vi.fn();
    render(
      <>
        <Button variant="selector" pressed={false} onPressedChange={onChange} disabled>
          Only overridden
        </Button>
        <Button>Save</Button>
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Only overridden" }));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Save" })).not.toHaveAttribute("aria-pressed");
  });

  it("lets its own click handler cancel the toggle", async () => {
    const onChange = vi.fn();
    render(
      <Button pressed={false} onPressedChange={onChange} onClick={(e) => e.preventDefault()}>
        Agents
      </Button>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Agents" }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
