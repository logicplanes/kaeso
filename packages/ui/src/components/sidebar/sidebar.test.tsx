import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { forwardRef } from "react";
import { LinkProvider, type LinkComponentProps } from "../../utils/link-provider";
import { Menu } from "../menu/menu";
import { Sidebar, useSidebar } from "./sidebar";

function State() {
  const { collapsed } = useSidebar();
  return <span data-testid="state">{collapsed ? "collapsed" : "expanded"}</span>;
}

describe("Sidebar", () => {
  it("[ collapses and expands the page's sidebar", () => {
    render(
      <Sidebar.Provider>
        <State />
      </Sidebar.Provider>,
    );
    fireEvent.keyDown(window, { key: "[" });
    expect(screen.getByTestId("state")).toHaveTextContent("collapsed");
    fireEvent.keyDown(window, { key: "[" });
    expect(screen.getByTestId("state")).toHaveTextContent("expanded");
  });
  it("ignores [ in fields and when the shortcut is off", () => {
    render(
      <Sidebar.Provider shortcut={false}>
        <State />
      </Sidebar.Provider>,
    );
    fireEvent.keyDown(window, { key: "[" });
    expect(screen.getByTestId("state")).toHaveTextContent("expanded");
  });
  it("puts the resize edge beside the column that clips while it animates, and drops it when collapsed", () => {
    render(
      <Sidebar.Provider>
        <Sidebar aria-label="Console">
          <Sidebar.Content />
        </Sidebar>
        <State />
      </Sidebar.Provider>,
    );
    const edge = screen.getByRole("separator", { name: "Resize sidebar; click to collapse" });
    expect(edge.closest(".sidebar-motion")).toBeNull();
    expect(edge.closest("nav")).toBeNull();
    fireEvent.keyDown(edge, { key: "Enter" });
    expect(screen.getByTestId("state")).toHaveTextContent("collapsed");
    expect(screen.queryByRole("separator", { name: "Resize sidebar; click to collapse" })).toBeNull();
  });
  it("marks the current page", () => {
    render(
      <Sidebar.Provider>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Content>
            <Sidebar.Group>
              <Sidebar.Item href="/logs" active>
                Request logs
              </Sidebar.Item>
              <Sidebar.Item href="/usage">Usage</Sidebar.Item>
            </Sidebar.Group>
          </Sidebar.Content>
        </Sidebar>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("link", { name: "Request logs" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Usage" })).not.toHaveAttribute("aria-current");
  });
  it("shows the wordmark as the home link and keeps the circles", () => {
    render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Brand href="/" label="Kaeso home">
            <Sidebar.CollapseToggle />
          </Sidebar.Brand>
        </Sidebar>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("link", { name: "Kaeso home" })).toHaveAttribute("href", "/");
    expect(screen.getByRole("button", { name: "Collapse sidebar" })).toBeInTheDocument();
  });
  it("opens the switcher's menu", async () => {
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
    fireEvent.click(screen.getByRole("button", { name: "Northwind Labs: switch or open settings" }));
    await waitFor(() => expect(screen.getByRole("menuitem", { name: "Organization settings" })).toBeInTheDocument());
  });
  it("shows a caption under the switcher's label in the same 36px row", () => {
    render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Footer>
            <Sidebar.Switcher avatar={null} label="Northwind Labs" caption="Deletion on 20 Oct" captionTone="danger">
              <Menu.Item>Organization settings</Menu.Item>
            </Sidebar.Switcher>
          </Sidebar.Footer>
        </Sidebar>
      </Sidebar.Provider>,
    );
    const button = screen.getByRole("button", { name: "Northwind Labs: switch or open settings" });
    expect(button).toHaveClass("h-36");
    expect(button).toHaveAccessibleDescription("Deletion on 20 Oct");
    const caption = screen.getByText("Deletion on 20 Oct");
    expect(caption).toHaveClass("truncate", "text-caption", "text-danger-text");
    expect(screen.getByText("Northwind Labs")).toHaveClass("truncate");
  });
  it("keeps the row's height without a caption, and the caption's default tone is secondary", () => {
    const { rerender } = render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar.Switcher avatar={null} label="Northwind Labs">
          <Menu.Item>Organization settings</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    const button = screen.getByRole("button", { name: "Northwind Labs: switch or open settings" });
    const classes = button.className;
    expect(button).not.toHaveAttribute("aria-describedby");
    rerender(
      <Sidebar.Provider shortcut={false}>
        <Sidebar.Switcher avatar={null} label="Northwind Labs" caption="Trial ends on 2 Nov">
          <Menu.Item>Organization settings</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("button", { name: "Northwind Labs: switch or open settings" }).className).toBe(classes);
    expect(screen.getByText("Trial ends on 2 Nov")).toHaveClass("text-fg-secondary");
  });
  it("keeps the caption as the accessible description in rail mode", () => {
    render(
      <Sidebar.Provider shortcut={false} defaultRail>
        <Sidebar.Switcher avatar={null} label="Northwind Labs" caption="Deletion on 20 Oct" captionTone="danger">
          <Menu.Item>Organization settings</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    const button = screen.getByRole("button", { name: "Northwind Labs: switch or open settings" });
    expect(button).toHaveAccessibleDescription("Deletion on 20 Oct");
    expect(screen.getByText("Deletion on 20 Oct")).toHaveClass("sr-only");
  });
  it("heads the items with the place, which opens its menu below", async () => {
    render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Content>
            <Sidebar.BackLink href="/orgs/northwind">Northwind Labs</Sidebar.BackLink>
            <Sidebar.Place avatar={<span data-testid="picture" />} label="Checkout assistant" aria-label="Checkout assistant: switch project">
              <Menu.RadioGroup value="checkout">
                <Menu.RadioItem value="checkout" leading={<span data-testid="menu-picture" />} hint="Default">
                  Checkout assistant
                </Menu.RadioItem>
                <Menu.RadioItem value="fraud">Fraud screening</Menu.RadioItem>
              </Menu.RadioGroup>
            </Sidebar.Place>
            <Sidebar.Group>
              <Sidebar.Item href="/keys">API keys</Sidebar.Item>
            </Sidebar.Group>
          </Sidebar.Content>
        </Sidebar>
      </Sidebar.Provider>,
    );
    const place = screen.getByRole("button", { name: "Checkout assistant: switch project" });
    expect(place).toHaveClass("h-36", "px-12", "gap-8", "font-semibold", "text-fg");
    expect(screen.getByTestId("picture").parentElement).toHaveClass("size-18");
    expect(screen.getByRole("link", { name: "Northwind Labs" })).toHaveClass("px-12", "gap-8");
    fireEvent.click(place);
    await waitFor(() => expect(screen.getByRole("menuitemradio", { name: /Checkout assistant/ })).toHaveAttribute("aria-checked", "true"));
    expect(screen.getByTestId("menu-picture").parentElement).toHaveClass("size-16");
  });
  it("shows a place without a menu as a link, and in the rail as its picture", () => {
    render(
      <Sidebar.Provider shortcut={false} defaultRail>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Content>
            <Sidebar.Place avatar={null} label="Checkout assistant" href="/projects/checkout" />
          </Sidebar.Content>
        </Sidebar>
      </Sidebar.Provider>,
    );
    const link = screen.getByRole("link", { name: "Checkout assistant" });
    expect(link).toHaveAttribute("href", "/projects/checkout");
    expect(link).toHaveClass("justify-center", "px-0");
  });
  it("routes menu links through LinkProvider", async () => {
    const RouterLink = forwardRef<HTMLAnchorElement, LinkComponentProps>(function RouterLink(props, ref) {
      return <a ref={ref} data-router="yes" {...props} />;
    });
    render(
      <LinkProvider component={RouterLink}>
        <Menu defaultOpen>
          <Menu.Trigger>
            <button type="button">Open</button>
          </Menu.Trigger>
          <Menu.Content>
            <Menu.LinkItem href="/settings">Settings</Menu.LinkItem>
            <Menu.LinkItem href="https://ui.kaeso.app" external>
              Documentation
            </Menu.LinkItem>
          </Menu.Content>
        </Menu>
      </LinkProvider>,
    );
    await waitFor(() => expect(screen.getByRole("menuitem", { name: "Settings" })).toHaveAttribute("data-router", "yes"));
    expect(screen.getByRole("menuitem", { name: /Documentation/ })).not.toHaveAttribute("data-router");
  });
});
