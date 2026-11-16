import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Separator } from "./separator.js";

describe("Separator", () => {
  it("keeps a label readable to screen readers, beside a separator", () => {
    render(<Separator label="or continue with" />);
    expect(screen.getByText("or continue with")).toHaveClass("text-fg-secondary");
    expect(screen.getAllByRole("separator")).toHaveLength(1);
    expect(screen.getByRole("separator").textContent).toBe("");
  });
  it("draws its line in forced colours", () => {
    render(<Separator orientation="vertical" />);
    expect(screen.getByRole("separator")).toHaveClass("forced-line", "w-1");
  });
  it("draws a strong rule, and a quotation's ink rule 2px wide when vertical", () => {
    const { rerender } = render(<Separator tone="strong" />);
    expect(screen.getByRole("separator")).toHaveClass("bg-line-strong", "h-1");
    rerender(<Separator tone="ink" orientation="vertical" />);
    const rule = screen.getByRole("separator");
    expect(rule).toHaveClass("bg-fg", "w-2");
    expect(rule).not.toHaveClass("w-1");
    rerender(<Separator tone="ink" />);
    expect(screen.getByRole("separator")).toHaveClass("bg-fg", "h-1", "w-full");
  });
});
