import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame } from "./app-frame.js";
import { Sidebar } from "../sidebar/sidebar.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const gate = vi.hoisted(() => {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open, asked: 0 };
});
vi.mock("./nav-sheet.js", async (importOriginal) => {
  gate.asked++;
  await gate.promise;
  return importOriginal();
});

await vi.importActual("./nav-sheet.js");

const idle: (() => void)[] = [];
vi.stubGlobal("requestIdleCallback", (run: () => void) => idle.push(run));
vi.stubGlobal("cancelIdleCallback", vi.fn());
afterEach(() => void (idle.length = 0));

function Console() {
  return (
    <Sidebar.Provider shortcut={false}>
      <AppFrame
        brandHref="/"
        sidebar={
          <Sidebar aria-label="Console" resizable={false}>
            <Sidebar.Content>
              <Sidebar.Group>
                <Sidebar.Item href="/logs">Request logs</Sidebar.Item>
              </Sidebar.Group>
            </Sidebar.Content>
          </Sidebar>
        }
      >
        <p>Usage</p>
      </AppFrame>
    </Sidebar.Provider>
  );
}

describe("the phone navigation of an app frame", () => {
  it("draws the page without the sheet's code, opens on a tap before the code arrives, and loads it once the page is idle", async () => {
    (window as unknown as { happyDOM: { setViewport: (v: { width: number; height: number }) => void } }).happyDOM.setViewport({ width: 390, height: 844 });
    const user = userEvent.setup();
    render(<Console />);
    await act(() => lazyModulesSettled());
    expect(screen.getByText("Usage")).toBeInTheDocument();
    expect(gate.asked).toBe(0);
    expect(idle).toHaveLength(1);

    const toggle = within(document.querySelector<HTMLElement>("[data-phone-bar]")!).getByRole("button", { name: "Open navigation" });
    await user.click(toggle);
    expect(gate.asked).toBe(1);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.queryByRole("dialog")).toBeNull();

    gate.open();
    await act(() => lazyModulesSettled());
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    expect(within(sheet).getByRole("link", { name: "Request logs" })).toBeInTheDocument();

    await act(async () => idle.forEach((run) => run()));
    expect(gate.asked).toBe(1);
  });
});
