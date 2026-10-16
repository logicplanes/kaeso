import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createRef } from "react";
import { Flow } from "./flow";
import { icons } from "../../icons/vocabulary";

const boxes: Record<string, { left: number; top: number; width: number; height: number }> = {
  app: { left: 0, top: 40, width: 80, height: 36 },
  kaeso: { left: 160, top: 40, width: 80, height: 36 },
  openai: { left: 320, top: 0, width: 80, height: 36 },
  anthropic: { left: 320, top: 80, width: 80, height: 36 },
};

function rect(b: { left: number; top: number; width: number; height: number }) {
  return { ...b, x: b.left, y: b.top, right: b.left + b.width, bottom: b.top + b.height, toJSON: () => b } as DOMRect;
}

afterEach(() => vi.restoreAllMocks());

describe("Flow", () => {
  it("draws a straight edge between nodes on one line and a rounded elbow between lines", () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const id = this.getAttribute("data-flow-node");
      return rect(id ? boxes[id]! : { left: 0, top: 0, width: 400, height: 116 });
    });
    const { container } = render(
      <Flow
        animated
        node="box"
        aria-label="Route"
        columns={[
          [{ id: "app", label: "App", icon: icons.code }],
          [{ id: "kaeso", label: "Kaeso", emphasis: true, description: "Gateway" }],
          [
            { id: "openai", label: "OpenAI", leading: <span>o</span> },
            { id: "anthropic", label: "Anthropic" },
          ],
        ]}
        edges={[
          ["app", "kaeso"],
          ["kaeso", "openai"],
          ["kaeso", "anthropic"],
          ["kaeso", "missing"],
        ]}
      />,
    );
    expect(screen.getByRole("img", { name: "Route" })).toBeInTheDocument();
    const d = [...container.querySelectorAll("svg.absolute path")].map((p) => p.getAttribute("d"));
    expect(d).toEqual(["M80 58.5H160", expect.stringMatching(/^M240 58\.5H\d+Q/), expect.stringMatching(/^M240 58\.5H\d+Q/)]);
    expect(container.querySelectorAll("animateMotion")).toHaveLength(3);
    expect(screen.getByText("Gateway")).toBeInTheDocument();
    expect(container.querySelector("svg.absolute")).toHaveAttribute("width", "400");
  });

  it("has a default name, hides descriptions on pills, and forwards its ref", () => {
    const ref = createRef<HTMLDivElement>();
    const fnRef = vi.fn();
    const columns = [[{ id: "a", label: "A", description: "Hidden on a pill" }]];
    const { rerender } = render(<Flow ref={ref} columns={columns} edges={[]} />);
    expect(screen.getByRole("img", { name: "Flow diagram" })).toBe(ref.current);
    expect(screen.queryByText("Hidden on a pill")).toBeNull();
    rerender(<Flow ref={fnRef} columns={columns} edges={[]} />);
    expect(fnRef).toHaveBeenCalled();
  });
});
