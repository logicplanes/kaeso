import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sidebar } from "./sidebar.js";
import { Menu } from "../menu/menu.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const gate = vi.hoisted(() => {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open, asked: false };
});
vi.mock("./sidebar-menu.js", async (importOriginal) => {
  gate.asked = true;
  await gate.promise;
  return importOriginal();
});

await vi.importActual("./sidebar-menu.js");

async function arrive() {
  gate.open();
  await act(() => lazyModulesSettled());
}

describe("a sidebar menu pressed before it has loaded", () => {
  it("loads only when wanted, shows open at once, cancels on Escape or a press elsewhere, and opens when it arrives", async () => {
    const user = userEvent.setup();
    render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Footer>
            <Sidebar.Switcher avatar={null} label="Northwind Labs">
              <Menu.Item>Organization settings</Menu.Item>
            </Sidebar.Switcher>
          </Sidebar.Footer>
        </Sidebar>
        <button type="button">Elsewhere</button>
      </Sidebar.Provider>,
    );
    const trigger = screen.getByRole("button", { name: "Northwind Labs", description: "Switch or open settings" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    await act(() => lazyModulesSettled());
    expect(gate.asked).toBe(false);

    await user.click(trigger);
    expect(gate.asked).toBe(true);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(trigger).toHaveAttribute("data-popup-open");
    await user.keyboard("{Escape}");
    expect(trigger).toHaveAttribute("aria-expanded", "false");

    await user.click(trigger);
    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    expect(trigger).toHaveAttribute("aria-expanded", "false");

    trigger.focus();
    await user.keyboard("{ArrowDown}");
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    await arrive();
    expect(screen.getByRole("button", { name: "Northwind Labs", description: "Switch or open settings" })).toBe(trigger);
    expect(screen.getByRole("menuitem", { name: "Organization settings" })).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    await user.click(trigger);
    expect(screen.getByRole("menuitem", { name: "Organization settings" })).toBeInTheDocument();
  });
});
