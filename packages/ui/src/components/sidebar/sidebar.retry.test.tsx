import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sidebar } from "./sidebar.js";
import { Menu } from "../menu/menu.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const loads = vi.hoisted(() => ({ attempts: 0, failing: 2 }));
vi.mock("./sidebar-menu.js", async (importOriginal) => {
  loads.attempts++;
  if (loads.failing-- > 0) throw new TypeError("Importing a module script failed.");
  return importOriginal();
});

await vi.importActual("./sidebar-menu.js");

const idle: (() => void)[] = [];
vi.stubGlobal("requestIdleCallback", (run: () => void) => idle.push(run));
vi.stubGlobal("cancelIdleCallback", vi.fn());

describe("a sidebar menu whose code failed to load", () => {
  it("loads it again when the pointer comes back, never on its own, and opens on a press once it arrives", async () => {
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
      </Sidebar.Provider>,
    );
    const trigger = screen.getByRole("button", { name: "Northwind Labs", description: "Switch or open settings" });

    await act(async () => idle.forEach((run) => run()));
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(1);
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(1);

    fireEvent.pointerEnter(trigger);
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(2);
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menu")).toBeNull();

    await user.click(trigger);
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toBe(3);
    expect(await screen.findByRole("menuitem", { name: "Organization settings" })).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "true");
  });
});
