import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { Textarea } from "./textarea.js";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("Textarea", () => {
  it("stops growing at exactly maxRows of its own line height, so no line is cut at the bottom", () => {
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(600);
    render(
      <Textarea
        aria-label="System prompt"
        mono
        maxRows={8}
        style={{ lineHeight: "18px", paddingTop: "10px", paddingBottom: "10px" }}
        defaultValue="You answer in one sentence."
      />,
    );
    expect(screen.getByRole("textbox", { name: "System prompt" }).style.height).toBe(`${8 * 18 + 20}px`);
  });

  it("keeps its rows while hidden, so a later step of a dialog shows it full height with its placeholder readable", () => {
    let observed: ResizeObserverCallback = () => {};
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(callback: ResizeObserverCallback) {
          observed = callback;
        }
        observe() {}
        disconnect() {}
      },
    );
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (run: FrameRequestCallback) => frames.push(run));
    const height = vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(0);
    render(
      <div hidden>
        <Textarea
          label="Sample text"
          placeholder="Paste a prompt to see what the rules find"
          style={{ lineHeight: "20px", paddingTop: "10px", paddingBottom: "10px" }}
        />
      </div>,
    );
    const field = screen.getByRole("textbox", { name: "Sample text", hidden: true });
    expect(field).toHaveAttribute("rows", "3");
    expect(field.style.height).toBe("auto");

    height.mockReturnValue(80);
    act(() => {
      observed([], {} as ResizeObserver);
      for (const run of frames.splice(0)) run(0);
    });
    expect(field.style.height).toBe("80px");
  });
});
