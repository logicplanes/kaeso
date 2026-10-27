import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ContextMenu, Menu } from "./menu.js";

describe("Menu items for a screen reader", () => {
  it("names an item by its label alone and describes it by its hint and shortcut", async () => {
    render(
      <Menu open>
        <Menu.Trigger>
          <button type="button">Key actions</button>
        </Menu.Trigger>
        <Menu.Content>
          <Menu.Item shortcut={["e"]}>Rename</Menu.Item>
          <Menu.Item hint="3 keys">Move to project</Menu.Item>
          <Menu.Item>Copy key id</Menu.Item>
        </Menu.Content>
      </Menu>,
    );
    const rename = await screen.findByRole("menuitem", { name: "Rename" });
    expect(rename).toHaveAccessibleDescription("E");
    expect(screen.getByRole("menuitem", { name: "Move to project" })).toHaveAccessibleDescription("3 keys");
    expect(screen.getByRole("menuitem", { name: "Copy key id" })).not.toHaveAttribute("aria-describedby");
  });

  it("keeps a footer outside the menu, and the menu itself is the one focus stop", async () => {
    render(
      <Menu>
        <Menu.Trigger>
          <button type="button">Account</button>
        </Menu.Trigger>
        <Menu.Content footer="Kaeso · self-hosted">
          <Menu.Item>Account settings</Menu.Item>
        </Menu.Content>
      </Menu>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Account" }));
    const menu = await screen.findByRole("menu");
    expect(menu).not.toHaveTextContent("Kaeso · self-hosted");
    expect(screen.getByText("Kaeso · self-hosted")).toBeInTheDocument();
  });

  it("puts no extra focus stop around a menu with a header", async () => {
    render(
      <Menu>
        <Menu.Trigger>
          <button type="button">Account</button>
        </Menu.Trigger>
        <Menu.Content header={<Menu.Header name="Ada Okafor" email="ada@northwind.example" />}>
          <Menu.Item>Account settings</Menu.Item>
        </Menu.Content>
      </Menu>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Account" }));
    const menu = await screen.findByRole("menu");
    expect(menu).not.toHaveTextContent("ada@northwind.example");
    const stops = [...document.querySelectorAll<HTMLElement>("[tabindex='0']")].filter((el) => el.contains(menu));
    expect(stops).toHaveLength(1);
  });
});

describe("ContextMenu from the keyboard", () => {
  it("makes an area with nothing focusable in it a focus stop, and gives the focus back to it on Escape", async () => {
    render(
      <ContextMenu>
        <ContextMenu.Trigger>Right-click this request</ContextMenu.Trigger>
        <ContextMenu.Content header="req_01JAR4X2KQ">
          <ContextMenu.Item>Copy request id</ContextMenu.Item>
        </ContextMenu.Content>
      </ContextMenu>,
    );
    const area = screen.getByText("Right-click this request");
    expect(area).toHaveAttribute("tabindex", "0");
    area.focus();
    fireEvent.contextMenu(area);
    const menu = await screen.findByRole("menu");
    expect(menu).not.toHaveTextContent("req_01JAR4X2KQ");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(area).toHaveFocus();
  });

  it("leaves the focus order alone when the area holds a focusable row", () => {
    render(
      <ContextMenu>
        <ContextMenu.Trigger>
          <button type="button">POST /v1/messages</button>
        </ContextMenu.Trigger>
        <ContextMenu.Content>
          <ContextMenu.Item>Copy request id</ContextMenu.Item>
        </ContextMenu.Content>
      </ContextMenu>,
    );
    expect(screen.getByRole("button", { name: "POST /v1/messages" }).parentElement).not.toHaveAttribute("tabindex", "0");
  });
});
