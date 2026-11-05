import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { icons } from "../../icons/vocabulary.js";
import { Status } from "./status.js";

describe("Status", () => {
  it("puts a mark before the word when given one", () => {
    const { container } = render(
      <Status tone="success" icon={icons.success}>
        Up to date
      </Status>,
    );
    expect(screen.getByText("Up to date")).toHaveClass("text-success-text");
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("is the word alone without one", () => {
    const { container } = render(<Status size="sm">Archived</Status>);
    expect(screen.getByText("Archived")).toHaveClass("text-small");
    expect(container.querySelector("svg")).toBeNull();
  });
});
