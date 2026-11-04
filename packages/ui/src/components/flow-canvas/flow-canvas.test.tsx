import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef, useState } from "react";
import { FlowCanvas, type FlowCanvasNode } from "./flow-canvas.js";
import { LocaleProvider } from "../../i18n/index.js";
import { Menu } from "../menu/menu.js";
import de from "../../i18n/locales/de.js";

const route: FlowCanvasNode = {
  id: "start",
  kind: "Start",
  title: "fast",
  children: [
    {
      id: "root",
      insertLabel: "Insert a step before the condition",
      node: {
        id: "plan",
        kind: "Condition",
        title: "x-plan is paid",
        subtitle: "1 rule",
        children: [
          {
            id: "plan-then",
            label: "Then",
            node: {
              id: "opus",
              kind: "Model",
              title: "claude-opus-4-1",
              subtitle: "Anthropic",
              leading: <span data-testid="badge" />,
              terminal: true,
              children: [{ id: "opus-fb", label: "Fallback", variant: "fallback", addLabel: "Add a fallback to claude-opus-4-1" }],
            },
          },
          {
            id: "plan-else",
            label: "Else",
            node: {
              id: "mini",
              kind: "Model",
              title: "gpt-5-mini",
              label: "gpt-5-mini",
              subtitle: "OpenAI",
              terminal: true,
              status: { tone: "danger", label: "Model removed" },
            },
          },
        ],
      },
    },
  ],
};

const transform = () => screen.getByRole("tree").style.transform;
const scale = () => Number(/scale\(([\d.]+)\)/.exec(transform())![1]);
const translate = () => /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(transform())!.slice(1).map(Number);

afterEach(() => vi.restoreAllMocks());

function wheel(el: HTMLElement, init: Partial<Pick<WheelEvent, "deltaX" | "deltaY" | "deltaMode" | "ctrlKey" | "metaKey" | "clientX" | "clientY">>) {
  const e = new Event("wheel", { bubbles: true, cancelable: true });
  for (const [key, value] of Object.entries({ deltaX: 0, deltaY: 0, deltaMode: 0, ctrlKey: false, metaKey: false, clientX: 0, clientY: 0, ...init }))
    Object.defineProperty(e, key, { value });
  act(() => {
    el.dispatchEvent(e);
  });
  return e;
}

function size(w: number, h: number) {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(w);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(h);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    right: w,
    bottom: h,
    width: w,
    height: h,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
}

