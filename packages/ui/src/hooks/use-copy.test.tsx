import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useCopy } from "./use-copy";

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("useCopy", () => {
  it("copies through the clipboard and resets after the duration", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn(() => Promise.resolve());
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    const { result } = renderHook(() => useCopy(500));
    await act(async () => expect(await result.current.copy("sk-kaeso-live")).toBe(true));
    expect(writeText).toHaveBeenCalledWith("sk-kaeso-live");
    expect(result.current.copied).toBe(true);
    act(() => vi.advanceTimersByTime(500));
    expect(result.current.copied).toBe(false);
    vi.unstubAllGlobals();
  });

  it("falls back to a hidden text area, and reports when that fails too", async () => {
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText: () => Promise.reject(new Error("denied")) } });
    const exec = vi.fn(() => true);
    document.execCommand = exec;
    const { result } = renderHook(() => useCopy());
    await act(async () => expect(await result.current.copy("one")).toBe(true));
    expect(exec).toHaveBeenCalledWith("copy");
    expect(document.querySelector("textarea")).toBeNull();
    exec.mockReturnValue(false);
    const { result: second } = renderHook(() => useCopy());
    await act(async () => expect(await second.current.copy("two")).toBe(false));
    expect(second.current.copied).toBe(false);
    vi.unstubAllGlobals();
  });
});
