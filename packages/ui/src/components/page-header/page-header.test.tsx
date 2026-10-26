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

describe("PageHeader on a phone", () => {
  it("keeps a long title and its actions on one row, the title shrinking first, and the tabs on the next", () => {
    render(<PageHeader title="Anfrageprotokolle für alle Projekte der Organisation" actions={<button type="button">Exportieren</button>} />);
    const heading = screen.getByRole("heading", { level: 1 });
    expect(heading).toHaveClass("min-w-0", "flex-1", "shrink-4", "sm:flex-initial");
    expect(screen.getByText(/^Anfrageprotokolle/)).toHaveClass("truncate");
    expect(screen.getByRole("button", { name: "Exportieren" }).parentElement).toHaveClass("ml-auto", "shrink-0");
  });
});
