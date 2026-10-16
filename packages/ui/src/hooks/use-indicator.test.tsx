import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { useRef } from "react";
import { useIndicator } from "./use-indicator.js";

function Probe({ layout }: { layout: string }) {
  const track = useRef<HTMLDivElement | null>(null);
  const ind = useIndicator(track, "[data-checked]", layout);
  return (
    <div ref={track} data-testid="track">
      <span data-checked="">Day</span>
      <output>{ind.ready ? "ready" : "waiting"}</output>
    </div>
  );
}

describe("useIndicator", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("measures again once fonts load, but not after the layout it measured for is gone", async () => {
    let loaded: () => void = () => undefined;
    const ready = new Promise<void>((resolve) => (loaded = resolve));
    const fonts = Object.getOwnPropertyDescriptor(document, "fonts");
    Object.defineProperty(document, "fonts", { configurable: true, value: { ready } });
    try {
      const { rerender, unmount } = render(<Probe layout="day" />);
      expect(screen.getByRole("status")).toHaveTextContent("ready");
      const track = screen.getByTestId("track");
      const query = vi.spyOn(track, "querySelector");
      rerender(<Probe layout="week" />);
      unmount();
      const before = query.mock.calls.length;
      await act(async () => {
        loaded();
        await ready;
      });
      expect(query.mock.calls.length).toBe(before);
    } finally {
      if (fonts) Object.defineProperty(document, "fonts", fonts);
      else Reflect.deleteProperty(document, "fonts");
    }
  });
});
