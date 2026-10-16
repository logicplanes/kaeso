import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { List } from "./list";

const checkboxGate = vi.hoisted(() => {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open };
});
vi.mock("../checkbox/checkbox", async (importOriginal) => {
  await checkboxGate.promise;
  return importOriginal();
});
const items = ["checkout-web", "checkout-worker", "ci-runner"].map((n) => ({ id: n, title: n }));

describe("List, loading what it needs", () => {
  const scroller = () => (document.scrollingElement ?? document.documentElement) as HTMLElement;
  beforeAll(() => {
    Object.defineProperty(scroller(), "offsetHeight", { configurable: true, value: 800 });
    Object.defineProperty(scroller(), "offsetWidth", { configurable: true, value: 1280 });
  });
  afterAll(() => {
    delete (scroller() as unknown as Record<string, unknown>).offsetHeight;
    delete (scroller() as unknown as Record<string, unknown>).offsetWidth;
  });

  it("draws a long list's first rows at once and keeps its height when the virtualizer arrives", async () => {
    const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}` }));
    render(<List aria-label="Keys" items={many} />);
    const body = () => screen.getByRole("grid", { name: "Keys" }).firstElementChild as HTMLElement;
    expect(body().style.height).toBe(`${300 * 44}px`);
    const first = screen.getAllByRole("gridcell").length;
    expect(first).toBeGreaterThan(0);
    expect(first).toBeLessThan(300);
    expect(body().children[1]).toHaveStyle({ transform: "translateY(44px)" });
    await waitFor(() => expect(screen.getAllByRole("gridcell").length).not.toBe(first), { timeout: 10_000 });
    expect(body().style.height).toBe(`${300 * 44}px`);
    expect(body().children[1]).toHaveStyle({ transform: "translateY(44px)" });
  }, 20_000);
  it("moves through a virtualized list with the keyboard", async () => {
    const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}` }));
    render(<List aria-label="Keys" items={many} />);
    screen.getAllByRole("gridcell")[0]!.focus();
    await userEvent.keyboard("j");
    await waitFor(() => expect(document.activeElement).toHaveAttribute("data-index", "1"));
  }, 20_000);
  it("stands a box in for the checkbox until it loads, which toggles the row the same way", async () => {
    const user = userEvent.setup();
    render(<List aria-label="Keys" items={items} selectable selectionBar={false} />);
    const box = (name: string) => screen.getByRole("checkbox", { name: `Select ${name}` });
    const cell = (i: number) => screen.getAllByRole("gridcell")[i];
    const web = box("checkout-web");
    await user.click(web);
    expect(cell(0)).toHaveAttribute("aria-selected", "true");
    expect(web).toHaveAttribute("aria-checked", "true");
    const worker = box("checkout-worker");
    await user.pointer({ keys: "[MouseLeft>]", target: worker });
    checkboxGate.open();
    await act(async () => {
      await import("../checkbox/checkbox");
      await new Promise((resolve) => setTimeout(resolve));
    });
    expect(box("checkout-web")).not.toBe(web);
    expect(box("checkout-worker")).toBe(worker);
    await user.pointer({ keys: "[/MouseLeft]", target: box("checkout-worker") });
    expect(cell(1)).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(box("checkout-worker")).not.toBe(worker));
    expect(box("checkout-web")).toHaveAttribute("aria-checked", "true");
    expect(box("checkout-worker")).toHaveAttribute("aria-checked", "true");
    expect(box("ci-runner")).toHaveAttribute("aria-checked", "false");
    expect(cell(0)).toHaveAttribute("aria-selected", "true");
    expect(cell(2)).toHaveAttribute("aria-selected", "false");
    await user.click(box("checkout-web"));
    expect(cell(0)).toHaveAttribute("aria-selected", "false");
  }, 20_000);
});
