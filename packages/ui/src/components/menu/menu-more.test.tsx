import { describe, expect, it, vi } from "vitest";
import { createRef } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ContextMenu, Menu, Menubar } from "./menu";

describe("Menu parts", () => {
  it("checks items, chooses radios, opens a submenu and shows a header", async () => {
    const onChecked = vi.fn();
    const onRadio = vi.fn();
    const onSub = vi.fn();
    const ref = createRef<HTMLDivElement>();
    render(
      <Menu>
        <Menu.Trigger>
          <button>Account</button>
        </Menu.Trigger>
        <Menu.Content ref={ref} header={<Menu.Header name="Ada Okafor" email="ada@example.com" avatar={<span>A</span>} />}>
          <Menu.CheckboxItem onCheckedChange={onChecked}>Compact rows</Menu.CheckboxItem>
          <Menu.Separator />
          <Menu.Group label="Theme">
            <Menu.RadioGroup defaultValue="system" onValueChange={onRadio}>
              <Menu.RadioItem value="system">System</Menu.RadioItem>
              <Menu.RadioItem value="dark">Dark</Menu.RadioItem>
            </Menu.RadioGroup>
          </Menu.Group>
          <Menu.Sub onOpenChange={onSub}>
            <Menu.SubTrigger>Language</Menu.SubTrigger>
            <Menu.SubContent>
              <Menu.Item>Deutsch</Menu.Item>
            </Menu.SubContent>
          </Menu.Sub>
        </Menu.Content>
      </Menu>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Account" }));
    expect(await screen.findByText("ada@example.com")).toBeInTheDocument();
    expect(ref.current).not.toBeNull();
    await userEvent.click(screen.getByRole("menuitemcheckbox", { name: "Compact rows" }));
    expect(onChecked).toHaveBeenCalledWith(true);
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Dark" }));
    expect(onRadio).toHaveBeenCalledWith("dark");
    expect(screen.getByRole("separator")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "Language" }));
    expect(await screen.findByRole("menuitem", { name: "Deutsch" })).toBeInTheDocument();
    expect(onSub).toHaveBeenCalledWith(true);
  });

  it("opens a context menu on a right click", async () => {
    const onOpenChange = vi.fn();
    render(
      <ContextMenu onOpenChange={onOpenChange}>
        <ContextMenu.Trigger className="block">
          <div>Row</div>
        </ContextMenu.Trigger>
        <ContextMenu.Content header={<span>production</span>} footer={<span>Right-click actions</span>}>
          <ContextMenu.Item>Copy id</ContextMenu.Item>
        </ContextMenu.Content>
      </ContextMenu>,
    );
    fireEvent.contextMenu(screen.getByText("Row"));
    expect(await screen.findByRole("menuitem", { name: "Copy id" })).toBeInTheDocument();
    expect(screen.getByText("Right-click actions")).toBeInTheDocument();
    expect(onOpenChange).toHaveBeenCalledWith(true);
  });

  it("puts menus in a menubar", async () => {
    render(
      <Menubar aria-label="Editor">
        <Menubar.Menu>
          <Menubar.Trigger data-force="hover">File</Menubar.Trigger>
          <Menubar.Content>
            <Menubar.Item>Export</Menubar.Item>
          </Menubar.Content>
        </Menubar.Menu>
      </Menubar>,
    );
    expect(screen.getByRole("menubar", { name: "Editor" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "File" }));
    expect(await screen.findByRole("menuitem", { name: "Export" })).toBeInTheDocument();
  });
});
