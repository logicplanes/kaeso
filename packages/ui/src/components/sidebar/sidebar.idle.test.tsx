import { afterEach, describe, expect, it, vi } from "vitest";
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

const idle: (() => void)[] = [];
vi.stubGlobal("requestIdleCallback", (run: () => void) => idle.push(run));
vi.stubGlobal("cancelIdleCallback", vi.fn());
afterEach(() => void (idle.length = 0));

function Console() {
  return (
    <Sidebar.Provider shortcut={false}>
      <Sidebar aria-label="Console" resizable={false}>
        <Sidebar.Footer>
          <Sidebar.Switcher avatar={null} label="Northwind Labs">
            <Menu.Item>Organization settings</Menu.Item>
          </Sidebar.Switcher>
        </Sidebar.Footer>
      </Sidebar>
    </Sidebar.Provider>
  );
}

describe("a sidebar menu on a slow machine", () => {
  it("loads its menu once the page is idle, without a press, and then opens on the press itself", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<Console />);
    await act(() => lazyModulesSettled());
    expect(gate.asked).toBe(false);
    expect(idle).toHaveLength(1);

    await act(async () => idle.forEach((run) => run()));
    await vi.waitFor(() => expect(gate.asked).toBe(true));
    gate.open();
    await act(() => lazyModulesSettled());

    const trigger = screen.getByRole("button", { name: "Northwind Labs: switch or open settings" });
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    await user.click(trigger);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Organization settings" })).toBeInTheDocument();
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    unmount();
  });

  it("stops waiting for idle time when it leaves the page", () => {
    const { unmount } = render(<Console />);
    expect(idle).toHaveLength(1);
    unmount();
    expect(cancelIdleCallback).toHaveBeenCalledTimes(1);
  });
});
