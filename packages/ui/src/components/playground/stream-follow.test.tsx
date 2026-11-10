import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { CompareGrid } from "./compare-grid.js";
import { ResponseMetrics } from "./response-metrics.js";
import { ResponseView } from "./response-view.js";

let grow: (() => void)[] = [];
const Original = globalThis.ResizeObserver;

beforeEach(() => {
  grow = [];
  globalThis.ResizeObserver = class {
    constructor(cb: () => void) {
      grow.push(cb);
    }
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});
afterEach(() => {
  globalThis.ResizeObserver = Original;
});

function thread(followKey?: number) {
  return (
    <CompareGrid aria-label="Models">
      <CompareGrid.Column label="GPT-5" header="GPT-5" {...(followKey !== undefined ? { followKey } : {})}>
        <p>The reply so far.</p>
      </CompareGrid.Column>
    </CompareGrid>
  );
}

function sized(el: HTMLElement, height: { scroll: number; client: number }) {
  let top = 0;
  const max = () => Math.max(0, height.scroll - height.client);
  Object.defineProperty(el, "scrollHeight", { configurable: true, get: () => height.scroll });
  Object.defineProperty(el, "clientHeight", { configurable: true, get: () => height.client });
  Object.defineProperty(el, "scrollTop", {
    configurable: true,
    get: () => Math.min(top, max()),
    set: (v: number) => {
      top = Math.max(0, Math.min(v, max()));
    },
  });
}

function streamed(height: { scroll: number; client: number }, by: number) {
  height.scroll += by;
  act(() => grow.forEach((cb) => cb()));
}

describe("a column that follows a stream", () => {
  it("follows the end while the reader is there and lets go at the first scroll up, even a few pixels", () => {
    render(thread());
    const body = screen.getByRole("region", { name: "GPT-5 thread" });
    const height = { scroll: 1000, client: 400 };
    sized(body, height);
    streamed(height, 100);
    expect(body.scrollTop).toBe(700);

    fireEvent.wheel(body, { deltaY: -3 });
    body.scrollTop = 697;
    fireEvent.scroll(body);
    streamed(height, 200);
    expect(body.scrollTop).toBe(697);

    body.scrollTop = 500;
    fireEvent.scroll(body);
    streamed(height, 50);
    expect(body.scrollTop).toBe(500);
  });

  it("follows again once the reader is back at the end, or when a new prompt is sent", () => {
    const { rerender } = render(thread(1));
    const body = screen.getByRole("region", { name: "GPT-5 thread" });
    const height = { scroll: 1000, client: 400 };
    sized(body, height);
    streamed(height, 0);
    fireEvent.keyDown(body, { key: "PageUp" });
    body.scrollTop = 300;
    fireEvent.scroll(body);
    streamed(height, 100);
    expect(body.scrollTop).toBe(300);

    body.scrollTop = 700;
    fireEvent.scroll(body);
    streamed(height, 100);
    expect(body.scrollTop).toBe(800);

    body.scrollTop = 200;
    fireEvent.scroll(body);
    rerender(thread(2));
    expect(body.scrollTop).toBe(800);
    streamed(height, 100);
    expect(body.scrollTop).toBe(900);
  });

  it("keeps following when the content shrinks under the reader or a touch moves down", () => {
    render(thread());
    const body = screen.getByRole("region", { name: "GPT-5 thread" });
    const height = { scroll: 1000, client: 400 };
    sized(body, height);
    streamed(height, 0);
    expect(body.scrollTop).toBe(600);
    height.scroll = 900;
    fireEvent.scroll(body);
    fireEvent.touchStart(body, { touches: [{ clientY: 300 }] });
    fireEvent.touchMove(body, { touches: [{ clientY: 200 }] });
    streamed(height, 100);
    expect(body.scrollTop).toBe(600);

    fireEvent.touchMove(body, { touches: [{ clientY: 260 }] });
    streamed(height, 100);
    expect(body.scrollTop).toBe(600);
  });

  it("doesn't let go of a column that has nothing to scroll yet", () => {
    render(thread());
    const body = screen.getByRole("region", { name: "GPT-5 thread" });
    const height = { scroll: 300, client: 400 };
    sized(body, height);
    fireEvent.wheel(body, { deltaY: -40 });
    streamed(height, 400);
    expect(body.scrollTop).toBe(300);
  });
});

describe("a response's row as it finishes", () => {
  it("lines the metrics up with the reply's text while it streams, then slides Copy and Retry in beside them", () => {
    const footer = <ResponseMetrics live inputTokens={null} outputTokens={null} />;
    const { container, rerender } = render(<ResponseView status="streaming" text="Half a reply" footer={footer} />);
    expect(container.querySelector("[data-reply-actions]")).toBeNull();
    expect(screen.queryByRole("button", { name: "Copy response" })).toBeNull();
    expect(container.querySelector("dl")?.parentElement).toHaveClass("px-12");

    rerender(<ResponseView status="done" text="Half a reply, then the rest." footer={footer} onRetry={() => undefined} />);
    const actions = container.querySelector("[data-reply-actions]");
    expect(actions).toHaveClass("starting:-ml-72", "starting:opacity-0", "reduced:transition-none");
    expect(actions).toContainElement(screen.getByRole("button", { name: "Copy response" }));
    expect(actions).toContainElement(screen.getByRole("button", { name: "Retry" }));
    expect(container.querySelector("dl")?.parentElement).toHaveClass("px-8");
  });

  it("slides one button in by its own width", () => {
    const { container } = render(<ResponseView status="done" text="A reply" footer={<ResponseMetrics duration={900} />} />);
    expect(container.querySelector("[data-reply-actions]")).toHaveClass("starting:-ml-36");
  });
});
