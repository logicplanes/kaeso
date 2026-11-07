import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Sparkline } from "./sparkline.js";

const DAYS = ["Mon", "Tue", "Wed", "Thu"];
const live = (group: HTMLElement) => group.querySelector("[aria-live]") as HTMLElement;

describe("Sparkline", () => {
  afterEach(() => vi.restoreAllMocks());

  it("draws a trend as one image, a flat line without values and a dot for a single value", () => {
    const { rerender, container } = render(<Sparkline values={[3, 5, 4]} aria-label="Requests per day" />);
    expect(screen.getByRole("img", { name: "Requests per day" })).toBeInTheDocument();
    expect(container.querySelector("[data-dot]")).not.toBeNull();
    rerender(<Sparkline values={[]} aria-label="Requests per day" />);
    expect(container.querySelector("line")).not.toBeNull();
    rerender(<Sparkline values={[7]} fluid height={48} aria-label="Requests per day" />);
    expect(container.querySelectorAll("path")).toHaveLength(2);
  });

  it("is read value by value with the arrow keys, Home and End once it has labels", () => {
    render(
      <Sparkline values={[3, 5, Number.NaN, 4]} labels={DAYS} seriesLabel="Requests" formatValue={(v) => `${v} req`} fluid aria-label="Requests per day" />,
    );
    const group = screen.getByRole("group", { name: "Requests per day" });
    fireEvent.keyDown(group, { key: "ArrowRight" });
    expect(live(group)).toHaveTextContent("Thu: 4 req");
    fireEvent.keyDown(group, { key: "ArrowLeft" });
    expect(live(group)).toHaveTextContent("Wed: –");
    expect(group.querySelector("[data-crosshair]")).toBeNull();
    fireEvent.keyDown(group, { key: "Home" });
    expect(live(group)).toHaveTextContent("Mon: 3 req");
    expect(group.querySelector("[data-crosshair]")).not.toBeNull();
    fireEvent.keyDown(group, { key: "ArrowLeft" });
    expect(live(group)).toHaveTextContent("Mon: 3 req");
    fireEvent.keyDown(group, { key: "End" });
    expect(live(group)).toHaveTextContent("Thu: 4 req");
    fireEvent.keyDown(group, { key: "a" });
    expect(live(group)).toHaveTextContent("Thu: 4 req");
    fireEvent.keyDown(group, { key: "Escape" });
    expect(live(group)).toHaveTextContent("");
  });

  it("follows the pointer and forgets the value when the pointer or focus leaves", () => {
    render(<Sparkline values={[3, 5, 6, 4]} labels={DAYS} aria-label="Requests per day" />);
    const group = screen.getByRole("group", { name: "Requests per day" });
    vi.spyOn(group, "getBoundingClientRect").mockReturnValue({
      left: 0,
      width: 300,
      top: 0,
      height: 24,
      right: 300,
      bottom: 24,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    fireEvent.pointerMove(group, { clientX: 100 });
    expect(live(group)).toHaveTextContent("Tue: 5");
    fireEvent.pointerLeave(group);
    expect(live(group)).toHaveTextContent("");
    fireEvent.pointerMove(group, { clientX: 900 });
    expect(live(group)).toHaveTextContent("Thu: 4");
    fireEvent.blur(group);
    expect(live(group)).toHaveTextContent("");
  });

  it("picks the first value when it has no width to measure", () => {
    render(<Sparkline values={[3, 5]} labels={["Mon"]} aria-label="Requests per day" />);
    const group = screen.getByRole("group", { name: "Requests per day" });
    fireEvent.pointerMove(group, { clientX: 40 });
    expect(live(group)).toHaveTextContent("Mon: 3");
    fireEvent.keyDown(group, { key: "End" });
    expect(live(group)).toHaveTextContent(": 5");
  });
});
