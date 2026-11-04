import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HISTORY_LIMIT, useHistory } from "./history";

afterEach(() => vi.useRealTimers());

describe("useHistory", () => {
  it("undoes and redoes each change, and a new change drops what was undone", () => {
    const { result } = renderHook(() => useHistory(() => 0));
    expect(result.current).toMatchObject({ present: 0, canUndo: false, canRedo: false });
    act(() => result.current.commit((n) => n + 1));
    act(() => result.current.commit((n) => n + 1));
    expect(result.current).toMatchObject({ present: 2, canUndo: true });
    act(() => result.current.undo());
    expect(result.current).toMatchObject({ present: 1, canRedo: true });
    act(() => result.current.redo());
    expect(result.current.present).toBe(2);
    act(() => result.current.undo());
    act(() => result.current.commit((n) => n + 10));
    expect(result.current).toMatchObject({ present: 11, canRedo: false });
    act(() => result.current.redo());
    expect(result.current.present).toBe(11);
  });

  it("keeps a change that changes nothing out of the history", () => {
    const { result } = renderHook(() => useHistory(() => "a"));
    act(() => result.current.commit((s) => s));
    expect(result.current.canUndo).toBe(false);
    act(() => result.current.undo());
    expect(result.current.present).toBe("a");
  });

  it("merges typing in one field within a second into one step", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useHistory(() => ""));
    act(() => result.current.commit(() => "f", "name"));
    act(() => result.current.commit(() => "fa", "name"));
    act(() => result.current.commit(() => "fas", "name"));
    act(() => vi.advanceTimersByTime(1500));
    act(() => result.current.commit(() => "fast", "name"));
    act(() => result.current.commit(() => "fast!", "other"));
    act(() => result.current.undo());
    expect(result.current.present).toBe("fast");
    act(() => result.current.undo());
    expect(result.current.present).toBe("fas");
    act(() => result.current.undo());
    expect(result.current.present).toBe("");
  });

  it("keeps the last hundred steps", () => {
    const { result } = renderHook(() => useHistory(() => 0));
    for (let i = 0; i < HISTORY_LIMIT + 20; i++) act(() => result.current.commit((n) => n + 1));
    for (let i = 0; i < HISTORY_LIMIT + 20; i++) act(() => result.current.undo());
    expect(result.current).toMatchObject({ present: 20, canUndo: false, canRedo: true });
  });
});
