import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { Menu } from "./menu";

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
});
