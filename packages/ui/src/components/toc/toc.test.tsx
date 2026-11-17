import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { TableOfContents, KAESO_TABLE_OF_CONTENTS_VARIANTS } from "./toc.js";

const items = [
  { id: "installation", label: "Installation" },
  { id: "usage", label: "Usage" },
  { id: "sizes", label: "Sizes", depth: 1 as const },
];

describe("TableOfContents", () => {
  it("names the list by its title and marks the heading in view at full strength", () => {
    render(<TableOfContents items={items} active="usage" />);
    const nav = screen.getByRole("navigation", { name: "On this page" });
    const current = screen.getByRole("link", { name: "Usage" });
    expect(nav).toContainElement(current);
    expect(current).toHaveAttribute("aria-current", "location");
    expect(current).toHaveClass(KAESO_TABLE_OF_CONTENTS_VARIANTS.state.active.classes);
    expect(screen.getByRole("link", { name: "Installation" })).not.toHaveAttribute("aria-current");
    expect(screen.getByRole("link", { name: "Installation" })).toHaveClass("text-fg-tertiary");
  });
  it("draws a hairline under every row and the ink line under the current one only, kept in forced colours", () => {
    const { container } = render(<TableOfContents items={items} active="usage" />);
    const rows = screen.getAllByRole("listitem");
    for (const row of rows) expect(row).toHaveClass("hairline-b-strong");
    expect(screen.getByRole("list")).toHaveClass("hairline-t-strong");
    const lines = [...container.querySelectorAll(".bg-ink")];
    expect(lines).toHaveLength(items.length);
    expect(lines.filter((l) => !l.classList.contains("opacity-0"))).toEqual([rows[1]!.querySelector(".bg-ink")]);
    for (const line of lines) expect(line).toHaveClass("forced-line");
  });
  it("starts a subheading's row and its line 12px in, never through a negative margin", () => {
    render(<TableOfContents items={items} active="installation" />);
    expect(screen.getByRole("link", { name: "Sizes" }).parentElement).toHaveClass("ms-12");
    for (const row of screen.getAllByRole("listitem")) expect(row.className).not.toMatch(/(^|\s)-m/);
  });
  it("keeps every row at least 32px tall, 44px under a coarse pointer", () => {
    render(<TableOfContents items={items} />);
    for (const link of screen.getAllByRole("link")) expect(link).toHaveClass("min-h-32", "coarse:min-h-44");
  });
});
