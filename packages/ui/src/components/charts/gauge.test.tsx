import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Gauge } from "./gauge.js";

const arc = (container: HTMLElement) => container.querySelectorAll("path")[1]?.getAttribute("stroke");

describe("Gauge", () => {
  it("is a meter with the share as a percentage and a caption", () => {
    const { container } = render(<Gauge aria-label="Budget" value={0.42} label="of $500.00 budget" />);
    const meter = screen.getByRole("meter", { name: "Budget" });
    expect(meter).toHaveAttribute("aria-valuenow", "42");
    expect(meter).toHaveTextContent("42%of $500.00 budget");
    expect(arc(container)).toBe("var(--k-viz-1)");
    expect(container.querySelectorAll("line")).toHaveLength(1);
  });

  it("turns to warning from the lowest threshold and to danger when full", () => {
    const { container, rerender } = render(<Gauge aria-label="Budget" value={0.6} thresholds={[0.9, 0.5]} label="of $500.00 budget" />);
    expect(arc(container)).toBe("var(--k-warning)");
    expect(screen.getByText("Near the limit")).toHaveClass("text-warning-text");
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "60%, of $500.00 budget, Near the limit");
    rerender(<Gauge aria-label="Budget" value={1.3} valueLabel="$650.00" />);
    expect(arc(container)).toBe("var(--k-danger)");
    expect(screen.getByRole("meter")).toHaveTextContent("$650.00");
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuemax", "100");
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuenow", "100");
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "$650.00, Over the limit");
    expect(screen.getByRole("meter")).toHaveTextContent("Over the limit");
  });

  it("shows a dash with no arc or status when the value is missing, and says there is no data", () => {
    const { container, rerender } = render(<Gauge aria-label="Budget" value={Number.NaN} label="of $500.00 budget" />);
    const meter = screen.getByRole("meter", { name: "Budget" });
    expect(meter).toHaveTextContent(/^–of \$500\.00 budget$/);
    expect(meter).toHaveAttribute("aria-valuenow", "0");
    expect(meter).toHaveAttribute("aria-valuetext", "No data, of $500.00 budget");
    expect(container.querySelectorAll("path")).toHaveLength(1);
    rerender(<Gauge aria-label="Budget" value={Number.POSITIVE_INFINITY} valueLabel="$0.00" />);
    expect(meter).toHaveTextContent(/^\$0\.00$/);
    expect(meter).toHaveAttribute("aria-valuetext", "$0.00");
    expect(container.querySelectorAll("path")).toHaveLength(1);
  });

  it("draws no arc at zero", () => {
    const { container } = render(<Gauge aria-label="Budget" value={0} />);
    expect(container.querySelectorAll("path")).toHaveLength(1);
  });
});
