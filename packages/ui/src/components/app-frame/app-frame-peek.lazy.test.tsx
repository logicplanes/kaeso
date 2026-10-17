import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame, PEEK_DELAY } from "./app-frame.js";
import { PageHeader } from "../page-header/page-header.js";
import { Sidebar, useSidebar } from "../sidebar/sidebar.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

const loads = vi.hoisted(() => ({ attempts: [] as { arrive: () => void }[] }));
vi.mock("./nav-sheet.js", async (importOriginal) => {
  await new Promise<void>((arrive) => loads.attempts.push({ arrive }));
  return importOriginal();
});

await vi.importActual("./nav-sheet.js");

vi.stubGlobal("requestIdleCallback", vi.fn());
vi.stubGlobal("cancelIdleCallback", vi.fn());

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function Collapse() {
  const { setCollapsed } = useSidebar();
  return <button onClick={() => setCollapsed(true)}>Collapse now</button>;
}

function Shell() {
  return (
    <Sidebar.Provider shortcut={false}>
      <AppFrame
        sidebar={
          <Sidebar aria-label="Console">
            <Sidebar.Content>
              <Sidebar.Group>
                <Sidebar.Item href="/logs">Request logs</Sidebar.Item>
              </Sidebar.Group>
            </Sidebar.Content>
          </Sidebar>
        }
      >
        <PageHeader start={<AppFrame.MenuButton />} title="Usage" actions={<Collapse />} />
      </AppFrame>
    </Sidebar.Provider>
  );
}

function restOn(button: HTMLElement) {
  vi.useFakeTimers();
  fireEvent.pointerEnter(button);
  act(() => vi.advanceTimersByTime(PEEK_DELAY));
  vi.useRealTimers();
}

describe("the collapsed sidebar's peek on a slow network", () => {
  it("never peeks once the pointer has left before the sheet's code arrived, and peeks on the next rest", async () => {
    vi.spyOn(window, "matchMedia").mockImplementation(
      (q: string) => ({ matches: q.includes("min-width: 1024px"), media: q, addEventListener() {}, removeEventListener() {} }) as unknown as MediaQueryList,
    );
    render(<Shell />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const header = screen.getByRole("heading", { level: 1, name: "Usage" }).closest("header")!;
    const menu = within(header).getByRole("button", { name: "Expand sidebar" });

    restOn(menu);
    fireEvent.pointerLeave(menu);
    await vi.waitFor(() => expect(loads.attempts).toHaveLength(1));

    await act(async () => loads.attempts[0]!.arrive());
    await act(() => lazyModulesSettled());
    expect(screen.queryByRole("dialog", { name: "Navigation" })).toBeNull();

    restOn(menu);
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    expect(within(sheet).getByRole("link", { name: "Request logs" })).toBeInTheDocument();
    fireEvent.pointerLeave(menu);
    expect(screen.getByRole("dialog", { name: "Navigation" })).toBeInTheDocument();
    expect(loads.attempts).toHaveLength(1);
  });
});
