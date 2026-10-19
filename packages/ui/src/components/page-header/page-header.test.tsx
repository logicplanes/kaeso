import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PageHeader } from "./page-header.js";
import { PillTabs } from "../tabs/tabs.js";

describe("PageHeader", () => {
  it("starts its tabs on the gutter, so a selected pill lines up with the toolbar under it on a phone", () => {
    render(
      <PageHeader
        title="Models"
        tabs={
          <PillTabs defaultValue="catalogue">
            <PillTabs.List aria-label="Models">
              <PillTabs.Tab value="catalogue">Catalogue</PillTabs.Tab>
              <PillTabs.Tab value="aliases">Aliases</PillTabs.Tab>
            </PillTabs.List>
          </PillTabs>
        }
      />,
    );
    const slot = screen.getByRole("tablist").parentElement!.parentElement!;
    expect(slot).toHaveClass("basis-full", "sm:ml-8");
    expect(slot.className).not.toMatch(/(^| )-ml-/);
    expect(screen.getByRole("banner")).toHaveClass("px-24");
  });
});
