import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PageHeader } from "./page-header.js";
import { Tabs } from "../tabs/tabs.js";

describe("PageHeader", () => {
  it("starts its tabs on the gutter, so a selected pill lines up with the toolbar under it on a phone", () => {
    render(
      <PageHeader
        title="Models"
        tabs={
          <Tabs variant="pill" defaultValue="catalogue">
            <Tabs.List aria-label="Models">
              <Tabs.Tab value="catalogue">Catalogue</Tabs.Tab>
              <Tabs.Tab value="aliases">Aliases</Tabs.Tab>
            </Tabs.List>
          </Tabs>
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
  it("keeps a long title and its actions on one row, the title shrinking first but never under 96px, and the tabs on the next", () => {
    render(<PageHeader title="Anfrageprotokolle für alle Projekte der Organisation" actions={<button type="button">Exportieren</button>} />);
    const heading = screen.getByRole("heading", { level: 1 });
    expect(heading).toHaveClass("min-w-0", "flex-1", "shrink-4", "sm:flex-initial", "max-sm:min-w-96");
    expect(screen.getByText(/^Anfrageprotokolle/)).toHaveClass("truncate");
    expect(screen.getByRole("button", { name: "Exportieren" }).parentElement).toHaveClass("ml-auto", "shrink-0");
  });
});
