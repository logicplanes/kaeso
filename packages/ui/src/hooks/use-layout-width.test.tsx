import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef } from "react";
import { useLayoutWidth } from "./use-layout-width";
import { Combobox } from "../components/combobox/combobox";

function Probe() {
  const ref = useRef<HTMLDivElement | null>(null);
  const width = useLayoutWidth(ref);
  return (
    <div ref={ref} data-testid="anchor" style={{ boxSizing: "border-box", width: "312.5px", transform: "scale(0.98)" }}>
      {width}
    </div>
  );
}

describe("useLayoutWidth", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("reads the width as laid out, before the first paint, and ignores transforms", () => {
    render(<Probe />);
    expect(screen.getByTestId("anchor")).toHaveTextContent("312.5");
  });
  it("follows a resize in the next frame, never inside the observer's callback", async () => {
    const callbacks: ResizeObserverCallback[] = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(cb: ResizeObserverCallback) {
          callbacks.push(cb);
        }
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
    render(<Probe />);
    const anchor = screen.getByTestId("anchor");
    anchor.style.width = "400px";
    act(() => {
      for (const cb of callbacks) cb([], {} as ResizeObserver);
    });
    expect(anchor).toHaveTextContent("312.5");
    await act(() => new Promise((r) => requestAnimationFrame(() => r(undefined))));
    expect(anchor).toHaveTextContent("400");
  });
});

describe("Combobox popup width", () => {
  it("is at least the field's layout width, not Base UI's on-screen --anchor-width", async () => {
    render(<Combobox aria-label="New owner" options={[{ value: "lin", label: "Lin Park" }]} />);
    await userEvent.click(screen.getByRole("combobox", { name: "New owner" }));
    const popup = (await screen.findByRole("option", { name: "Lin Park" })).closest<HTMLElement>(".min-w-anchor");
    expect(popup).not.toBeNull();
    expect(popup!.style.getPropertyValue("--k-anchor-width")).toMatch(/^\d+(\.\d+)?px$/);
  });
});
