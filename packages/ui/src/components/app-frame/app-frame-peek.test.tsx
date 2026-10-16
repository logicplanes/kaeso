import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppFrame, PEEK_DELAY } from "./app-frame.js";
import { PageHeader } from "../page-header/page-header.js";
import { Sidebar, useSidebar } from "../sidebar/sidebar.js";

function wideScreen(wide: boolean) {
  vi.spyOn(window, "matchMedia").mockImplementation(
    (q: string) =>
      ({
        matches: wide && q.includes("min-width: 1024px"),
        media: q,
        addEventListener() {},
        removeEventListener() {},
      }) as unknown as MediaQueryList,
  );
}

function Collapse() {
  const { setCollapsed } = useSidebar();
  return <button onClick={() => setCollapsed(true)}>Collapse now</button>;
}

function Shell({ fill }: { fill?: "screen" | "parent" }) {
  return (
    <Sidebar.Provider shortcut={false}>
      <AppFrame
        {...(fill ? { fill } : {})}
        corner={<span>Help</span>}
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

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("AppFrame on a wide screen with the sidebar collapsed", () => {
  it("peeks at the sidebar after resting on the menu button, and hides it again on leaving", async () => {
    wideScreen(true);
    render(<Shell />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const header = screen.getByRole("heading", { level: 1, name: "Usage" }).closest("header")!;
    const menu = within(header).getByRole("button", { name: "Expand sidebar" });
    vi.useFakeTimers();
    fireEvent.pointerEnter(menu);
    act(() => vi.advanceTimersByTime(PEEK_DELAY));
    vi.useRealTimers();
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    fireEvent.pointerEnter(sheet);
    fireEvent.pointerLeave(sheet);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Navigation" })).toBeNull());
    fireEvent.pointerEnter(menu);
    fireEvent.pointerLeave(menu);
    await userEvent.click(menu);
    expect(within(header).queryByRole("button", { name: "Expand sidebar" })).toBeNull();
  });

  it("closes the sheet when a link in it is followed, and fills its parent when embedded", async () => {
    wideScreen(false);
    render(<Shell fill="parent" />);
    expect(screen.queryByRole("main")).toBeNull();
    expect(screen.getByText("Help")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const header = screen.getByRole("heading", { level: 2, name: "Usage" }).closest("header")!;
    await userEvent.click(within(header).getByRole("button", { name: "Expand sidebar" }));
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    fireEvent.pointerLeave(sheet);
    await userEvent.click(within(sheet).getByRole("link", { name: "Request logs" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Navigation" })).toBeNull());
  });
});
