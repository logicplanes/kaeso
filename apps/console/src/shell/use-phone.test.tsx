import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useMediaQuery, usePhone } from "./use-phone";

function Width() {
  return <output>{usePhone() ? "phone" : "wider"}</output>;
}

afterEach(() => vi.unstubAllGlobals());

describe("usePhone", () => {
  it("follows the phone media query as the window changes width", () => {
    const listeners = new Set<() => void>();
    const list = {
      matches: true,
      addEventListener: (_: string, fn: () => void) => listeners.add(fn),
      removeEventListener: (_: string, fn: () => void) => listeners.delete(fn),
    };
    const matchMedia = vi.fn(() => list);
    vi.stubGlobal("matchMedia", matchMedia);
    const { unmount } = render(<Width />);
    expect(screen.getByRole("status")).toHaveTextContent("phone");
    expect(matchMedia).toHaveBeenCalledWith("(max-width: 639px)");
    list.matches = false;
    act(() => listeners.forEach((fn) => fn()));
    expect(screen.getByRole("status")).toHaveTextContent("wider");
    unmount();
    expect(listeners.size).toBe(0);
  });
  it("answers the media query it's given", () => {
    vi.stubGlobal("matchMedia", (query: string) => ({ matches: query === "(max-width: 1279px)", addEventListener() {}, removeEventListener() {} }));
    function Narrow() {
      return <output>{useMediaQuery("(max-width: 1279px)") ? "narrow" : "wide"}</output>;
    }
    render(
      <>
        <Narrow />
        <Width />
      </>,
    );
    expect(screen.getAllByRole("status").map((s) => s.textContent)).toEqual(["narrow", "wider"]);
  });
  it("is not a phone where the browser can't answer media queries", () => {
    vi.stubGlobal("matchMedia", undefined);
    render(<Width />);
    expect(screen.getByRole("status")).toHaveTextContent("wider");
  });
});
