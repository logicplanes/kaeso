import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Empty } from "./empty.js";

describe("Empty", () => {
  it("titles an empty state with a heading at every size", () => {
    const { rerender } = render(<Empty title="No keys yet" description="Create a key to send requests." />);
    expect(screen.getByRole("heading", { level: 2, name: "No keys yet" })).toBeInTheDocument();
    rerender(<Empty size="md" title="No keys yet" />);
    expect(screen.getByRole("heading", { level: 2, name: "No keys yet" })).toBeInTheDocument();
    rerender(<Empty size="compact" title="No rows" />);
    expect(screen.getByRole("heading", { level: 2, name: "No rows" })).toBeInTheDocument();
  });

  it("hangs a page's and a section's empty state from the edge, and centers a row's", () => {
    const { container, rerender } = render(<Empty title="No keys yet" />);
    expect(container.firstElementChild).toHaveClass("items-start", "text-start");
    expect(screen.getByRole("heading")).toHaveClass("text-title-lg");
    rerender(<Empty size="md" title="No members" />);
    expect(container.firstElementChild).toHaveClass("items-start", "text-start");
    rerender(<Empty size="compact" title="No rows" />);
    expect(container.firstElementChild).toHaveClass("items-center", "text-center");
  });
});
