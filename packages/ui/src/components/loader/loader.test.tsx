import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, renderHook, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { Loader } from "./status.js";
import { Spinner, useDelayedLoading } from "./loader.js";
import { Button } from "../button/button.js";
import { motion } from "@logicplanes/kaeso-tokens";

describe("Spinner", () => {
  it("draws three routes, and is silent beside a control that says it is busy", () => {
    const { container } = render(<Spinner />);
    const svg = container.querySelector("svg")!;
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg.querySelectorAll(".k-spinner-route")).toHaveLength(3);
    expect(svg.getAttribute("class")).toContain("k-spinner");
    expect(svg.getAttribute("class")).toContain("size-16");
    expect(new Set([...svg.querySelectorAll("path")].map((p) => p.getAttribute("d"))).size).toBe(1);
  });
  it("with a label, is a status with that name", () => {
    render(<Spinner label="Loading your session" size="xl" />);
    const s = screen.getByRole("status", { name: "Loading your session" });
    expect(s.getAttribute("class")).toContain("size-32");
  });
});

describe("Spinner motion", () => {
  const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
  it("turns the svg only, on the pulse easing, a third per beat", () => {
    expect(css).toMatch(/\.k-spinner \{[^}]*animation: k-spinner var\(--k-spinner-turn\) var\(--k-ease-pulse\) infinite;/);
    const frames = css.slice(css.indexOf("@keyframes k-spinner {"), css.indexOf("@keyframes k-spinner-handoff"));
    expect(frames).toContain("rotate(120deg)");
    expect(frames).not.toMatch(/opacity|stroke-dash/);
    expect(css).toMatch(/\.k-spinner-route \{\s*vector-effect: non-scaling-stroke;/);
  });
  it("with reduced motion, holds still and passes the light by opacity alone", () => {
    const start = css.indexOf("@keyframes k-spinner-handoff");
    const reduced = css.slice(start, css.indexOf("\n}\n", css.indexOf("@layer components", start)));
    expect(reduced).toMatch(/@media \(prefers-reduced-motion: reduce\) \{\s*\.k-spinner \.k-spinner-route \{\s*animation: k-spinner-handoff/);
    const handoff = reduced.slice(0, reduced.indexOf(".k-spinner {"));
    expect(handoff).not.toMatch(/transform|rotate/);
  });
});

describe("show-after-delay", () => {
  const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
  it("hides by visibility for the spinner's delay, also with reduced motion (which shortens durations, not delays)", () => {
    expect(css).toMatch(/@utility show-after-delay \{\s*animation: k-hidden 1ms var\(--k-spinner-delay\) backwards;/);
    expect(css).toMatch(/@keyframes k-hidden \{\s*from,\s*to \{\s*visibility: hidden;/);
  });
});

describe("Loader", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("is a status region that announces Loading without a label, in secondary text, once the spinner's delay has passed", () => {
    render(<Loader />);
    expect(screen.getByRole("status")).not.toHaveTextContent("Loading");
    expect(screen.getByRole("status").lastElementChild).toBeEmptyDOMElement();
    act(() => vi.advanceTimersByTime(300));
    expect(screen.getByRole("status")).toHaveTextContent("Loading");
    expect(screen.getByRole("status").className).toContain("text-fg-secondary");
  });
  it("takes a tone and a size", () => {
    const { container } = render(<Loader tone="tertiary" size="xl" />);
    expect(screen.getByRole("status").className).toContain("text-fg-tertiary");
    expect(container.querySelector(".k-spinner")!.getAttribute("class")).toContain("size-32");
  });
  it("can keep its label for assistive technology only", () => {
    render(<Loader label="Signing you in" hideLabel delay={false} />);
    expect(screen.getByRole("status")).toHaveTextContent("Signing you in");
    expect(screen.getByText("Signing you in").className).toContain("sr-only");
  });
  it("keeps its place while it waits out the delay, then shows, putting its words into the region it rendered empty", () => {
    render(<Loader label="Saving provider…" />);
    const inner = screen.getByRole("status").firstElementChild!;
    const live = screen.getByRole("status").lastElementChild!;
    expect(inner.className).toContain("invisible");
    expect(live).toBeEmptyDOMElement();
    act(() => vi.advanceTimersByTime(299));
    expect(inner.className).toContain("invisible");
    act(() => vi.advanceTimersByTime(1));
    expect(inner.className).not.toContain("invisible");
    expect(live).toHaveTextContent("Saving provider…");
  });
  it("shows at once with delay={false}", () => {
    render(<Loader label="Saving provider…" delay={false} />);
    expect(screen.getByRole("status").firstElementChild!.className).not.toContain("invisible");
  });
});

describe("useDelayedLoading", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  const hook = (initial: boolean, delay?: number, minDuration?: number) =>
    renderHook(({ loading }) => useDelayedLoading(loading, delay, minDuration), { initialProps: { loading: initial } });

  it("shows nothing for work that ends inside the delay", () => {
    const h = hook(true);
    act(() => vi.advanceTimersByTime(250));
    expect(h.result.current).toBe(false);
    h.rerender({ loading: false });
    act(() => vi.advanceTimersByTime(1000));
    expect(h.result.current).toBe(false);
  });
  it("shows after the delay (300ms by default) and stays at least the minimum (400ms)", () => {
    const h = hook(true);
    act(() => vi.advanceTimersByTime(300));
    expect(h.result.current).toBe(true);
    act(() => vi.advanceTimersByTime(50));
    h.rerender({ loading: false });
    expect(h.result.current).toBe(true);
    act(() => vi.advanceTimersByTime(349));
    expect(h.result.current).toBe(true);
    act(() => vi.advanceTimersByTime(1));
    expect(h.result.current).toBe(false);
  });
  it("hides at once when the work outlasted the minimum", () => {
    const h = hook(true);
    act(() => vi.advanceTimersByTime(1200));
    h.rerender({ loading: false });
    act(() => vi.advanceTimersByTime(0));
    expect(h.result.current).toBe(false);
  });
  it("stays shown, without a flicker, when work starts again during the minimum", () => {
    const h = hook(true);
    act(() => vi.advanceTimersByTime(300));
    h.rerender({ loading: false });
    act(() => vi.advanceTimersByTime(100));
    h.rerender({ loading: true });
    act(() => vi.advanceTimersByTime(1000));
    expect(h.result.current).toBe(true);
  });
  it("defaults to the spinner tokens", () => {
    const h = hook(true);
    act(() => vi.advanceTimersByTime(motion.spinner.delay - 1));
    expect(h.result.current).toBe(false);
    act(() => vi.advanceTimersByTime(1));
    expect(h.result.current).toBe(true);
    h.rerender({ loading: false });
    act(() => vi.advanceTimersByTime(motion.spinner.minimum - 1));
    expect(h.result.current).toBe(true);
    act(() => vi.advanceTimersByTime(1));
    expect(h.result.current).toBe(false);
  });
  it("with no delay, shows on the first render", () => {
    const h = hook(true, 0, 0);
    expect(h.result.current).toBe(true);
  });
});

describe("Button loading", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("is busy at once and keeps its label until the spinner's delay has passed", () => {
    const { container, rerender } = render(<Button>Save</Button>);
    rerender(<Button loading>Save</Button>);
    const b = screen.getByRole("button");
    expect(b).toHaveAttribute("aria-busy", "true");
    expect(container.querySelector(".k-spinner")).toBeNull();
    act(() => vi.advanceTimersByTime(300));
    expect(container.querySelector(".k-spinner")).not.toBeNull();
    expect(screen.getByText("Save").closest("span")!.className).toContain("opacity-0");
  });
  it("stays busy, ignoring clicks, until the spinner's minimum has passed", () => {
    const onClick = vi.fn();
    const { rerender } = render(<Button onClick={onClick}>Save</Button>);
    rerender(
      <Button loading onClick={onClick}>
        Save
      </Button>,
    );
    act(() => vi.advanceTimersByTime(320));
    rerender(<Button onClick={onClick}>Save</Button>);
    const b = screen.getByRole("button");
    expect(b).toHaveAttribute("aria-busy", "true");
    act(() => b.click());
    expect(onClick).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(400));
    expect(b).not.toHaveAttribute("aria-busy");
    act(() => b.click());
    expect(onClick).toHaveBeenCalledOnce();
  });
});
