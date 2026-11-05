import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { createRef } from "react";
import { FlowWorkspace } from "./flow-workspace.js";

describe("FlowWorkspace", () => {
  it("puts the canvas beside a named inspector whose panel has a heading, a description and actions", () => {
    const ref = createRef<HTMLDivElement>();
    const panelRef = createRef<HTMLElement>();
    render(
      <FlowWorkspace
        ref={ref}
        panelLabel="Step settings"
        panel={
          <FlowWorkspace.Panel
            ref={panelRef}
            title="Model step"
            description="Requests go to this model first."
            actions={<button type="button">Remove step</button>}
          >
            <label>
              Model <input />
            </label>
          </FlowWorkspace.Panel>
        }
      >
        <div data-testid="canvas" />
      </FlowWorkspace>,
    );
    expect(ref.current).toHaveAttribute("data-flow-workspace");
    expect(screen.getByTestId("canvas").parentElement).toHaveAttribute("data-flow-workspace-canvas");
    expect(screen.getByTestId("canvas").parentElement).toHaveClass("flow-stacked-height", "lg:h-auto");
    const aside = screen.getByRole("complementary", { name: "Step settings" });
    expect(aside).toHaveClass("lg:w-400", "lg:divider-l", "max-lg:divider-t");
    const section = screen.getByRole("region", { name: "Model step" });
    expect(section).toBe(panelRef.current);
    expect(screen.getByRole("heading", { level: 2, name: "Model step" })).toBeInTheDocument();
    expect(screen.getByText("Requests go to this model first.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove step" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Model" })).toBeInTheDocument();
  });

  it("is the canvas alone without a panel, and widens the inspector on request", () => {
    const { rerender, container } = render(
      <FlowWorkspace>
        <div />
      </FlowWorkspace>,
    );
    expect(container.querySelector("aside")).toBeNull();
    rerender(
      <FlowWorkspace width="wide" panel={<FlowWorkspace.Panel title="Alias" />}>
        <div />
      </FlowWorkspace>,
    );
    expect(container.querySelector("aside")).toHaveClass("lg:w-480");
    expect(container.querySelector("aside")).not.toHaveAttribute("aria-label");
    expect(screen.getByRole("heading", { name: "Alias" }).closest("section")!.querySelectorAll("p")).toHaveLength(0);
  });
});
