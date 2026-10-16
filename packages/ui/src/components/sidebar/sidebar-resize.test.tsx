import { afterEach, describe, expect, it } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sidebar, SidebarSheetContext, useSidebar } from "./sidebar";
import { icons } from "../../icons/vocabulary";

function State() {
  const { collapsed, rail, width } = useSidebar();
  return <output>{`${collapsed ? "collapsed" : "expanded"} ${rail ? "rail" : "full"} ${width}`}</output>;
}

const edge = () => screen.getByRole("separator", { name: "Resize sidebar; click to collapse" });

afterEach(() => localStorage.clear());

describe("Sidebar resizing", () => {
  it("resizes with the arrows within its limits and remembers the width", () => {
    const { unmount } = render(
      <Sidebar.Provider storageKey="console">
        <Sidebar>
          <Sidebar.Content />
        </Sidebar>
        <State />
      </Sidebar.Provider>,
    );
    fireEvent.keyDown(edge(), { key: "ArrowRight" });
    expect(edge()).toHaveAttribute("aria-valuenow", "256");
    for (let i = 0; i < 10; i++) fireEvent.keyDown(edge(), { key: "ArrowRight" });
    expect(edge()).toHaveAttribute("aria-valuenow", "280");
    for (let i = 0; i < 20; i++) fireEvent.keyDown(edge(), { key: "ArrowLeft" });
    expect(edge()).toHaveAttribute("aria-valuenow", "232");
    unmount();
    render(
      <Sidebar.Provider storageKey="console">
        <State />
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("status")).toHaveTextContent("expanded full 232");
  });

  it("drags to resize, and a click without a drag collapses", () => {
    render(
      <Sidebar.Provider>
        <Sidebar>
          <Sidebar.Content />
        </Sidebar>
        <State />
      </Sidebar.Provider>,
    );
    fireEvent.pointerMove(edge(), { clientY: 120 });
    expect(screen.getByText("Drag")).toHaveClass("font-semibold");
    fireEvent.pointerLeave(edge());
    expect(screen.queryByText("Drag")).toBeNull();
    fireEvent.pointerDown(edge(), { clientX: 248 });
    expect(document.body.style.cursor).toBe("col-resize");
    act(() => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 250 }));
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 270 }));
      window.dispatchEvent(new PointerEvent("pointerup"));
    });
    expect(screen.getByRole("status")).toHaveTextContent("expanded full 270");
    expect(document.body.style.cursor).toBe("");
    fireEvent.pointerDown(edge(), { clientX: 270 });
    act(() => window.dispatchEvent(new PointerEvent("pointerup")));
    expect(screen.getByRole("status")).toHaveTextContent("collapsed");
  });

  it("switches to the rail, where items are named by tooltips and groups lose their caption", async () => {
    render(
      <Sidebar.Provider defaultRail>
        <Sidebar>
          <Sidebar.Brand href="/">
            <Sidebar.RailToggle />
          </Sidebar.Brand>
          <Sidebar.Content>
            <Sidebar.Group label="Project">
              <Sidebar.Item icon={icons.key} href="/keys" shortcut={["g", "k"]} count={3}>
                API keys
              </Sidebar.Item>
            </Sidebar.Group>
          </Sidebar.Content>
        </Sidebar>
        <State />
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("link", { name: "API keys" })).toBeInTheDocument();
    expect(screen.queryByText("Project")).toBeNull();
    expect(screen.queryByText("3")).toBeNull();
    expect(screen.queryByRole("separator")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Expand sidebar" }));
    expect(screen.getByRole("status")).toHaveTextContent("expanded full");
    expect(screen.getByRole("button", { name: "Collapse sidebar" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText("Project")).toBeInTheDocument();
  });
});

describe("Sidebar parts", () => {
  it("collapses a group, remembering it by id", async () => {
    render(
      <Sidebar.Provider storageKey="console">
        <Sidebar.Group label="Settings" id="settings">
          <Sidebar.Item onClick={() => undefined} depth={1} trailing={<span>new</span>} leading={<span>L</span>} data-force="hover">
            Members
          </Sidebar.Item>
        </Sidebar.Group>
        <Sidebar.Group label="Plain" collapsible={false}>
          <Sidebar.Item>Usage</Sidebar.Item>
        </Sidebar.Group>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("button", { name: /Members/ })).toHaveAttribute("data-force", "hover");
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(localStorage.getItem("kaeso:console:group:settings")).toBe("false");
    expect(screen.getByText("Plain")).toBeInTheDocument();
  });

  it("toggles collapse, keeps a peeking sheet open, and closes a sheet", async () => {
    let closed = 0;
    const { rerender } = render(
      <Sidebar.Provider>
        <Sidebar.CollapseToggle />
        <State />
      </Sidebar.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(screen.getByRole("button", { name: "Expand sidebar" })).toHaveAttribute("aria-expanded", "false");
    rerender(
      <Sidebar.Provider>
        <SidebarSheetContext.Provider value={{ close: () => (closed += 1), peek: false }}>
          <Sidebar.CollapseToggle />
        </SidebarSheetContext.Provider>
      </Sidebar.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Close navigation" }));
    expect(closed).toBe(1);
    rerender(
      <Sidebar.Provider>
        <SidebarSheetContext.Provider value={{ close: () => (closed += 1), peek: true }}>
          <Sidebar.CollapseToggle />
        </SidebarSheetContext.Provider>
        <State />
      </Sidebar.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Keep sidebar open" }));
    expect(screen.getByRole("status")).toHaveTextContent("expanded");
  });

  it("shows a back link, a loading state and a brand without a link", () => {
    render(
      <Sidebar.Provider>
        <Sidebar.Brand label="Kaeso" />
        <Sidebar.BackLink href="/" />
        <Sidebar.BackLink href="/org">Back to Northwind</Sidebar.BackLink>
        <Sidebar.Loading rows={2} />
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("link", { name: "Back to app" })).toHaveAttribute("href", "/");
    expect(screen.getByRole("link", { name: "Back to Northwind" })).toBeInTheDocument();
    expect(document.querySelectorAll("[aria-busy] > div")).toHaveLength(2);
    expect(screen.getByRole("img", { name: "Kaeso" })).toBeInTheDocument();
  });

  it("gives defaults outside a provider", () => {
    render(<State />);
    expect(screen.getByRole("status")).toHaveTextContent("expanded full 248");
  });
});

describe("Sidebar focus", () => {
  it("hands focus to the expand control when it collapses with focus inside", async () => {
    render(
      <Sidebar.Provider>
        <Sidebar>
          <Sidebar.Content>
            <Sidebar.Group>
              <Sidebar.Item href="/logs">Request logs</Sidebar.Item>
            </Sidebar.Group>
          </Sidebar.Content>
        </Sidebar>
        <button data-sidebar-expand="">Expand sidebar</button>
      </Sidebar.Provider>,
    );
    screen.getByRole("link", { name: "Request logs" }).focus();
    fireEvent.keyDown(window, { key: "[" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Expand sidebar" })).toHaveFocus());
  });

  it("leaves focus alone when it was elsewhere", async () => {
    render(
      <Sidebar.Provider>
        <Sidebar>
          <Sidebar.Content />
        </Sidebar>
        <input aria-label="Search" />
        <button data-sidebar-expand="">Expand sidebar</button>
      </Sidebar.Provider>,
    );
    const outside = screen.getByRole("button", { name: "Expand sidebar" });
    document.body.focus();
    fireEvent.keyDown(window, { key: "[" });
    await new Promise((r) => requestAnimationFrame(r));
    expect(outside).not.toHaveFocus();
  });
});
