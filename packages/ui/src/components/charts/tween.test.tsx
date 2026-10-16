import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { tweens, useTweened, useTweenedScale } from "./core.js";

afterEach(() => vi.restoreAllMocks());

describe("tweens", () => {
  it("tweens data into other data of the same shape", () => {
    expect(tweens([1, 2, 3], [2, 3, 4])).toBe(true);
  });
  it("shows data arriving after a load at once, never grown from a baseline", () => {
    expect(tweens([0, 0, 0], [5, 9, 2])).toBe(false);
    expect(tweens([5, 9, 2], [0, 0, 0])).toBe(false);
    expect(tweens([1, 2], [1, 2, 3])).toBe(false);
    expect(tweens([1, 2], [1, 2])).toBe(false);
  });
});

describe("useTweened", () => {
  it("draws values that arrive after a load in the same render, at their size", () => {
    const { result, rerender } = renderHook(({ v }) => useTweened(v), { initialProps: { v: [0, 0, 0] } });
    rerender({ v: [10, 20, 30] });
    expect(result.current).toEqual([10, 20, 30]);
  });

  it("never draws past either end, whatever a frame's timestamp says", () => {
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => frames.push(cb));
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
    let now = 1000;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    const { result, rerender } = renderHook(({ v }) => useTweened(v), { initialProps: { v: [10, 20] } });
    rerender({ v: [20, 40] });
    expect(result.current).toEqual([10, 20]);
    act(() => frames.shift()?.(now - 400));
    expect(result.current).toEqual([10, 20]);
    now += 120;
    act(() => frames.shift()?.(now));
    for (const [i, v] of result.current.entries()) {
      expect(v).toBeGreaterThan([10, 20][i]!);
      expect(v).toBeLessThan([20, 40][i]!);
    }
    now += 400;
    act(() => frames.shift()?.(now));
    expect(result.current).toEqual([20, 40]);
  });
});

describe("data through a load", () => {
  it("never tweens from what was drawn before a load, even when the old data stayed on screen", () => {
    const { result, rerender } = renderHook(({ v, still }) => useTweened(v, still), { initialProps: { v: [10, 20], still: false } });
    rerender({ v: [10, 20], still: true });
    rerender({ v: [30, 5], still: false });
    expect(result.current).toEqual([30, 5]);
  });

  it("tweens a change of the same data after the load", () => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation(() => 1);
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
    const { result, rerender } = renderHook(({ v, still }) => useTweened(v, still), { initialProps: { v: [10, 20], still: true } });
    rerender({ v: [10, 20], still: false });
    rerender({ v: [12, 24], still: false });
    expect(result.current).toEqual([10, 20]);
  });
});

describe("useTweenedScale", () => {
  it("tweens the domain's top with the marks, so a rescale glides", () => {
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => frames.push(cb));
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
    let now = 1000;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    const { result, rerender } = renderHook(({ v, top }) => useTweenedScale(v, top), { initialProps: { v: [50, 100], top: 120 } });
    rerender({ v: [50, 200], top: 250 });
    expect(result.current).toEqual({ values: [50, 100], top: 120 });
    now += 120;
    act(() => frames.shift()?.(now));
    expect(result.current.top).toBeGreaterThan(120);
    expect(result.current.top).toBeLessThan(250);
    expect(50 / result.current.top).toBeGreaterThan(50 / 250);
    now += 400;
    act(() => frames.shift()?.(now));
    expect(result.current).toEqual({ values: [50, 200], top: 250 });
  });

  it("draws data arriving after nothing at once, with its own top", () => {
    const { result, rerender } = renderHook(({ v, top }) => useTweenedScale(v, top), { initialProps: { v: [0, 0], top: 1 } });
    rerender({ v: [5, 9], top: 10 });
    expect(result.current).toEqual({ values: [5, 9], top: 10 });
  });
});
