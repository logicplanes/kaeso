import { afterEach, describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { listenForSearchKey, openSearch, setSearchOpen, setSearchQuery, useSearchOpen, useSearchQuery } from "./search-store";

const press = (key: string, init: KeyboardEventInit = {}, target: EventTarget = window) =>
  target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, ...init }));

let stop: () => void = () => undefined;
afterEach(() => {
  stop();
  act(() => {
    setSearchOpen(false);
    setSearchQuery("");
  });
  document.body.replaceChildren();
});

function state() {
  return renderHook(() => ({ open: useSearchOpen(), query: useSearchQuery() }));
}

describe("the search shortcut", () => {
  it("opens and closes with ⌘K or Ctrl+K, and shows what is typed before the field has focus", () => {
    stop = listenForSearchKey();
    const { result } = state();
    expect(result.current.open).toBe(false);
    act(() => press("k", { metaKey: true }));
    expect(result.current.open).toBe(true);
    act(() => {
      for (const key of ["r", "a", "x", "Backspace", "Shift", "t", "e"]) press(key);
    });
    expect(result.current.query).toBe("rate");
    act(() => press("K", { ctrlKey: true }));
    expect(result.current.open).toBe(false);
  });

  it("leaves what is typed into the search field to the field", () => {
    stop = listenForSearchKey();
    const { result } = state();
    const field = document.body.appendChild(document.createElement("input"));
    field.setAttribute("data-palette-input", "");
    act(() => press("k", { metaKey: true }));
    act(() => press("a"));
    act(() => press("l", {}, field));
    act(() => press("Backspace", {}, field));
    act(() => press("i"));
    expect(result.current.query).toBe("ai");
  });

  it("starts empty every time it opens", () => {
    stop = listenForSearchKey();
    const { result } = state();
    act(() => press("k", { metaKey: true }));
    act(() => {
      for (const key of ["o", "l", "d"]) press(key);
    });
    act(() => press("Escape"));
    expect(result.current.open).toBe(false);
    expect(result.current.query).toBe("old");
    act(() => press("k", { metaKey: true }));
    expect(result.current.query).toBe("");
    act(() => setSearchQuery("kept"));
    expect(result.current.query).toBe("kept");
  });

  it("ignores other shortcuts, and keys while it is closed", () => {
    stop = listenForSearchKey();
    const { result } = state();
    act(() => press("k", { metaKey: true, shiftKey: true }));
    expect(result.current.open).toBe(false);
    act(() => press("x"));
    expect(result.current.query).toBe("");
    act(() => openSearch());
    act(() => {
      press("y", { altKey: true });
      press("y", { ctrlKey: true });
      press("y", { metaKey: true });
    });
    expect(result.current.open).toBe(true);
    expect(result.current.query).toBe("");
  });

  it("stops listening when asked", () => {
    listenForSearchKey()();
    const { result } = state();
    act(() => press("k", { metaKey: true }));
    expect(result.current.open).toBe(false);
  });
});
