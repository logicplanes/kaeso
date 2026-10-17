import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { render, screen } from "@testing-library/react";
import { ScrollArea } from "./scroll-area.js";

const css = readFileSync(join(__dirname, "../../styles/kaeso.css"), "utf8");
const rule = (name: string) => new RegExp(`@utility ${name} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";

describe("ScrollArea", () => {
  it("scrolls in its own direction only, and nowhere else", () => {
    render(
      <>
        <ScrollArea aria-label="Vertical">
          <p>Lines</p>
        </ScrollArea>
        <ScrollArea orientation="horizontal" aria-label="Horizontal">
          <p>Wide</p>
        </ScrollArea>
        <ScrollArea orientation="both" aria-label="Both">
          <p>Grid</p>
        </ScrollArea>
      </>,
    );
    expect(screen.getByRole("region", { name: "Vertical" })).toHaveClass("scroll-axis-y");
    expect(screen.getByRole("region", { name: "Horizontal" })).toHaveClass("scroll-axis-x");
    expect(screen.getByRole("region", { name: "Horizontal" })).not.toHaveClass("scroll-axis-y");
    expect(screen.getByRole("region", { name: "Both" })).toHaveClass("scroll-axis-both");
  });

  it("hides the other axis and lets the scrolling axis show a bar only when its content overflows", () => {
    expect(rule("scroll-axis-x")).toMatch(/overflow-x: auto !important;\s*overflow-y: hidden !important;/);
    expect(rule("scroll-axis-y")).toMatch(/overflow-x: hidden !important;\s*overflow-y: auto !important;/);
    expect(rule("scroll-axis-both")).toMatch(/overflow: auto !important;/);
  });
});
