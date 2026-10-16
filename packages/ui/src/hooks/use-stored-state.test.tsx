import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { useStoredState } from "./use-stored-state.js";

function Probe({ id, storageKey, initial = false }: { id: string; storageKey?: string; initial?: boolean }) {
  const [value, setValue] = useStoredState(storageKey, initial);
  return (
    <button type="button" data-testid={id} onClick={() => setValue(!value)}>
      {String(value)}
    </button>
  );
}

describe("useStoredState", () => {
  afterEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("draws the remembered value in the first render, so a reload never shows the default first", () => {
    localStorage.setItem("kaeso:sidebar:collapsed", "true");
    const seen: string[] = [];
    function First() {
      const [collapsed] = useStoredState("sidebar:collapsed", false);
      seen.push(String(collapsed));
      return null;
    }
    render(<First />);
    expect(seen[0]).toBe("true");
    expect(seen).not.toContain("false");
  });

  it("stores a change and every hook on the key follows it", () => {
    render(
      <>
        <Probe id="a" storageKey="pane" />
        <Probe id="b" storageKey="pane" />
      </>,
    );
    act(() => screen.getByTestId("a").click());
    expect(localStorage.getItem("kaeso:pane")).toBe("true");
    expect(screen.getByTestId("b").textContent).toBe("true");
  });

  it("keeps the value for the page when storage refuses it", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    render(<Probe id="a" storageKey="blocked" />);
    act(() => screen.getByTestId("a").click());
    expect(screen.getByTestId("a").textContent).toBe("true");
  });

  it("without a key it is plain state", () => {
    render(<Probe id="a" />);
    act(() => screen.getByTestId("a").click());
    expect(screen.getByTestId("a").textContent).toBe("true");
    expect(localStorage.length).toBe(0);
  });

  it("falls back to the initial value when the stored one isn't JSON", () => {
    localStorage.setItem("kaeso:broken", "{nope");
    render(<Probe id="a" storageKey="broken" initial />);
    expect(screen.getByTestId("a").textContent).toBe("true");
  });
});
