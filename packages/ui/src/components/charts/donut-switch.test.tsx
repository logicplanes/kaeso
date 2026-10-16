import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render } from "@testing-library/react";
import { Donut } from "./donut.js";

describe("Donut", () => {
  afterEach(() => vi.useRealTimers());

  it("draws fewer, more or other slices after a change without throwing", async () => {
    const three = [
      { id: "a", label: "A", value: 5 },
      { id: "b", label: "B", value: 3 },
      { id: "c", label: "C", value: 2 },
    ];
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
    const { rerender } = render(<Donut aria-label="Cost" slices={three} />);
    for (const next of [
      [{ id: "x", label: "X", value: 4 }],
      [],
      three,
      [
        { id: "a", label: "A", value: 1 },
        { id: "d", label: "D", value: 9 },
      ],
    ]) {
      expect(() => rerender(<Donut aria-label="Cost" slices={next} />)).not.toThrow();
      await act(() => vi.advanceTimersByTimeAsync(30));
    }
  });

  it("survives a change in slice count in the middle of a tween", () => {
    const a = [
      { id: "a", label: "A", value: 5 },
      { id: "b", label: "B", value: 3 },
    ];
    const b = [
      { id: "a", label: "A", value: 1 },
      { id: "b", label: "B", value: 8 },
    ];
    const { rerender } = render(<Donut aria-label="Cost" slices={a} />);
    rerender(<Donut aria-label="Cost" slices={b} />);
    expect(() => rerender(<Donut aria-label="Cost" slices={[{ id: "z", label: "Z", value: 2 }]} />)).not.toThrow();
    expect(() => rerender(<Donut aria-label="Cost" slices={a} />)).not.toThrow();
  });
});

describe("Donut with other slices", () => {
  it("draws another set of as many slices at once, never tweening one's values into the other's", () => {
    const paths = () => [...document.querySelectorAll("path")].map((p) => p.getAttribute("d"));
    const a = [
      { id: "a", label: "A", value: 9 },
      { id: "b", label: "B", value: 1 },
    ];
    const b = [
      { id: "c", label: "C", value: 1 },
      { id: "d", label: "D", value: 9 },
    ];
    const alone = render(<Donut aria-label="Cost" slices={b} />);
    const fresh = paths();
    alone.unmount();
    const { rerender } = render(<Donut aria-label="Cost" slices={a} />);
    rerender(<Donut aria-label="Cost" slices={b} />);
    expect(paths()).toEqual(fresh);
  });
});
