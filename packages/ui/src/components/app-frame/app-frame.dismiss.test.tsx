import { describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame } from "./app-frame.js";
import { Sidebar } from "../sidebar/sidebar.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const loads = vi.hoisted(() => ({ attempts: [] as { arrive: () => void }[] }));
vi.mock("./nav-sheet.js", async (importOriginal) => {
  await new Promise<void>((arrive) => loads.attempts.push({ arrive }));
  return importOriginal();
});

await vi.importActual("./nav-sheet.js");

vi.stubGlobal("requestIdleCallback", vi.fn());
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

describe("the phone navigation of an app frame on a slow network", () => {
  it("never opens the sheet late once the person has taken the tap back with Escape, a tap outside or the toggle", async () => {
    (window as unknown as { happyDOM: { setViewport: (v: { width: number; height: number }) => void } }).happyDOM.setViewport({ width: 390, height: 844 });
    const user = userEvent.setup();
    render(<Console />);
    const toggle = within(document.querySelector<HTMLElement>("[data-phone-bar]")!).getByRole("button", { name: "Open navigation" });

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await user.click(screen.getByText("Usage"));
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    expect(loads.attempts).toHaveLength(1);
    await act(async () => loads.attempts[0]!.arrive());
    await act(() => lazyModulesSettled());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    await user.click(toggle);
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    expect(within(sheet).getByRole("link", { name: "Request logs" })).toBeInTheDocument();
    expect(loads.attempts).toHaveLength(1);
  });
});
