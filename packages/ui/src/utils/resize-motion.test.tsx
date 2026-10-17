import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { useState } from "react";
import { resizeSources, useResizeMotion } from "./resize-motion.js";

class FakeObserver {
  static all: FakeObserver[] = [];
  targets = new Set<Element>();
  disconnected = false;
  constructor(private readonly callback: ResizeObserverCallback) {
    FakeObserver.all.push(this);
  }
  observe(el: Element) {
    this.targets.add(el);
  }
  unobserve(el: Element) {
    this.targets.delete(el);
  }
  disconnect() {
    this.disconnected = true;
    this.targets.clear();
  }
  fire() {
    act(() => this.callback([], this as unknown as ResizeObserver));
  }
}

interface FakeAnimation {
  keyframes: Keyframe[];
  options: KeyframeAnimationOptions;
  cancel: () => void;
  cancelled: boolean;
  currentTime: number;
  onfinish: (() => void) | null;
}

let size = { w: 560, h: 300 };
let reached = { w: 0, h: 0 };
let tokens: Record<string, string> = {};
let runs: FakeAnimation[] = [];

function Box({ rows, onSettle }: { rows: string[]; onSettle?: () => void }) {
  const [box, setBox] = useState<HTMLElement | null>(null);
  useResizeMotion(box, onSettle);
  return (
    <div
      data-testid="box"
      ref={(el) => {
        if (el) Object.assign(el, { animate });
        setBox(el);
      }}
    >
      <div data-testid="frame">
        <h2>Create key</h2>
        <div data-dialog-body="">
          {rows.map((r) => (
            <p key={r}>{r}</p>
          ))}
        </div>
        <footer>Done</footer>
      </div>
    </div>
  );
}

function animate(keyframes: Keyframe[], options: KeyframeAnimationOptions) {
  const run: FakeAnimation = {
    keyframes,
    options,
    cancelled: false,
    currentTime: 0,
    onfinish: null,
    cancel() {
      run.cancelled = true;
    },
  };
  runs.push(run);
  return run;
}

const observer = () => FakeObserver.all.at(-1)!;
const box = () => screen.getByTestId("box");
const change = (next: { w?: number; h: number }) => {
  size = { w: next.w ?? size.w, h: next.h };
  observer().fire();
};
const finish = (run: FakeAnimation) => act(() => run.onfinish?.());

