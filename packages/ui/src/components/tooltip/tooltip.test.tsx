import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { TooltipSurface } from "./tooltip.js";

describe("TooltipSurface", () => {
  it("forwards its ref to the tooltip", () => {
    const ref = createRef<HTMLDivElement>();
    render(
      <TooltipSurface ref={ref} shortcut={["["]}>
        Collapse sidebar
      </TooltipSurface>,
    );
    expect(ref.current).toBe(screen.getByRole("tooltip"));
    expect(ref.current).toHaveTextContent("Collapse sidebar");
  });
});
