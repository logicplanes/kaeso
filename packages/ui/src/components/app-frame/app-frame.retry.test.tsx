import { describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame } from "./app-frame.js";
import { Sidebar } from "../sidebar/sidebar.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const loads = vi.hoisted(() => ({ attempts: [] as { arrive: () => void; fail: () => void }[] }));
vi.mock("./nav-sheet.js", async (importOriginal) => {
  await new Promise<void>((arrive, reject) => loads.attempts.push({ arrive, fail: () => reject(new Error("Failed to fetch dynamically imported module")) }));
  return importOriginal();
});

await vi.importActual("./nav-sheet.js");

const idle: (() => void)[] = [];
vi.stubGlobal("requestIdleCallback", (run: () => void) => idle.push(run));
vi.stubGlobal("cancelIdleCallback", vi.fn());

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

describe("the phone navigation of an app frame on a flaky network", () => {
  it("tries the sheet's code again on the next tap after a failed load, never on its own, and opens once it arrives", async () => {
    (window as unknown as { happyDOM: { setViewport: (v: { width: number; height: number }) => void } }).happyDOM.setViewport({ width: 390, height: 844 });
    const user = userEvent.setup();
    render(<Console />);
    const toggle = within(document.querySelector<HTMLElement>("[data-phone-bar]")!).getByRole("button", { name: "Open navigation" });

    await act(async () => idle.forEach((run) => run()));
    expect(loads.attempts).toHaveLength(1);
    await act(async () => loads.attempts[0]!.fail());
    await act(() => lazyModulesSettled());
    expect(loads.attempts).toHaveLength(1);
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    await user.click(toggle);
    expect(loads.attempts).toHaveLength(2);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await act(async () => loads.attempts[1]!.fail());
    await act(() => lazyModulesSettled());
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(loads.attempts).toHaveLength(2);

    await user.click(toggle);
    expect(loads.attempts).toHaveLength(3);
    await act(async () => loads.attempts[2]!.arrive());
    await act(() => lazyModulesSettled());
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    expect(within(sheet).getByRole("link", { name: "Request logs" })).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-expanded", "true");
  });
});
