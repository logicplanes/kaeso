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

  it("cuts a long title short before its tabs start to scroll, beside them on one line", () => {
    render(
      <PageHeader
        title="checkout-assistant-primary-reply-with-fallbacks-for-peak-traffic"
        tabs={
          <Tabs variant="pill" defaultValue="editor">
            <Tabs.List aria-label="Alias views">
              <Tabs.Tab value="editor">Editor</Tabs.Tab>
              <Tabs.Tab value="test">Test</Tabs.Tab>
              <Tabs.Tab value="audit">Audit log</Tabs.Tab>
            </Tabs.List>
          </Tabs>
        }
      />,
    );
    // sm:flex-initial resets flex-shrink to 1, so the title and the tabs would give up room alike and the last tab
    // would hide behind the list's edge while the title still had letters to spare.
    const title = screen.getByRole("heading", { level: 1 }).parentElement!;
    expect(title).toHaveClass("sm:flex-initial", "sm:shrink-1000");
    expect(screen.getByRole("tablist").parentElement!.parentElement).not.toHaveClass("shrink-0");
  });
});

describe("PageHeader on a phone", () => {
  it("moves the actions under a title that doesn't fit beside them, instead of cutting the title short", () => {
    render(<PageHeader title="Anfrageprotokolle für alle Projekte der Organisation" actions={<button type="button">Exportieren</button>} />);
    const heading = screen.getByRole("heading", { level: 1 });
    // flex-auto starts the title at its whole width, so a wrapping header breaks before the actions rather than shrinking it.
    expect(heading.parentElement).toHaveClass("min-w-0", "flex-auto", "shrink-4", "sm:flex-initial", "max-sm:min-w-96");
    expect(heading.parentElement).not.toHaveClass("flex-1");
    expect(screen.getByRole("banner")).toHaveClass("flex-wrap", "sm:flex-nowrap");
    expect(screen.getByText(/^Anfrageprotokolle/)).toHaveClass("truncate");
    expect(screen.getByRole("button", { name: "Exportieren" }).parentElement).toHaveClass("ml-auto", "shrink-0");
  });

  it("keeps a back button on the title's line when a long title sends the actions under it", () => {
    render(
      <PageHeader
        start={<button type="button">Back to aliases</button>}
        title="checkout-assistant-primary-reply-with-fallbacks-for-peak-traffic"
        actions={<button type="button">Save</button>}
      />,
    );
    const back = screen.getByRole("button", { name: "Back to aliases" });
    expect(back.parentElement).toBe(screen.getByRole("heading", { level: 1 }).parentElement);
    expect(back.parentElement).not.toHaveClass("flex-wrap");
    expect(screen.getByRole("button", { name: "Save" }).parentElement?.parentElement).toBe(screen.getByRole("banner"));
  });

  it("wraps actions wider than the page onto another line instead of running them off the edge", () => {
    render(
      <PageHeader
        title="Analysen"
        actions={
          <>
            <button type="button">USD</button>
            <button type="button">Letzte 30 Tage</button>
            <button type="button">Teilen</button>
          </>
        }
      />,
    );
    expect(screen.getByRole("button", { name: "Teilen" }).parentElement).toHaveClass("max-w-full", "flex-wrap", "justify-end");
  });
});
