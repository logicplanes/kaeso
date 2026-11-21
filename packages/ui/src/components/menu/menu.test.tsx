import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { Menu } from "./menu.js";
import { icons } from "../../icons/vocabulary.js";

function Orgs() {
  return (
    <Menu open>
      <Menu.Trigger>
        <button type="button">Northwind Labs</button>
      </Menu.Trigger>
      <Menu.Content>
        <Menu.RadioGroup value="org_01J9Z4">
          <Menu.RadioItem value="org_01J9Z4" description="Deletion on 20 Oct" descriptionTone="danger">
            Northwind Labs
          </Menu.RadioItem>
          <Menu.RadioItem value="org_01J9Z7" description="3 projects">
            Southwind Retail
          </Menu.RadioItem>
        </Menu.RadioGroup>
      </Menu.Content>
    </Menu>
  );
}

describe("Menu", () => {
  it("shows a description in the danger tone, and in the secondary tone by default", async () => {
    render(<Orgs />);
    await waitFor(() => expect(screen.getByRole("menuitemradio", { name: /Northwind Labs/ })).toBeInTheDocument());
    expect(screen.getByText("Deletion on 20 Oct")).toHaveClass("text-danger-text", "text-small");
    expect(screen.getByText("Deletion on 20 Oct")).not.toHaveClass("text-fg-secondary");
    expect(screen.getByText("3 projects")).toHaveClass("text-fg-secondary");
  });

  it("keeps an item without an icon on the label column of a menu whose other items have icons", async () => {
    render(
      <Menu open>
        <Menu.Trigger>
          <button type="button">Key actions</button>
        </Menu.Trigger>
        <Menu.Content>
          <Menu.Item icon={icons.delete}>Delete key</Menu.Item>
          <Menu.Item>Archive key</Menu.Item>
        </Menu.Content>
      </Menu>,
    );
    await waitFor(() => expect(screen.getByRole("menuitem", { name: "Archive key" })).toBeInTheDocument());
    expect(screen.getByRole("menu")).toHaveClass("group/menu");
    expect(screen.getByRole("menuitem", { name: "Delete key" }).querySelector("[data-menu-glyph]")).not.toBeNull();
    const spacer = screen.getByRole("menuitem", { name: "Archive key" }).firstElementChild;
    expect(spacer).toHaveClass("hidden", "size-16", "group-has-data-menu-glyph/menu:block");
  });
});

describe("Menu figures", () => {
  it("sets an item's hint naming a version as written", async () => {
    render(
      <Menu open>
        <Menu.Trigger>
          <button type="button">Release</button>
        </Menu.Trigger>
        <Menu.Content>
          <Menu.Item hint="40.4.3-rc.1">Roll back</Menu.Item>
          <Menu.Item hint="12">Requests</Menu.Item>
        </Menu.Content>
      </Menu>,
    );
    expect(await screen.findByText("40.4.3-rc.1")).not.toHaveClass("tabular");
    expect(screen.getByText("12")).toHaveClass("tabular");
  });
});
