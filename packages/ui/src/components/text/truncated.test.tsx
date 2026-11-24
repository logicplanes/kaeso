import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TruncatedText } from "./truncated.js";
import { Text } from "./text.js";
import { TooltipProvider } from "../tooltip/tooltip-provider.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

// An observer that reports only when the test says the browser laid the page out, so the test sees when measuring happens.
class FakeObserver {
  static made = 0;
  static all: FakeObserver[] = [];
  targets = new Set<Element>();
  constructor(private readonly callback: ResizeObserverCallback) {
    FakeObserver.made++;
    FakeObserver.all.push(this);
  }
  observe(el: Element) {
    this.targets.add(el);
  }
  unobserve(el: Element) {
    this.targets.delete(el);
  }
  disconnect() {
    this.targets.clear();
  }
}

let frames: FrameRequestCallback[] = [];
let wide = new Set<string>();
let reads = 0;

// The browser lays the page out: every observer reports its elements, then the next frame runs.
function layout() {
  act(() => {
    for (const o of FakeObserver.all)
      if (o.targets.size)
        o["callback"](
          [...o.targets].map((target) => ({ target }) as ResizeObserverEntry),
          o as unknown as ResizeObserver,
        );
    const due = frames;
    frames = [];
    for (const run of due) run(0);
  });
}

beforeEach(() => {
  FakeObserver.made = 0;
  FakeObserver.all = [];
  frames = [];
  wide = new Set();
  reads = 0;
  // A fresh class per test, as a fresh page would have: the shared observer is made again.
  vi.stubGlobal("ResizeObserver", class extends FakeObserver {});
  vi.stubGlobal("requestAnimationFrame", (run: FrameRequestCallback) => frames.push(run));
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (this: HTMLElement) {
    reads++;
    return wide.has(this.textContent ?? "") ? 400 : 120;
  });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(120);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const cells = (n: number) =>
  Array.from({ length: n }, (_, i) => (
    <div key={i}>
      <TruncatedText>{`Model ${i}`}</TruncatedText>
      <Text truncate>{`Provider ${i}`}</Text>
    </div>
  ));

describe("text that may be cut off, as in table cells", () => {
  it("reads no layout while it renders and shares one observer, however many cells there are", async () => {
    render(<TooltipProvider>{cells(60)}</TooltipProvider>);
    await act(() => lazyModulesSettled());
    expect(reads).toBe(0);
    expect(FakeObserver.made).toBe(1);
    layout();
    expect(reads).toBe(120);
  });

  it("mounts no tooltip for text that fits, and the tooltip's trigger once it is cut", async () => {
    wide.add("Model 1");
    render(<TooltipProvider>{cells(3)}</TooltipProvider>);
    await act(() => lazyModulesSettled());
    layout();
    await act(() => lazyModulesSettled());
    expect(screen.getByText("Model 0")).not.toHaveAttribute("data-trigger-disabled");
    expect(screen.getByText("Model 0")).not.toHaveAttribute("tabindex");
    expect(document.querySelectorAll("[data-base-ui-tooltip-trigger]")).toHaveLength(1);
    expect(screen.getByText("Model 1")).toHaveAttribute("data-base-ui-tooltip-trigger");
  });

  it("is a tab stop when cut that shows all of it on keyboard focus, with the same element as before", async () => {
    const user = userEvent.setup();
    wide.add("Northwind Labs Research and Development");
    render(
      <TooltipProvider>
        <TruncatedText>Northwind Labs Research and Development</TruncatedText>
      </TooltipProvider>,
    );
    await act(() => lazyModulesSettled());
    const text = screen.getByText("Northwind Labs Research and Development");
    expect(text).not.toHaveAttribute("tabindex");
    layout();
    await act(() => lazyModulesSettled());
    expect(screen.getByText("Northwind Labs Research and Development")).toBe(text);
    expect(text).toHaveAttribute("tabindex", "0");
    await user.tab();
    expect(text).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Northwind Labs Research and Development");
  });

  it("measures again when a web font arrives and widens the text inside the same box", async () => {
    const fonts = new EventTarget();
    Object.defineProperty(document, "fonts", { value: fonts, configurable: true });
    render(
      <TooltipProvider>
        <TruncatedText>open_pull_request</TruncatedText>
      </TooltipProvider>,
    );
    layout();
    expect(screen.getByText("open_pull_request")).not.toHaveAttribute("tabindex");
    wide.add("open_pull_request");
    act(() => {
      fonts.dispatchEvent(new Event("loadingdone"));
      const due = frames;
      frames = [];
      for (const run of due) run(0);
    });
    await act(() => lazyModulesSettled());
    expect(screen.getByText("open_pull_request")).toHaveAttribute("tabindex", "0");
    Reflect.deleteProperty(document, "fonts");
  });

  it("leaves the tab order again when a wider window lets it fit", async () => {
    wide.add("Northwind Labs Research and Development");
    render(
      <TooltipProvider>
        <Text truncate>Northwind Labs Research and Development</Text>
      </TooltipProvider>,
    );
    layout();
    await act(() => lazyModulesSettled());
    expect(screen.getByText("Northwind Labs Research and Development")).toHaveAttribute("tabindex", "0");
    wide.clear();
    layout();
    expect(screen.getByText("Northwind Labs Research and Development")).not.toHaveAttribute("tabindex");
  });
});
