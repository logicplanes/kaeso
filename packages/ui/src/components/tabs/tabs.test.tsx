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
  it("hangs the first pill's padding outside a padded column, so its label starts on the column's edge", () => {
    const { rerender } = render(pills);
    expect(screen.getByRole("tablist")).toHaveClass("-m-4");
    expect(screen.getByRole("tablist")).not.toHaveClass("-ml-16");
    rerender(<InsetContent>{pills}</InsetContent>);
    expect(screen.getByRole("tablist")).toHaveClass("-ml-16");
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
