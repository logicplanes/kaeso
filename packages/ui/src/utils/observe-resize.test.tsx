import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useLayoutEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useLayoutWidth } from "../hooks/use-layout-width.js";

class FakeObserver {
  static all: FakeObserver[] = [];
  targets: Element[] = [];
  constructor(private readonly callback: ResizeObserverCallback) {
    FakeObserver.all.push(this);
  }
  observe(el: Element) {
    this.targets.push(el);
  }
  unobserve() {}
  disconnect() {
    this.targets = [];
  }
  deliver() {
    if (this.targets.length) this.callback([], this as unknown as ResizeObserver);
  }
}

type Globals = { IS_REACT_ACT_ENVIRONMENT?: boolean | undefined };
let frames: FrameRequestCallback[] = [];
let actEnvironment: boolean | undefined;

const startFrame = () => {
  const due = frames;
  frames = [];
  for (const run of due) run(0);
};
const observerStep = () => {
  for (const observer of FakeObserver.all) observer.deliver();
};

function Anchor({ name, width }: { name: string; width: string }) {
  const ref = useRef<HTMLDivElement | null>(null);
  const measured = useLayoutWidth(ref);
  return (
    <div ref={ref} data-testid={name} style={{ boxSizing: "border-box", width }}>
      {measured}
    </div>
  );
}

const popup: { place?: (top: number) => void } = {};

function Popup() {
  const [top, setTop] = useState(0);
  useLayoutEffect(() => {
    popup.place = setTop;
  }, []);
  return <div data-testid="popup">{top}</div>;
}

beforeEach(() => {
  FakeObserver.all = [];
  frames = [];
  vi.stubGlobal("ResizeObserver", FakeObserver);
  vi.stubGlobal("requestAnimationFrame", (run: FrameRequestCallback) => frames.push(run));
  actEnvironment = (globalThis as Globals).IS_REACT_ACT_ENVIRONMENT;
});

afterEach(() => {
  startFrame();
  (globalThis as Globals).IS_REACT_ACT_ENVIRONMENT = actEnvironment;
  vi.unstubAllGlobals();
});

describe("observeResize", () => {
  it("commits nothing while a floating popup renders synchronously in the same observer step, and applies the width in the next frame", () => {
    render(
      <>
        <Anchor name="trigger" width="392px" />
        <Popup />
      </>,
    );
    const trigger = screen.getByTestId("trigger");
    (globalThis as Globals).IS_REACT_ACT_ENVIRONMENT = false;

    trigger.style.width = "360px";
    observerStep();
    flushSync(() => popup.place?.(6));
    expect(screen.getByTestId("popup")).toHaveTextContent("6");
    expect(trigger).toHaveTextContent("392");

    startFrame();
    expect(trigger).toHaveTextContent("360");
  });

  it("applies every change measured in one frame together, in a single frame callback", () => {
    render(
      <>
        <Anchor name="first" width="200px" />
        <Anchor name="second" width="300px" />
      </>,
    );
    (globalThis as Globals).IS_REACT_ACT_ENVIRONMENT = false;
    screen.getByTestId("first").style.width = "210px";
    screen.getByTestId("second").style.width = "310px";
    observerStep();
    expect(frames).toHaveLength(1);

    startFrame();
    expect(screen.getByTestId("first")).toHaveTextContent("210");
    expect(screen.getByTestId("second")).toHaveTextContent("310");
  });

  it("measures once where the browser has no ResizeObserver", () => {
    vi.stubGlobal("ResizeObserver", undefined);
    const { unmount } = render(<Anchor name="plain" width="280px" />);
    expect(screen.getByTestId("plain")).toHaveTextContent("280");
    unmount();
    expect(frames).toHaveLength(0);
  });

  it("forgets a change measured just before the element went away", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const { unmount } = render(<Anchor name="gone" width="200px" />);
    (globalThis as Globals).IS_REACT_ACT_ENVIRONMENT = false;
    screen.getByTestId("gone").style.width = "240px";
    observerStep();
    unmount();

    startFrame();
    expect(screen.queryByTestId("gone")).toBeNull();
    expect(error).not.toHaveBeenCalled();
    error.mockRestore();
  });
});
