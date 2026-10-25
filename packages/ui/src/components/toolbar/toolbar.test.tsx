import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Toolbar } from "./toolbar.js";

describe("Toolbar.Spacer", () => {
  it("forwards its ref to the space it fills", () => {
    const ref = createRef<HTMLSpanElement>();
    render(
      <Toolbar aria-label="Filters">
        <button type="button">Status</button>
        <Toolbar.Spacer ref={ref} />
        <button type="button">Export CSV</button>
      </Toolbar>,
    );
    expect(screen.getByRole("toolbar", { name: "Filters" })).toContainElement(ref.current);
    expect(ref.current).toHaveClass("flex-1");
    expect(ref.current).toHaveAttribute("aria-hidden");
  });
});
