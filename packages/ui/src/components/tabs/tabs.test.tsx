import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

const underline = (
  <Tabs defaultValue="a">
    <Tabs.List aria-label="Request">
      <Tabs.Tab value="a">Overview</Tabs.Tab>
      <Tabs.Tab value="b">Response</Tabs.Tab>
    </Tabs.List>
  </Tabs>
);

describe("PillTabs", () => {
  it("never pulls the list into the gutter, in a padded column too", () => {
    const { rerender } = render(pills);
    expect(screen.getByRole("tablist").className).not.toMatch(/(^|\s)-m[xl]?-\d+/);
    rerender(<InsetContent>{pills}</InsetContent>);
    expect(screen.getByRole("tablist").className).not.toMatch(/(^|\s)-m[xl]?-\d+/);
  });
  it("draws the focus ring inside the pill, so it never touches the next pill or gets cut by the scroller", () => {
    render(pills);
    for (const tab of screen.getAllByRole("tab")) expect(tab).toHaveClass("focus-ring-inset");
  });
});

describe("Tabs", () => {
  it("leaves underline tabs inside the column with the first label on the content edge", () => {
    render(<InsetContent>{underline}</InsetContent>);
    expect(screen.getByRole("tablist").className).not.toMatch(/(^|\s)-m[xl]?-\d+/);
    expect(screen.getByRole("tablist")).toHaveClass("gap-24");
    expect(screen.getByText("Overview").className).not.toMatch(/(^|\s)p[xl]-\d+/);
  });
  it("selects with Enter or Space after arrows move focus", async () => {
    render(underline);
    screen.getByRole("tab", { name: "Overview" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Response" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("tab", { name: "Response" })).toHaveAttribute("aria-selected", "true");
  });
  it("keeps the indicator visible in forced colours", () => {
    const { container } = render(underline);
    expect(container.querySelector(".bg-ink")).toHaveClass("forced-line");
  });
});