beforeEach(() => {
  FakeObserver.all = [];
  runs = [];
  size = { w: 560, h: 300 };
  tokens = { "--k-duration-dialog": "200ms", "--k-ease-standard": "cubic-bezier(0.2, 0, 0, 1)" };
  vi.stubGlobal("ResizeObserver", FakeObserver);
  vi.spyOn(window, "getComputedStyle").mockImplementation((el) => {
    const shown = el.hasAttribute("data-resizing") ? reached : size;
    return { width: `${shown.w}px`, height: `${shown.h}px`, getPropertyValue: (name: string) => tokens[name] ?? "" } as unknown as CSSStyleDeclaration;
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("resizeSources", () => {
  it("watches the parts that hold their size while the box animates: header, each row of the body and the footer", () => {
    render(<Box rows={["Name", "Expiry"]} />);
    expect(resizeSources(box()).map((el) => el.textContent)).toEqual(["Create key", "Name", "Expiry", "Done"]);
  });
});

describe("useResizeMotion", () => {
  it("opens without a size animation", () => {
    render(<Box rows={["Name"]} />);
    observer().fire();
    expect(runs).toEqual([]);
    expect(box()).not.toHaveAttribute("data-resizing");
  });
  it("grows from the old size to the new one with the dialog's duration and the standard curve, and leaves nothing fixed behind", () => {
    const settled = vi.fn();
    render(<Box rows={["Name"]} onSettle={settled} />);
    observer().fire();
    settled.mockClear();
    change({ h: 420 });
    expect(settled).not.toHaveBeenCalled();
    expect(runs).toHaveLength(1);
    expect(runs[0]!.keyframes).toEqual([
      { width: "560px", height: "300px" },
      { width: "560px", height: "420px" },
    ]);
    expect(runs[0]!.options).toEqual({ duration: 200, easing: "cubic-bezier(0.2, 0, 0, 1)" });
    expect(box()).toHaveAttribute("data-resizing");
    expect(box().style.cssText).toBe("");
    expect(box().querySelector("[data-dialog-body]")).toHaveAttribute("data-resize-clip");
    finish(runs[0]!);
    expect(box().querySelector("[data-dialog-body]")).not.toHaveAttribute("data-resize-clip");
    expect(box()).not.toHaveAttribute("data-resizing");
    expect(box().style.cssText).toBe("");
    expect(screen.getByTestId("frame").style.cssText).toBe("");
    expect(settled).toHaveBeenCalledOnce();
  });
  it("reads the duration token whether a stylesheet writes it in seconds or milliseconds", () => {
    tokens["--k-duration-dialog"] = " .2s";
    render(<Box rows={["Name"]} />);
    observer().fire();
    change({ h: 420 });
    expect(runs[0]!.options.duration).toBe(200);
  });
  it("shrinks back down to a shorter step instead of keeping the taller size", () => {
    render(<Box rows={["Name"]} />);
    observer().fire();
    change({ h: 520 });
    finish(runs[0]!);
    change({ h: 240 });
    expect(runs[1]!.keyframes).toEqual([
      { width: "560px", height: "520px" },
      { width: "560px", height: "240px" },
    ]);
    finish(runs[1]!);
    expect(box().style.cssText).toBe("");
  });
  it("turns a grow into a shrink midway from the size it has reached, without a jump", () => {
    render(<Box rows={["Name"]} />);
    observer().fire();
    change({ h: 500 });
    reached = { w: 560, h: 380 };
    change({ h: 260 });
    expect(runs[0]!.cancelled).toBe(true);
    expect(runs[1]!.keyframes).toEqual([
      { width: "560px", height: "380px" },
      { width: "560px", height: "260px" },
    ]);
    finish(runs[0]!);
    expect(box()).toHaveAttribute("data-resizing");
    finish(runs[1]!);
    expect(box()).not.toHaveAttribute("data-resizing");
  });
  it("animates the width too when the dialog changes size", () => {
    render(<Box rows={["Name"]} />);
    observer().fire();
    change({ w: 720, h: 360 });
    expect(runs[0]!.keyframes).toEqual([
      { width: "560px", height: "300px" },
      { width: "720px", height: "360px" },
    ]);
  });
  it("snaps with reduced motion, without a duration, while closing and when the window resized", () => {
    render(<Box rows={["Name"]} />);
    observer().fire();
    vi.stubGlobal("matchMedia", (q: string) => ({ matches: q.includes("reduce") }));
    change({ h: 400 });
    vi.unstubAllGlobals();
    vi.stubGlobal("ResizeObserver", FakeObserver);
    tokens["--k-duration-dialog"] = "";
    change({ h: 300 });
    tokens["--k-duration-dialog"] = "200ms";
    box().setAttribute("data-ending-style", "");
    change({ h: 420 });
    box().removeAttribute("data-ending-style");
    vi.stubGlobal("innerWidth", 375);
    change({ h: 300 });
    expect(runs).toEqual([]);
    change({ h: 360 });
    expect(runs).toHaveLength(1);
  });
  it("snaps while a list or menu anchored to a field is open or closing, so the popup never chases a moving field", () => {
    render(<Box rows={["Name"]} />);
    observer().fire();
    const positioner = document.createElement("div");
    positioner.setAttribute("data-side", "bottom");
    positioner.setAttribute("data-align", "start");
    positioner.innerHTML = '<div role="listbox"></div>';
    document.body.append(positioner);
    const shown = vi.spyOn(positioner, "getClientRects").mockReturnValue([new DOMRect(0, 0, 200, 300)] as unknown as DOMRectList);
    change({ h: 420 });
    expect(runs).toEqual([]);
    shown.mockReturnValue([] as unknown as DOMRectList);
    change({ h: 300 });
    expect(runs).toHaveLength(1);
    shown.mockReturnValue([new DOMRect(0, 0, 200, 300)] as unknown as DOMRectList);
    positioner.innerHTML = '<div role="tooltip"></div>';
    change({ h: 360 });
    expect(runs).toHaveLength(2);
    positioner.remove();
  });
  it("lets a running animation carry on from where it is when a notification keeps its target", () => {
    const settled = vi.fn();
    render(<Box rows={["Name"]} onSettle={settled} />);
    observer().fire();
    change({ h: 400 });
    runs[0]!.currentTime = 80;
    settled.mockClear();
    reached = { w: 560, h: 340 };
    observer().fire();
    expect(runs[0]!.cancelled).toBe(true);
    expect(runs[1]!.keyframes).toEqual(runs[0]!.keyframes);
    expect(runs[1]!.currentTime).toBe(80);
    expect(box()).toHaveAttribute("data-resizing");
    expect(settled).not.toHaveBeenCalled();
  });
  it("measures on a render that changed the content, before the observer reports it, so no frame shows the new size first", () => {
    const { rerender } = render(<Box rows={["Name"]} />);
    observer().fire();
    size = { w: 560, h: 420 };
    rerender(<Box rows={["Name"]} />);
    expect(runs).toHaveLength(1);
    expect(runs[0]!.keyframes).toEqual([
      { width: "560px", height: "300px" },
      { width: "560px", height: "420px" },
    ]);
    observer().fire();
    expect(runs).toHaveLength(2);
    expect(runs[1]!.keyframes).toEqual(runs[0]!.keyframes);
  });
  it("watches rows that appear and measures again when one goes away", () => {
    const { rerender } = render(<Box rows={["Name"]} />);
    observer().fire();
    rerender(<Box rows={["Name", "Expiry"]} />);
    expect([...observer().targets].map((el) => el.textContent)).toContain("Expiry");
    size = { w: 560, h: 340 };
    rerender(<Box rows={["Name"]} />);
    expect([...observer().targets].map((el) => el.textContent)).not.toContain("Expiry");
    expect(runs[0]!.keyframes).toEqual([
      { width: "560px", height: "300px" },
      { width: "560px", height: "340px" },
    ]);
  });
  it("observes only while mounted, and cleans up a running animation", () => {
    const { unmount } = render(<Box rows={["Name"]} />);
    observer().fire();
    change({ h: 400 });
    const el = box();
    unmount();
    expect(observer().disconnected).toBe(true);
    expect(runs[0]!.cancelled).toBe(true);
    expect(el).not.toHaveAttribute("data-resizing");
    expect(el.style.cssText).toBe("");
  });
});
