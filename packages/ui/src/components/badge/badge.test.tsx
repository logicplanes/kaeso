import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Badge } from "./badge.js";

describe("Badge", () => {
  it("shows its words in one line and keeps them inside a narrow cell", () => {
    render(<Badge tone="success">Active</Badge>);
    expect(screen.getByText("Active")).toHaveClass("truncate", "max-w-full", "min-w-0", "rounded-pill", "text-success-text");
  });
  it("lets a cut label be read in full: it takes focus and names itself in a tooltip", () => {
    const spy = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(200);
    render(<Badge>Platform administrator</Badge>);
    expect(screen.getByText("Platform administrator")).toHaveAttribute("tabindex", "0");
    spy.mockRestore();
  });
  it("keeps an outline in forced colours, where its fill disappears", () => {
    render(<Badge>Owner</Badge>);
    expect(screen.getByText("Owner").closest("span.rounded-pill")).toHaveClass("forced-colors:outline");
  });
});
