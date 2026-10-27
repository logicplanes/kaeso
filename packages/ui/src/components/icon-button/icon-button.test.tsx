import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { IconButton } from "./icon-button.js";
import { icons } from "../../icons/vocabulary.js";

describe("IconButton", () => {
  it("keeps focus when it becomes disabled with a reason while focused", () => {
    const { rerender } = render(<IconButton icon={icons.add} aria-label="Add column" />);
    const add = screen.getByRole("button", { name: "Add column" });
    add.focus();
    rerender(<IconButton icon={icons.add} aria-label="Add column" disabled disabledReason="A table holds at most 12 columns" />);
    expect(document.activeElement).toBe(add);
    expect(add).toHaveAttribute("aria-description", "A table holds at most 12 columns");
  });
});
