import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { listenForSearchKey, openSearch, setSearchOpen, takeTypedQuery, useSearchOpen } from "./search-store";

const press = (key: string, init: KeyboardEventInit = {}) => window.dispatchEvent(new KeyboardEvent("keydown", { key, ...init }));

let stop: () => void = () => undefined;
afterEach(() => {
  stop();
  act(() => setSearchOpen(false));
});

describe("the search shortcut", () => {
  it("opens and closes with ⌘K or Ctrl+K, and keeps what is typed until the search is there", () => {
    stop = listenForSearchKey();
    const { result } = renderHook(() => useSearchOpen());
    expect(result.current).toBe(false);
    act(() => press("k", { metaKey: true }));
    expect(result.current).toBe(true);
    act(() => press("Escape"));
    expect(result.current).toBe(false);
    act(() => press("k", { metaKey: true }));
    act(() => {
      for (const key of ["r", "a", "x", "Backspace", "Shift", "t", "e"]) press(key);
    });
    expect(takeTypedQuery()).toBe("rate");
    act(() => press("K", { ctrlKey: true }));
    expect(result.current).toBe(false);
  });

  it("forgets what was typed when the search closes before it is there", async () => {
    vi.resetModules();
    const fresh = await import("./search-store");
    const stopFresh = fresh.listenForSearchKey();
    act(() => press("k", { metaKey: true }));
    act(() => {
      for (const key of ["o", "l", "d"]) press(key);
    });
    act(() => press("Escape"));
    act(() => press("k", { metaKey: true }));
    act(() => press("n"));
    act(() => fresh.setSearchOpen(false));
    act(() => press("k", { metaKey: true }));
    act(() => press("x"));
    expect(fresh.takeTypedQuery()).toBe("x");
    act(() => fresh.setSearchOpen(false));
    stopFresh();
  });

  it("ignores other shortcuts, and leaves Escape to the search once it is there", () => {
    stop = listenForSearchKey();
    const { result } = renderHook(() => useSearchOpen());
    act(() => press("k", { metaKey: true, shiftKey: true }));
    expect(result.current).toBe(false);
    act(() => openSearch());
    expect(result.current).toBe(true);
    act(() => press("Escape"));
    expect(result.current).toBe(true);
  });

  it("stops listening when asked", () => {
    listenForSearchKey()();
    const { result } = renderHook(() => useSearchOpen());
    act(() => press("k", { metaKey: true }));
    expect(result.current).toBe(false);
  });
});
