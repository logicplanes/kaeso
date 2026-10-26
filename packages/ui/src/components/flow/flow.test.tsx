import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import { createRef } from "react";
import { Flow } from "./flow.js";
import { icons } from "../../icons/vocabulary.js";

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
    const figure = screen.getByRole("figure", { name: "Route" });
    expect(
      within(figure)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["App to Kaeso", "Kaeso to OpenAI", "Kaeso to Anthropic"]);
    const d = [...container.querySelectorAll("svg.absolute path")].map((p) => p.getAttribute("d"));
    expect(d).toEqual(["M80 58.5H160", expect.stringMatching(/^M240 58\.5H\d+Q/), expect.stringMatching(/^M240 58\.5H\d+Q/)]);
    expect(container.querySelectorAll("[data-flow-dot]")).toHaveLength(0);
    expect(container.querySelector("svg.absolute path")).toHaveAttribute("stroke", "var(--k-text-tertiary)");
    expect(container.querySelector('[data-flow-node="kaeso"]')).toHaveClass("ring-step", "bg-panel");
    expect(container.querySelector('[data-flow-node="app"]')).toHaveClass("shadow-ring", "bg-panel");
    expect(figure).not.toHaveClass("-m-4");
    expect(screen.getByText("Gateway")).toBeInTheDocument();
    expect(container.querySelector("svg.absolute")).toHaveAttribute("width", "400");
  });

  it("has a default name, hides descriptions on pills, and forwards its ref", () => {
    const ref = createRef<HTMLDivElement>();
    const fnRef = vi.fn();
    const columns = [[{ id: "a", label: "A", description: "Hidden on a pill" }]];
    const { rerender } = render(<Flow ref={ref} columns={columns} edges={[]} />);
    expect(screen.getByRole("figure", { name: "Flow diagram" })).toBe(ref.current);
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.queryByText("Hidden on a pill")).toBeNull();
    rerender(<Flow ref={fnRef} columns={columns} edges={[]} />);
    expect(fnRef).toHaveBeenCalled();
  });

  describe("a changed route", () => {
    const columns = [
      [{ id: "app", label: "App" }],
      [{ id: "kaeso", label: "Kaeso" }],
      [
        { id: "openai", label: "OpenAI" },
        { id: "anthropic", label: "Anthropic" },
      ],
    ];
    const route = (to: string): [string, string][] => [
      ["app", "kaeso"],
      ["kaeso", to],
    ];
    const reduce = (on: boolean) =>
      vi.spyOn(window, "matchMedia").mockImplementation((q: string) => ({ matches: on && q.includes("reduce") }) as MediaQueryList);
    const geometry = () => {
      const proto = SVGElement.prototype as unknown as { getTotalLength?: () => number; getPointAtLength?: (n: number) => { x: number; y: number } };
      proto.getTotalLength = () => 100;
      proto.getPointAtLength = (n: number) => ({ x: n, y: 10 });
    };
    it("sends one dot along each connector once, never on first render", () => {
      geometry();
      reduce(false);
      vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
      const { container, rerender } = render(<Flow animated columns={columns} edges={route("openai")} />);
      act(() => void vi.advanceTimersByTime(200));
      expect(container.querySelectorAll("[data-flow-dot]")).toHaveLength(0);
      rerender(<Flow animated columns={columns} edges={route("anthropic")} />);
      act(() => void vi.advanceTimersByTime(400));
      expect(container.querySelectorAll("[data-flow-dot]").length).toBeGreaterThan(0);
      act(() => void vi.advanceTimersByTime(2000));
      expect(container.querySelectorAll("[data-flow-dot]")).toHaveLength(0);
      vi.useRealTimers();
    });
    it("moves nothing with reduced motion", () => {
      geometry();
      reduce(true);
      vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
      const { container, rerender } = render(<Flow animated columns={columns} edges={route("openai")} />);
      rerender(<Flow animated columns={columns} edges={route("anthropic")} />);
      act(() => void vi.advanceTimersByTime(400));
      expect(container.querySelectorAll("[data-flow-dot]")).toHaveLength(0);
      vi.useRealTimers();
    });
  });
});
