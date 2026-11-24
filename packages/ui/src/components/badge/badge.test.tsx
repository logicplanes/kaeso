import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Badge } from "./badge.js";

describe("Badge", () => {
  it("shows its words in one line and keeps them inside a narrow cell", () => {
    render(<Badge tone="success">Active</Badge>);
    expect(screen.getByText("Active")).toHaveClass("truncate", "max-w-full", "min-w-0", "rounded-code", "text-success-text");
  });
  it("carries its tone in the word itself, with no dot or mark beside it", () => {
    render(<Badge tone="danger">Revoked</Badge>);
    expect(screen.getByText("Revoked")).toHaveClass("bg-danger-tint", "text-danger-text");
    expect(screen.getByText("Revoked").className).not.toMatch(/before:/);
  });
  it("lets a cut label be read in full: it takes focus and names itself in a tooltip", () => {
    // Without an observer, cut-off text measures itself as it mounts (text/truncated.test.tsx covers the observer).
    vi.stubGlobal("ResizeObserver", undefined);
    const spy = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(200);
    render(<Badge>Platform administrator</Badge>);
    expect(screen.getByText("Platform administrator")).toHaveAttribute("tabindex", "0");
    spy.mockRestore();
    vi.unstubAllGlobals();
  });
  it("keeps an outline in forced colours, where its fill disappears", () => {
    render(<Badge>Owner</Badge>);
    expect(screen.getByText("Owner").closest("span.rounded-code")).toHaveClass("forced-colors:outline");
  });
});

describe("Badge figures", () => {
  it("sets a version or a hyphenated word as written, and a count in tabular figures", () => {
    render(
      <>
        <Badge>40.4.3-rc.1 available</Badge>
        <Badge>Pre-release</Badge>
        <Badge>{"+3 projects"}</Badge>
      </>,
    );
    expect(screen.getByText("40.4.3-rc.1 available")).not.toHaveClass("tabular");
    expect(screen.getByText("Pre-release")).not.toHaveClass("tabular");
    expect(screen.getByText("+3 projects")).toHaveClass("tabular");
  });
});
