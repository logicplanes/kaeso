import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Meter, Progress, meterColor } from "./meter.js";
import { neutral, semantic, viz } from "@logicplanes/kaeso-tokens";

const lum = (hex: string) => {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
};
const ratio = (a: string, b: string) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x! + 0.05) / (y! + 0.05);
};

describe("Meter", () => {
  it("turns amber at the first threshold and red at the limit, and says so in its value and to screen readers", () => {
    const { rerender } = render(<Meter label="Monthly budget" value={0.5} valueLabel="$25.00 of $50.00" thresholds={[0.8]} caption="Resets in 12 days" />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "$25.00 of $50.00");
    expect(screen.getByText("$25.00 of $50.00")).toHaveClass("text-fg-secondary");
    rerender(<Meter label="Monthly budget" value={0.86} valueLabel="$43.00 of $50.00" thresholds={[0.8]} caption="Resets in 12 days" />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "$43.00 of $50.00, near the limit");
    expect(screen.getByText("$43.00 of $50.00")).toHaveClass("text-warning-text");
    rerender(<Meter label="Monthly budget" value={1.1} valueLabel="$55.00 of $50.00" thresholds={[0.8]} caption="Over the limit: requests are refused" />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "$55.00 of $50.00, over the limit");
    expect(screen.getByText("Over the limit: requests are refused")).toHaveClass("text-danger-text");
  });
  it("cuts each threshold out of the bar exactly at its value, so the tick shows the surface below", () => {
    render(<Meter label="Requests" value={0.4} thresholds={[0.8, 0.5]} />);
    const mask = screen.getByRole("meter").style.maskImage;
    expect(mask).toContain("calc(50% - 1px)");
    expect(mask).toContain("calc(80% + 1px)");
    expect(mask.indexOf("50%")).toBeLessThan(mask.indexOf("80%"));
  });
  it("keeps its fill in forced colours, where an outline draws the track", () => {
    render(<Meter label="Requests" value={0.4} />);
    expect(screen.getByRole("meter")).toHaveClass("forced-colors:outline");
    expect(screen.getByRole("meter").firstElementChild).toHaveClass("data-mark");
  });
  it("ends its bar at 3:1 or more against the track in both themes, in every state", () => {
    const track = neutral["fill-2"];
    const fills = { "bg-viz-1": viz[0]!, "bg-viz-2": viz[1]!, "bg-danger": semantic.danger.solid } as const;
    for (const value of [0.4, 0.9, 1.2]) {
      const fill = fills[meterColor(value, [0.8]) as keyof typeof fills];
      expect(ratio(fill.light, track.light)).toBeGreaterThanOrEqual(3);
      expect(ratio(fill.dark, track.dark)).toBeGreaterThanOrEqual(3);
    }
  });
});

describe("Progress", () => {
  it("is an indeterminate line without a value and a pill bar with one", () => {
    const { rerender } = render(<Progress aria-label="Uploading" />);
    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    expect(screen.getByRole("progressbar").firstElementChild).toHaveClass("animate-indeterminate");
    rerender(<Progress aria-label="Uploading" value={0.4} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "40");
  });
});
