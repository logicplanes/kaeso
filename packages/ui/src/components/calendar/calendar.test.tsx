import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Calendar } from "./calendar.js";

describe("Calendar", () => {
  it("names each day by its full date, with the number it shows as a word of it", () => {
    render(<Calendar defaultMonth={new Date(2025, 2, 1)} selected={new Date(2025, 2, 14)} />);
    expect(screen.getByRole("button", { name: "Saturday, March 1, 2025" })).toHaveTextContent(/^1$/);
    expect(screen.getByRole("button", { name: "Friday, March 14, 2025, selected" })).toHaveTextContent(/^14$/);
    expect(screen.getByRole("button", { name: "Go to the previous month" })).toBeInTheDocument();
  });

  it("forwards its ref to the root element in both modes", () => {
    const ref = createRef<HTMLDivElement>();
    const { rerender } = render(<Calendar ref={ref} defaultMonth={new Date(2025, 2, 1)} className="w-320" />);
    expect(ref.current).toHaveClass("k-cal", "w-320");
    expect(ref.current).toContainElement(screen.getByRole("grid", { name: "March 2025" }));
    rerender(<Calendar ref={ref} mode="range" defaultMonth={new Date(2025, 2, 1)} />);
    expect(ref.current).toHaveClass("k-cal");
    expect(ref.current).toContainElement(screen.getByRole("grid", { name: "March 2025" }));
  });
});
