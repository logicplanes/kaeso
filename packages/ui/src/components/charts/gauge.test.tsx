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
    const { container, rerender } = render(<Gauge aria-label="Budget" value={0.6} thresholds={[0.9, 0.5]} />);
    expect(arc(container)).toBe("var(--k-warning)");
    rerender(<Gauge aria-label="Budget" value={1.3} valueLabel="$650.00" />);
    expect(arc(container)).toBe("var(--k-danger)");
    expect(screen.getByRole("meter")).toHaveTextContent("$650.00");
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuemax", "130");
  });

  it("draws no arc at zero", () => {
    const { container } = render(<Gauge aria-label="Budget" value={0} />);
    expect(container.querySelectorAll("path")).toHaveLength(1);
  });
});
