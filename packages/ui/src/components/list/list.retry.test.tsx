import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { List } from "./list.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const loads = vi.hoisted(() => ({ checkbox: { attempts: 0, failing: 1 }, rows: { attempts: 0, failing: 1 } }));
vi.mock("../checkbox/checkbox.js", async (importOriginal) => {
  loads.checkbox.attempts++;
  if (loads.checkbox.failing-- > 0) throw new TypeError("Importing a module script failed.");
  return importOriginal();
});
vi.mock("./virtual-rows.js", async (importOriginal) => {
  loads.rows.attempts++;
  if (loads.rows.failing-- > 0) throw new TypeError("Importing a module script failed.");
  return importOriginal();
});

await vi.importActual("../checkbox/checkbox.js");
await vi.importActual("./virtual-rows.js");

describe("a list whose code failed to load", () => {
  const scroller = () => (document.scrollingElement ?? document.documentElement) as HTMLElement;
  beforeAll(() => {
    Object.defineProperty(scroller(), "offsetHeight", { configurable: true, value: 800 });
    Object.defineProperty(scroller(), "offsetWidth", { configurable: true, value: 1280 });
  });
  afterAll(() => {
    delete (scroller() as unknown as Record<string, unknown>).offsetHeight;
    delete (scroller() as unknown as Record<string, unknown>).offsetWidth;
  });

  it("keeps a long list's first rows, and loads the rest of it again when the person reaches for the list", async () => {
    const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}` }));
    render(<List aria-label="Keys" items={many} />);
    await act(() => lazyModulesSettled());
    expect(loads.rows.attempts).toBe(1);
    const first = screen.getAllByRole("gridcell").length;
    await act(() => lazyModulesSettled());
    expect(loads.rows.attempts).toBe(1);

    fireEvent.wheel(screen.getByRole("grid", { name: "Keys" }));
    await act(() => lazyModulesSettled());
    expect(loads.rows.attempts).toBe(2);
    expect(screen.getAllByRole("gridcell").length).not.toBe(first);
  });

  it("keeps a row's checkbox working, and loads the checkbox again when the pointer comes to it", async () => {
    render(<List aria-label="Keys" selectable items={[{ id: "checkout-web", title: "checkout-web" }]} />);
    await act(() => lazyModulesSettled());
    expect(loads.checkbox.attempts).toBe(1);
    const box = screen.getByRole("checkbox", { name: /checkout-web/ });
    expect(box).not.toHaveAttribute("data-unchecked");

    fireEvent.pointerEnter(box);
    fireEvent.click(box);
    expect(screen.getByRole("checkbox", { name: /checkout-web/ })).toHaveAttribute("aria-checked", "true");
    fireEvent.pointerLeave(box);
    await act(() => lazyModulesSettled());
    expect(loads.checkbox.attempts).toBe(2);
    expect(screen.getByRole("checkbox", { name: /checkout-web/ })).toHaveAttribute("data-checked");
  });
});
