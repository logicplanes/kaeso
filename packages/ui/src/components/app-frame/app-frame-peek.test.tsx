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

  it("doesn't peek after a click on the menu button, however often the pointer entered it", async () => {
    wideScreen(true);
    render(<Shell />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const header = screen.getByRole("heading", { level: 1, name: "Usage" }).closest("header")!;
    const menu = within(header).getByRole("button", { name: "Expand sidebar" });
    vi.useFakeTimers();
    fireEvent.pointerEnter(menu);
    fireEvent.pointerEnter(menu);
    fireEvent.click(menu);
    act(() => vi.advanceTimersByTime(PEEK_DELAY * 2));
    vi.useRealTimers();
    await act(async () => {});
    expect(screen.queryByRole("dialog", { name: "Navigation" })).toBeNull();
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

function Full({ fill, onPick }: { fill?: "screen" | "parent"; onPick?: () => void }) {
  return (
    <Sidebar.Provider>
      <AppFrame
        {...(fill ? { fill } : {})}
        sidebar={
          <Sidebar aria-label="Console">
            <Sidebar.Brand>
              <Sidebar.CollapseToggle />
            </Sidebar.Brand>
            <Sidebar.Content>
              <Sidebar.Group>
                <Sidebar.Item href="/usage">Usage</Sidebar.Item>
                <Sidebar.Item href="/logs" active>
                  Request logs
                </Sidebar.Item>
                <Sidebar.Item onClick={onPick}>Pick a project</Sidebar.Item>
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

const nextFrame = () => act(() => new Promise<void>((r) => requestAnimationFrame(() => r())));

describe("AppFrame's peek and sheet", () => {
  it("closes a peek when the pointer leaves the menu button for the page, and never dims or blocks the page", async () => {
    wideScreen(true);
    render(<Shell />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const menu = within(screen.getByRole("heading", { level: 1 }).closest("header")!).getByRole("button", { name: "Expand sidebar" });
    vi.useFakeTimers();
    fireEvent.pointerEnter(menu);
    act(() => vi.advanceTimersByTime(PEEK_DELAY));
    vi.useRealTimers();
    await screen.findByRole("dialog", { name: "Navigation" });
    expect(document.querySelector(".bg-overlay")).toBeNull();
    fireEvent.pointerLeave(menu);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Navigation" })).toBeNull());
  });

  it("keeps a peek open while the pointer moves from the menu button into it", async () => {
    wideScreen(true);
    render(<Shell />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const menu = within(screen.getByRole("heading", { level: 1 }).closest("header")!).getByRole("button", { name: "Expand sidebar" });
    vi.useFakeTimers();
    fireEvent.pointerEnter(menu);
    act(() => vi.advanceTimersByTime(PEEK_DELAY));
    vi.useRealTimers();
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    vi.spyOn(sheet, "matches").mockImplementation((q: string) => q === ":hover");
    fireEvent.pointerLeave(menu);
    await new Promise((r) => setTimeout(r, 250));
    expect(screen.getByRole("dialog", { name: "Navigation" })).toBeInTheDocument();
  });

  it("opens an embedded frame's peek inside the frame, not over the host page", async () => {
    wideScreen(false);
    render(<Shell fill="parent" />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    await userEvent.click(within(screen.getByRole("heading", { level: 2 }).closest("header")!).getByRole("button", { name: "Expand sidebar" }));
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    expect(sheet.closest('[data-app-frame="parent"]')).not.toBeNull();
    await waitFor(() => expect(sheet).toHaveClass("absolute"));
    expect(screen.getByText("Help").parentElement).toHaveClass("absolute");
  });

  it("gives focus back to the sidebar when the menu button expands it", async () => {
    wideScreen(true);
    render(<Full />);
    await userEvent.click(screen.getByRole("button", { name: "Collapse now" }));
    const menu = within(screen.getByRole("heading", { level: 1 }).closest("header")!).getByRole("button", { name: "Expand sidebar" });
    menu.focus();
    await userEvent.keyboard("{Enter}");
    await nextFrame();
    expect(screen.getByRole("button", { name: "Collapse sidebar" })).toHaveFocus();
  });

  it("returns focus to the item that collapsed the sidebar when [ expands it again", async () => {
    wideScreen(true);
    render(<Full />);
    const usage = screen.getAllByRole("link", { name: "Usage" })[0]!;
    usage.focus();
    fireEvent.keyDown(usage, { key: "[" });
    await nextFrame();
    expect(document.activeElement).toHaveAttribute("data-sidebar-expand");
    fireEvent.keyDown(document.activeElement!, { key: "[" });
    await nextFrame();
    expect(usage).toHaveFocus();
  });

  it("opens the phone sheet on the current page, and closes it when a button item is chosen", async () => {
    wideScreen(false);
    const onPick = vi.fn();
    render(<Full onPick={onPick} />);
    await userEvent.click(within(document.querySelector<HTMLElement>("[data-phone-bar]")!).getByRole("button", { name: "Open navigation" }));
    const sheet = await screen.findByRole("dialog", { name: "Navigation" });
    await waitFor(() => expect(within(sheet).getByRole("link", { name: "Request logs" })).toHaveFocus());
    await userEvent.click(within(sheet).getByRole("button", { name: "Pick a project" }));
    expect(onPick).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Navigation" })).toBeNull());
  });
});
