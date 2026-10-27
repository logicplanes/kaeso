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
});
