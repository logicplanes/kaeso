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
});
