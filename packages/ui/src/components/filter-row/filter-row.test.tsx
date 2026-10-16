import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FilterRow } from "./filter-row";
import { PillTabs } from "../tabs/tabs";
import { Button } from "../button/button";
import { InsetContent } from "../../utils/inset";

function Row() {
  return (
    <FilterRow
      data-testid="row"
      tabs={
        <PillTabs defaultValue="all">
          <PillTabs.List aria-label="State">
            <PillTabs.Tab value="all">All</PillTabs.Tab>
            <PillTabs.Tab value="active">Active</PillTabs.Tab>
            <PillTabs.Tab value="suspended">Suspended</PillTabs.Tab>
          </PillTabs.List>
        </PillTabs>
      }
    >
      <Button size="sm">Every preset</Button>
      <Button size="sm">Every project</Button>
    </FilterRow>
  );
}

describe("FilterRow", () => {
  it("puts the tabs first and the filters in their own toolbar", () => {
    render(<Row />);
    const toolbar = screen.getByRole("toolbar", { name: "Filters" });
    expect(toolbar).toContainElement(screen.getByRole("button", { name: "Every preset" }));
    expect(toolbar).not.toContainElement(screen.getByRole("tablist"));
    expect(screen.getByRole("tablist").compareDocumentPosition(toolbar) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  it("moves across the filters with Left and Right, and leaves the tabs' arrows to the tabs", async () => {
    render(<Row />);
    screen.getByRole("button", { name: "Every preset" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Every project" })).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("button", { name: "Every preset" })).toHaveFocus();
    screen.getByRole("tab", { name: "All" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Active" })).toHaveFocus();
  });
  it("takes the settings rhythm in a padded column", () => {
    const { rerender } = render(<Row />);
    expect(screen.getByTestId("row")).toHaveClass("px-24");
    rerender(
      <InsetContent>
        <Row />
      </InsetContent>,
    );
    expect(screen.getByTestId("row")).toHaveClass("pt-16", "pb-16");
    expect(screen.getByTestId("row")).not.toHaveClass("px-24");
  });
  it("has no toolbar without filters", () => {
    render(<FilterRow tabs={<span>Tabs</span>} />);
    expect(screen.queryByRole("toolbar")).not.toBeInTheDocument();
  });
});
