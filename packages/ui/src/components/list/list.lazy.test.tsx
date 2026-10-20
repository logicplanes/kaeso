import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { List } from "./list.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const checkboxGate = vi.hoisted(() => {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open };
});
vi.mock("../checkbox/checkbox.js", async (importOriginal) => {
  await checkboxGate.promise;
  return importOriginal();
});
await vi.importActual("../checkbox/checkbox.js");

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
    await act(() => lazyModulesSettled());
    expect(screen.getAllByRole("gridcell").length).not.toBe(first);
    expect(body().style.height).toBe(`${300 * 44}px`);
    expect(body().children[1]).toHaveStyle({ transform: "translateY(44px)" });
  });
  it("shows the rows in view of a long list that sits below other content", async () => {
    const page = scroller();
    const above = 3000;
    const rect = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const top = this.parentElement?.getAttribute("role") === "grid" && this.getAttribute("role") === "presentation" ? above - page.scrollTop : 0;
      return { top, bottom: top, left: 0, right: 0, width: 0, height: 0, x: 0, y: top, toJSON: () => ({}) } as DOMRect;
    });
    try {
      const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}` }));
      render(<List aria-label="Keys" items={many} />);
      await act(() => lazyModulesSettled());
      page.scrollTop = above + 200 * 44;
      fireEvent.scroll(page);
      const row = screen.getByText("key-200").closest<HTMLElement>("[style*=translateY]");
      expect(row).toHaveStyle({ transform: `translateY(${200 * 44}px)` });
    } finally {
      rect.mockRestore();
      page.scrollTop = 0;
    }
  });
  it("moves through a virtualized list with the keyboard", async () => {
    const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}` }));
    render(<List aria-label="Keys" items={many} />);
    screen.getAllByRole("gridcell")[0]!.focus();
    await userEvent.keyboard("j");
    await waitFor(() => expect(document.activeElement).toHaveAttribute("data-index", "1"));
  });
  it("moves through a virtualized narrow list, whose rows the virtualizer measures, with the arrow keys", async () => {
    const rects = vi.spyOn(HTMLElement.prototype, "getClientRects").mockImplementation(() => [new DOMRect(0, 0, 375, 44)] as unknown as DOMRectList);
    const width = vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(375);
    try {
      const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}`, date: "Oct 2026" }));
      render(<List aria-label="Keys" items={many} />);
      await act(() => lazyModulesSettled());
      await waitFor(() => expect(document.querySelector("[data-v-index]")).not.toBeNull());
      const first = screen.getAllByRole("gridcell")[0]!;
      const start = Number(first.dataset.index);
      first.focus();
      await userEvent.keyboard("{ArrowDown}");
      await waitFor(() => expect(document.activeElement).toHaveAttribute("data-index", String(start + 1)));
      await userEvent.keyboard("{ArrowDown}");
      await waitFor(() => expect(document.activeElement).toHaveAttribute("data-index", String(start + 2)));
      expect(document.activeElement).toHaveAttribute("role", "gridcell");
    } finally {
      rects.mockRestore();
      width.mockRestore();
    }
  });
  it("selects with Space and x on a row while the checkbox is still on its way, and shows it on the stand-in box", async () => {
    const user = userEvent.setup();
    render(<List aria-label="Keys" items={items} selectable selectionBar={false} />);
    const box = (name: string) => screen.getByRole("checkbox", { name: `Select ${name}` });
    const cell = (i: number) => screen.getAllByRole("gridcell")[i]!;
    expect(box("checkout-web")).not.toHaveAttribute("data-unchecked");
    cell(0).focus();
    await user.keyboard(" ");
    expect(cell(0)).toHaveAttribute("aria-selected", "true");
    expect(box("checkout-web")).toHaveAttribute("aria-checked", "true");
    await user.keyboard("j");
    await user.keyboard("x");
    expect(cell(1)).toHaveAttribute("aria-selected", "true");
    expect(box("checkout-worker")).toHaveAttribute("aria-checked", "true");
    await user.keyboard("x");
    expect(cell(1)).toHaveAttribute("aria-selected", "false");
    expect(box("checkout-worker")).toHaveAttribute("aria-checked", "false");
  });
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
    await act(() => lazyModulesSettled());
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
  });
});
