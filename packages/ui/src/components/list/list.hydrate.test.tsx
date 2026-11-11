import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot } from "react-dom/client";
import { List } from "./list.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const height = window.innerHeight;
const windowHeight = (value: number) => Object.defineProperty(window, "innerHeight", { configurable: true, value });
afterEach(() => windowHeight(height));

describe("a long List rendered on a server", () => {
  it("hydrates in a window of another height, after another list on the page loaded the virtualizer", async () => {
    const many = Array.from({ length: 300 }, (_, i) => ({ id: `key-${i}`, title: `key-${i}` }));
    const list = <List aria-label="Keys" items={many} />;
    windowHeight(2400);
    const container = document.createElement("div");
    container.innerHTML = renderToString(list);
    document.body.append(container);

    windowHeight(600);
    render(<List aria-label="Other keys" items={many} />);
    await act(() => lazyModulesSettled());
    const onRecoverableError = vi.fn();
    const root = await act(async () => hydrateRoot(container, list, { onRecoverableError }));
    expect(onRecoverableError).not.toHaveBeenCalled();
    expect(screen.getByRole("grid", { name: "Keys" })).toHaveAttribute("aria-rowcount", "300");
    act(() => root.unmount());
    container.remove();
  });
});
