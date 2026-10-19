import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PillTabs, Tabs } from "./tabs.js";
import { InsetContent } from "../../utils/inset.js";

const pills = (
  <PillTabs defaultValue="all">
    <PillTabs.List aria-label="Keys">
      <PillTabs.Tab value="all">All</PillTabs.Tab>
      <PillTabs.Tab value="active">Active</PillTabs.Tab>
    </PillTabs.List>
  </PillTabs>
);

describe("PillTabs", () => {
  it("puts the first pill's fill on the column's edge, in a padded column too, so it lines up with the controls under it", () => {
    const { rerender } = render(pills);
    const list = screen.getByRole("tablist");
    expect(list).toHaveClass("-m-4", "p-4");
    expect(list.className).not.toMatch(/-ml-\d+/);
    rerender(<InsetContent>{pills}</InsetContent>);
    expect(screen.getByRole("tablist")).toHaveClass("-m-4", "p-4");
    expect(screen.getByRole("tablist").className).not.toMatch(/-ml-\d+/);
  });
  it("leaves underline tabs on the edge", () => {
    render(
      <InsetContent>
        <Tabs defaultValue="a">
          <Tabs.List aria-label="Request">
            <Tabs.Tab value="a">Overview</Tabs.Tab>
          </Tabs.List>
        </Tabs>
      </InsetContent>,
    );
    expect(screen.getByRole("tablist")).not.toHaveClass("-ml-16");
  });
});
