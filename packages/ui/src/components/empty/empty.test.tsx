import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Empty } from "./empty.js";

describe("Empty", () => {
  it("titles an empty page with a heading, and an empty section or table with text, so the page outline stays whole", () => {
    const { rerender } = render(<Empty title="No keys yet" description="Create a key to send requests." />);
    expect(screen.getByRole("heading", { level: 2, name: "No keys yet" })).toBeInTheDocument();
    rerender(<Empty size="md" title="No keys yet" />);
    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.getByText("No keys yet").tagName).toBe("P");
    rerender(<Empty size="compact" title="No rows" />);
    expect(screen.queryByRole("heading")).toBeNull();
  });
});
