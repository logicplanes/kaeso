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
});
