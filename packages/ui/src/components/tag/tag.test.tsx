import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Tag } from "./tag.js";

describe("Tag", () => {
  it("keeps a pressable tag and its remove circle as two separate buttons", async () => {
    const onClick = vi.fn();
    const onRemove = vi.fn();
    const { container } = render(
      <Tag onClick={onClick} pressed onRemove={onRemove} data-force="focus">
        production
      </Tag>,
    );
    const press = screen.getByRole("button", { name: "production" });
    const remove = screen.getByRole("button", { name: "Remove production" });
    expect(press).toHaveAttribute("aria-pressed", "true");
    expect(press).toHaveAttribute("data-force", "focus");
    expect(within(press).queryByRole("button")).toBeNull();
    expect(container.querySelectorAll("button button")).toHaveLength(0);
    await userEvent.click(remove);
    expect(onRemove).toHaveBeenCalledTimes(1);
    expect(onClick).not.toHaveBeenCalled();
    await userEvent.click(press);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("is one button when it only presses, and plain text with a remove circle otherwise", async () => {
    const onClick = vi.fn();
    const { rerender } = render(<Tag onClick={onClick}>staging</Tag>);
    await userEvent.click(screen.getByRole("button", { name: "staging" }));
    expect(onClick).toHaveBeenCalledTimes(1);
    const onRemove = vi.fn();
    rerender(<Tag onRemove={onRemove}>staging</Tag>);
    expect(screen.getAllByRole("button")).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: "Remove staging" }));
    expect(onRemove).toHaveBeenCalledTimes(1);
  });
});
