import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { icons } from "../../icons/vocabulary.js";
import { StatusDot } from "./status-dot.js";

describe("StatusDot", () => {
  it("puts a mark before the word when given one", () => {
    const { container } = render(
      <StatusDot tone="success" icon={icons.success}>
        Up to date
      </StatusDot>,
    );
    expect(screen.getByText("Up to date")).toHaveClass("text-success-text");
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("is the word alone without one", () => {
    const { container } = render(<StatusDot size="sm">Archived</StatusDot>);
    expect(screen.getByText("Archived")).toHaveClass("text-small");
    expect(container.querySelector("svg")).toBeNull();
  });
});