describe("FlowCanvas", () => {
  it("shows the route as a named tree of steps with their kinds, details and statuses", () => {
    const ref = createRef<HTMLDivElement>();
    const { container } = render(<FlowCanvas ref={ref} root={route} aria-label="Route of fast" />);
    expect(ref.current).toHaveAttribute("data-flow-canvas");
    const tree = screen.getByRole("tree", { name: "Route of fast" });
    expect(tree).toHaveAccessibleDescription(/Arrow keys move between steps/);
    const items = within(tree).getAllByRole("treeitem");
    expect(items.map((i) => i.textContent)).toEqual([
      "Startfast",
      "Conditionx-plan is paid1 rule",
      "Modelclaude-opus-4-1Anthropic",
      "ModelModel removedgpt-5-miniOpenAI",
    ]);
    expect(items.map((i) => i.getAttribute("aria-level"))).toEqual(["1", "2", "3", "3"]);
    expect(items[2]).toHaveAttribute("aria-posinset", "1");
    expect(items[3]).toHaveAttribute("aria-setsize", "2");
    expect(items[3]).toHaveClass("ring-invalid");
    expect(screen.getByTestId("badge")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Add a fallback/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Insert/ })).toBeNull();
    expect(
      within(container.querySelector("ul.sr-only") as HTMLElement)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["fast to x-plan is paid", "x-plan is paid to claude-opus-4-1, Then", "x-plan is paid to gpt-5-mini, Else"]);
    expect([...container.querySelectorAll("[data-flow-label]")].map((l) => l.textContent)).toEqual(["Then", "Else"]);
    expect(container.querySelector("[data-flow-canvas]")).toHaveClass("flow-canvas-dots");
  });

  it("selects a step on click and with Enter, highlights its lines, and clears with Escape or a click on the background", async () => {
    const onSelect = vi.fn();
    function Selecting() {
      const [selected, setSelected] = useState<string | null>(null);
      return (
        <FlowCanvas
          root={route}
          aria-label="Route"
          selected={selected}
          onSelect={(id) => {
            onSelect(id);
            setSelected(id);
          }}
        />
      );
    }
    const { container } = render(<Selecting />);
    const opus = screen.getByRole("treeitem", { name: /claude-opus-4-1/ });
    await userEvent.click(opus);
    expect(onSelect).toHaveBeenLastCalledWith("opus");
    expect(opus).toHaveAttribute("aria-selected", "true");
    expect(container.querySelector('[data-flow-wire="wire:opus"]')).toHaveAttribute("stroke", "var(--k-text-secondary)");
    expect(container.querySelector('[data-flow-wire="wire:mini"]')).toHaveAttribute("stroke", "var(--k-text-tertiary)");
    await userEvent.keyboard("{Escape}");
    expect(onSelect).toHaveBeenLastCalledWith(null);
    await userEvent.keyboard("{Escape}");
    expect(onSelect).toHaveBeenCalledTimes(2);
    await userEvent.keyboard("{Enter}");
    expect(onSelect).toHaveBeenLastCalledWith("opus");
    const canvas = container.querySelector("[data-flow-canvas]") as HTMLElement;
    fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, pointerType: "mouse", clientX: 10, clientY: 10 });
    fireEvent.pointerUp(canvas, { pointerId: 1, pointerType: "mouse", clientX: 11, clientY: 10 });
    expect(onSelect).toHaveBeenLastCalledWith(null);
  });

  it("moves through the tree with the arrow keys, Home and End, as one tab stop", async () => {
    const onDelete = vi.fn();
    render(<FlowCanvas root={route} aria-label="Route" onDelete={onDelete} />);
    const [start, plan, opus, mini] = screen.getAllByRole("treeitem");
    expect([start, plan, opus, mini].map((i) => i!.tabIndex)).toEqual([0, -1, -1, -1]);
    await userEvent.tab();
    expect(start).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(plan).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(opus).toHaveFocus();
    expect(opus!.tabIndex).toBe(0);
    await userEvent.keyboard("{ArrowRight}");
    expect(mini).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(mini).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(opus).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}");
    expect(plan).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(mini).toHaveFocus();
    await userEvent.keyboard("{Delete}");
    expect(onDelete).toHaveBeenCalledWith("mini");
    await userEvent.keyboard("{Home}");
    expect(start).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}");
    expect(start).toHaveFocus();
    await userEvent.keyboard("a");
    expect(start).toHaveFocus();
  });

  it("adds steps through open slots and the + between steps, whose menus the app fills", async () => {
    const addMenu = vi.fn(({ insert }: { edge: string; insert: boolean }) => (
      <Menu.Item onClick={() => undefined}>{insert ? "Insert model" : "Add model"}</Menu.Item>
    ));
    const { container } = render(<FlowCanvas root={route} aria-label="Route" addMenu={addMenu} />);
    const slot = screen.getByRole("treeitem", { name: "Add a fallback to claude-opus-4-1" });
    await userEvent.click(slot);
    expect(await screen.findByRole("menuitem", { name: "Add model" })).toBeInTheDocument();
    expect(addMenu).toHaveBeenCalledWith({ edge: "opus-fb", insert: false });
    await userEvent.keyboard("{Escape}");
    const insert = screen.getByRole("button", { name: "Insert a step before the condition" });
    expect(insert.tabIndex).toBe(-1);
    await userEvent.click(insert);
    expect(await screen.findByRole("menuitem", { name: "Insert model" })).toBeInTheDocument();
    expect(addMenu).toHaveBeenCalledWith({ edge: "root", insert: true });
    expect(screen.getAllByRole("button", { name: "Insert a step here" })).toHaveLength(2);
    expect(container.querySelector('[data-flow-wire="wire:opus::opus-fb"]')).toHaveAttribute("stroke-dasharray", "4 5");
  });

  it("opens a step's actions from its button and with Shift F10, without selecting it", async () => {
    const onSelect = vi.fn();
    render(
      <FlowCanvas
        root={route}
        aria-label="Route"
        onSelect={onSelect}
        nodeActions={(id) => (id === "start" ? null : <Menu.Item onClick={() => undefined}>Remove {id}</Menu.Item>)}
      />,
    );
    expect(screen.getAllByRole("treeitem")[0]).not.toHaveAttribute("aria-haspopup");
    const opus = screen.getByRole("treeitem", { name: /claude-opus-4-1/ });
    expect(opus).toHaveAttribute("aria-haspopup", "menu");
    await userEvent.click(within(opus).getByRole("button", { name: "Actions for claude-opus-4-1", hidden: true }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Remove opus" }));
    expect(onSelect).not.toHaveBeenCalled();
    await userEvent.click(within(opus).getByRole("button", { name: "Actions for claude-opus-4-1", hidden: true }));
    expect(await screen.findByRole("menuitem", { name: "Remove opus" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    act(() => screen.getByRole("treeitem", { name: /gpt-5-mini/ }).focus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    expect(await screen.findByRole("menuitem", { name: "Remove mini" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    act(() => within(opus).getByRole("button", { name: "Actions for claude-opus-4-1", hidden: true }).focus());
    expect(opus).toHaveFocus();
  });

  it("fits the route on first render and keeps the view when the route changes", () => {
    size(800, 420);
    const { rerender } = render(<FlowCanvas root={route} aria-label="Route" />);
    expect(scale()).toBeLessThan(1);
    expect(scale()).toBeGreaterThanOrEqual(0.67);
    const before = transform();
    rerender(<FlowCanvas root={{ ...route, title: "faster" }} aria-label="Route" />);
    expect(transform()).toBe(before);
  });

  it("zooms with the buttons and keys between 25% and 150%, and fits again", async () => {
    size(1200, 900);
    render(<FlowCanvas root={route} aria-label="Route" />);
    expect(scale()).toBe(1);
    const zoomIn = screen.getByRole("button", { name: "Zoom in" });
    const zoomOut = screen.getByRole("button", { name: "Zoom out" });
    await userEvent.click(zoomIn);
    expect(scale()).toBe(1.2);
    await userEvent.click(zoomIn);
    expect(scale()).toBe(1.5);
    expect(zoomIn).toHaveAttribute("aria-disabled", "true");
    const levels = [];
    for (let i = 0; i < 7; i++) {
      await userEvent.click(zoomOut);
      levels.push(scale());
    }
    expect(levels).toEqual([1.2, 1, 0.8, 0.67, 0.5, 0.33, 0.25]);
    expect(zoomOut).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(screen.getByRole("button", { name: "Fit to view" }));
    expect(scale()).toBe(1);
    act(() => screen.getAllByRole("treeitem")[0]!.focus());
    await userEvent.keyboard("+");
    expect(scale()).toBe(1.2);
    await userEvent.keyboard("-");
    expect(scale()).toBe(1);
    await userEvent.keyboard("-");
    await userEvent.keyboard("0");
    expect(scale()).toBe(1);
  });

  it("pans with the wheel and a drag, zooms at the pointer with Ctrl and the wheel, and with a pinch", () => {
    size(1200, 900);
    const { container } = render(<FlowCanvas root={route} aria-label="Route" />);
    const canvas = container.querySelector("[data-flow-canvas]") as HTMLElement;
    const [x0, y0] = translate();
    expect(wheel(canvas, { deltaX: 30, deltaY: 50 }).defaultPrevented).toBe(true);
    expect(translate()).toEqual([x0! - 30, y0! - 50]);
    wheel(canvas, { deltaY: 2, deltaMode: 1 });
    expect(translate()[1]).toBe(y0! - 50 - 32);
    wheel(canvas, { deltaY: 1, deltaMode: 2 });
    expect(translate()[1]).toBe(y0! - 50 - 32 - 900);
    wheel(canvas, { deltaY: -10, ctrlKey: true, clientX: 600, clientY: 450 });
    expect(scale()).toBeCloseTo(Math.exp(0.1));
    wheel(canvas, { deltaY: 10, metaKey: true, clientX: 600, clientY: 450 });
    expect(scale()).toBeCloseTo(1);
    wheel(canvas, { deltaY: -10, ctrlKey: true, clientX: 600, clientY: 450 });
    const [x1, y1] = translate();
    const k1 = scale();
    fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, pointerType: "mouse", clientX: 100, clientY: 100 });
    fireEvent.pointerMove(canvas, { pointerId: 1, pointerType: "mouse", clientX: 101, clientY: 101 });
    expect(translate()).toEqual([x1, y1]);
    fireEvent.pointerMove(canvas, { pointerId: 1, pointerType: "mouse", clientX: 160, clientY: 140 });
    expect(canvas).toHaveAttribute("data-dragging");
    expect(translate()).toEqual([x1! + 60, y1! + 40]);
    fireEvent.pointerUp(canvas, { pointerId: 1, pointerType: "mouse", clientX: 160, clientY: 140 });
    expect(canvas).not.toHaveAttribute("data-dragging");
    fireEvent.pointerDown(canvas, { pointerId: 2, pointerType: "touch", clientX: 500, clientY: 400 });
    fireEvent.pointerDown(canvas, { pointerId: 3, pointerType: "touch", clientX: 600, clientY: 400 });
    fireEvent.pointerMove(canvas, { pointerId: 3, pointerType: "touch", clientX: 700, clientY: 400 });
    expect(scale()).toBeCloseTo(Math.min(1.5, k1 * 2));
    fireEvent.pointerUp(canvas, { pointerId: 3, pointerType: "touch" });
    fireEvent.pointerMove(canvas, { pointerId: 2, pointerType: "touch", clientX: 520, clientY: 400 });
    fireEvent.pointerCancel(canvas, { pointerId: 2, pointerType: "touch" });
    fireEvent.pointerMove(canvas, { pointerId: 2, pointerType: "touch", clientX: 900, clientY: 400 });
    fireEvent.pointerDown(canvas, { pointerId: 4, button: 2, pointerType: "mouse", clientX: 10, clientY: 10 });
    fireEvent.pointerUp(canvas, { pointerId: 4, pointerType: "mouse" });
  });

  it("moves the view to show a step selected out of sight", async () => {
    size(320, 240);
    const { rerender } = render(<FlowCanvas root={route} aria-label="Route" selected={null} />);
    while (scale() < 1) await userEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    const [x, y] = translate();
    rerender(<FlowCanvas root={route} aria-label="Route" selected="mini" />);
    expect(scale()).toBe(1);
    const [x2, y2] = translate();
    expect(x2).toBeLessThan(x!);
    expect(y2).toBeLessThan(y!);
    wheel(document.querySelector("[data-flow-canvas]") as HTMLElement, { deltaX: 400 });
    const panned = transform();
    rerender(<FlowCanvas root={{ ...route }} aria-label="Route" selected="mini" />);
    expect(transform()).toBe(panned);
    rerender(<FlowCanvas root={route} aria-label="Route" selected="start" />);
    expect(translate()[0]).toBeGreaterThan(x2! - 400);
    rerender(<FlowCanvas root={route} aria-label="Route" selected="missing" />);
  });

  it("speaks the canvas in the provider's language", () => {
    render(
      <LocaleProvider locale="de" messages={de}>
        <FlowCanvas root={route} aria-label="Route" background="plain" addMenu={() => null} />
      </LocaleProvider>,
    );
    expect(screen.getByRole("button", { name: "Vergrößern" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "An Ansicht anpassen" })).toBeInTheDocument();
    expect(screen.getByRole("tree")).toHaveAccessibleDescription(/Pfeiltasten/);
    expect(screen.getByRole("treeitem", { name: "Add a fallback to claude-opus-4-1" })).toBeInTheDocument();
  });

  it("never fits below 67%: a route wider than the view keeps its start in the middle", () => {
    size(400, 300);
    render(<FlowCanvas root={route} aria-label="Route" />);
    expect(scale()).toBe(0.67);
    const [x] = translate();
    expect(x).toBe(64 + (400 - 64) / 2);
  });

  it("keeps a traced request's steps and lines strong, fades the rest and shows each step's badge", () => {
    const traced: FlowCanvasNode = {
      ...route,
      children: [
        {
          id: "root",
          node: {
            ...route.children![0]!.node!,
            children: route.children![0]!.node!.children!.map((e) => (e.id === "plan-then" ? { ...e, node: { ...e.node!, badge: "Try 1" } } : e)),
          },
        },
      ],
    };
    const { container } = render(<FlowCanvas root={traced} aria-label="Route" trace={["start", "plan", "opus"]} />);
    const items = screen.getAllByRole("treeitem");
    expect(items.map((i) => i.getAttribute("data-trace"))).toEqual(["on", "on", "on", "off"]);
    expect(items[2]).toHaveTextContent("Try 1");
    expect(items[3]).toHaveClass("shadow-ring");
    expect(within(items[3]!).getByText("gpt-5-mini")).toHaveClass("text-fg-secondary");
    expect(within(items[2]!).getByText("claude-opus-4-1")).toHaveClass("text-fg");
    expect(items[3]).toHaveTextContent("Model removed");
    const stroke = (to: string) => container.querySelector(`[data-flow-wire="wire:${to}"]`)!;
    expect(stroke("opus")).toHaveAttribute("stroke", "var(--k-text)");
    expect(stroke("opus")).toHaveAttribute("stroke-width", "2");
    expect(stroke("mini")).toHaveAttribute("stroke", "var(--k-line)");
    expect(container.querySelector('[data-flow-label="mini"]')).toHaveClass("text-fg-tertiary");
    expect(container.querySelector('[data-flow-label="opus"]')).toHaveClass("text-fg-secondary");
  });

  it("colors a tried path by its outcome, rings each tried step and runs the line while the request is on its way", () => {
    const tried: FlowCanvasNode = {
      ...route,
      children: [
        {
          id: "root",
          node: {
            ...route.children![0]!.node!,
            children: [
              {
                id: "plan-then",
                label: "Then",
                tone: "danger",
                node: {
                  id: "opus",
                  kind: "Model",
                  title: "claude-opus-4-1",
                  terminal: true,
                  status: { tone: "danger", label: "529 Overloaded" },
                  children: [
                    {
                      id: "opus-fb",
                      label: "Fallback",
                      variant: "fallback",
                      tone: "success",
                      node: { id: "sonnet", kind: "Model", title: "claude-sonnet-4-5", terminal: true, status: { tone: "success", label: "200 OK" } },
                    },
                  ],
                },
              },
              {
                id: "plan-else",
                label: "Else",
                tone: "warning",
                node: { id: "mini", kind: "Model", title: "gpt-5-mini", terminal: true, status: { tone: "neutral", label: "Skipped" } },
              },
            ],
          },
        },
      ],
    };
    const trace = ["start", "plan", "opus", "sonnet"];
    const { container, rerender } = render(<FlowCanvas root={tried} aria-label="Route" trace={trace} busy />);
    const wire = (to: string) => container.querySelector(`[data-flow-wire="wire:${to}"]`)!;
    expect(wire("opus")).toHaveAttribute("stroke", "var(--k-danger-text)");
    expect(wire("sonnet")).toHaveAttribute("stroke", "var(--k-success-text)");
    expect(wire("mini")).toHaveAttribute("stroke", "var(--k-line)");
    expect(wire("plan")).toHaveAttribute("data-flowing");
    expect(wire("plan")).toHaveClass("animate-flow-wire");
    expect(wire("plan")).toHaveAttribute("stroke-dasharray", "4 5");
    expect(wire("opus")).not.toHaveAttribute("data-flowing");
    const order = [...container.querySelectorAll("[data-flow-wire]")].map((w) => w.getAttribute("data-flow-wire"));
    expect(order.indexOf("wire:mini")).toBeLessThan(order.indexOf("wire:opus"));
    expect(screen.getByRole("tree")).toHaveAttribute("aria-busy", "true");
    const item = (name: RegExp) => screen.getByRole("treeitem", { name });
    expect(item(/claude-opus-4-1/)).toHaveClass("ring-invalid");
    expect(item(/claude-sonnet-4-5/)).toHaveClass("ring-success");
    expect(item(/claude-sonnet-4-5/)).toHaveTextContent("200 OK");
    expect(item(/gpt-5-mini/)).not.toHaveClass("ring-invalid");
    expect(item(/gpt-5-mini/)).toHaveTextContent("Skipped");

    rerender(<FlowCanvas root={tried} aria-label="Route" trace={trace} />);
    expect(wire("plan")).not.toHaveAttribute("data-flowing");
    expect(wire("plan")).not.toHaveAttribute("stroke-dasharray");
    expect(screen.getByRole("tree")).not.toHaveAttribute("aria-busy");
  });

  it("rings a step whose status warns", () => {
    render(<FlowCanvas root={{ id: "start", kind: "Start", title: "fast", status: { tone: "warning", label: "429 Rate limited" } }} aria-label="Route" />);
    expect(screen.getByRole("treeitem", { name: /fast/ })).toHaveClass("ring-warning");
  });
});
