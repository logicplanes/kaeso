import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { forwardRef } from "react";
import { LinkProvider, type LinkComponentProps } from "../../utils/link-provider.js";
import { Menu } from "../menu/menu.js";
import { KAESO_SIDEBAR_VARIANTS, Sidebar, useSidebar } from "./sidebar.js";

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
  it("clips the column while it animates without a scroll position of its own, so focusing a link inside never shifts the sidebar sideways", () => {
    const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
    const motion = css.slice(css.indexOf("@utility sidebar-motion {"), css.indexOf("}", css.indexOf("@utility sidebar-motion {")));
    expect(motion).toContain("overflow: clip;");
    expect(motion).not.toMatch(/overflow: (hidden|auto|scroll)/);
    render(
      <Sidebar.Provider>
        <Sidebar aria-label="Console" />
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("navigation", { name: "Console" }).closest(".sidebar-motion")).not.toBeNull();
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
    fireEvent.click(screen.getByRole("button", { name: "Northwind Labs", description: "Switch or open settings" }));
    await waitFor(() => expect(screen.getByRole("menuitem", { name: "Organization settings" })).toBeInTheDocument());
  });
  it("shows a caption under the switcher's label in the caption's tone, and reads it with the label", () => {
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
    const button = screen.getByRole("button", { name: "Northwind Labs Deletion on 20 Oct" });
    expect(button).toHaveAccessibleDescription("Switch or open settings");
    expect(button).toHaveTextContent("Northwind LabsDeletion on 20 Oct");
    expect(screen.getByText("Deletion on 20 Oct")).toHaveClass(KAESO_SIDEBAR_VARIANTS.caption.danger.classes);
  });
  it("is named by its label and caption and described by what opening it does, the caption in the secondary tone by default", () => {
    const { rerender } = render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar.Switcher avatar={null} label="Northwind Labs">
          <Menu.Item>Organization settings</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    const button = screen.getByRole("button", { name: "Northwind Labs" });
    expect(button).toHaveAccessibleDescription("Switch or open settings");
    expect(button).not.toHaveAttribute("aria-label");
    expect(button).toHaveTextContent("Northwind Labs");
    rerender(
      <Sidebar.Provider shortcut={false}>
        <Sidebar.Switcher avatar={null} label="Northwind Labs" caption="Trial ends on 2 Nov">
          <Menu.Item>Organization settings</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("button", { name: "Northwind Labs Trial ends on 2 Nov" })).toHaveAccessibleDescription("Switch or open settings");
    expect(screen.getByText("Trial ends on 2 Nov")).toHaveClass(KAESO_SIDEBAR_VARIANTS.caption.secondary.classes);
  });
  it("keeps the label and the caption in its name in rail mode", () => {
    render(
      <Sidebar.Provider shortcut={false} defaultRail>
        <Sidebar.Switcher avatar={null} label="Northwind Labs" caption="Deletion on 20 Oct" captionTone="danger">
          <Menu.Item>Organization settings</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    const button = screen.getByRole("button", { name: "Northwind Labs Deletion on 20 Oct" });
    expect(button).toHaveAccessibleDescription("Switch or open settings");
    expect(screen.getByText("Deletion on 20 Oct")).toHaveClass("sr-only");
  });
  it("takes what opening the switcher does from aria-label, and is named by it while it has no label", () => {
    const { rerender } = render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar.Switcher avatar={null} label="Ada Okafor" aria-label="Account and sign out">
          <Menu.Item>Sign out</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("button", { name: "Ada Okafor" })).toHaveAccessibleDescription("Account and sign out");
    rerender(
      <Sidebar.Provider shortcut={false}>
        <Sidebar.Switcher avatar={null} label="" aria-label="Account and sign out">
          <Menu.Item>Sign out</Menu.Item>
        </Sidebar.Switcher>
      </Sidebar.Provider>,
    );
    expect(screen.getByRole("button", { name: "Account and sign out" })).not.toHaveAttribute("aria-describedby");
  });
  it("heads the items with the place, which opens its menu below", async () => {
    render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Content>
            <Sidebar.BackLink href="/orgs/northwind">Northwind Labs</Sidebar.BackLink>
            <Sidebar.Place avatar={<span data-testid="picture" />} label="Checkout assistant" aria-label="Switch project">
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
    const place = screen.getByRole("button", { name: "Checkout assistant", description: "Switch project" });
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
