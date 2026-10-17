import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ScrollArea } from "./scroll-area/scroll-area.js";
import { Select } from "./select/select.js";

const styleElements = () => document.querySelectorAll("style").length;

describe("components under a strict Content-Security-Policy", () => {
  it("a scroll area hides its native scrollbar with the stylesheet, not an inline <style>", () => {
    render(
      <ScrollArea aria-label="Logs">
        <p>Lines</p>
      </ScrollArea>,
    );
    expect(screen.getByRole("region", { name: "Logs" })).toHaveClass("base-ui-disable-scrollbar");
    expect(styleElements()).toBe(0);
  });

  it("an open select adds no inline <style>", async () => {
    render(<Select aria-label="Model" options={[{ value: "gpt-5", label: "gpt-5" }]} />);
    await userEvent.click(screen.getByRole("combobox", { name: "Model" }));
    expect(await screen.findByRole("option", { name: "gpt-5" })).toBeInTheDocument();
    expect(styleElements()).toBe(0);
  });

  it("the stylesheet carries the rule that hides the scrollbar", () => {
    const css = readFileSync(join(__dirname, "../styles/kaeso.css"), "utf8");
    expect(css).toMatch(/\.base-ui-disable-scrollbar \{\s*scrollbar-width: none;\s*&::-webkit-scrollbar \{\s*display: none;/);
  });
});
