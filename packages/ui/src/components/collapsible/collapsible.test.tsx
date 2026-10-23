import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Collapsible } from "./collapsible.js";

describe("Collapsible", () => {
  it("sits on the gutter like the text around it, with no box of its own, and shows its content under its title", async () => {
    render(
      <Collapsible title="Every right" hint="With where each comes from">
        <p>View organization</p>
      </Collapsible>,
    );
    const trigger = screen.getByRole("button", { name: /Every right/ });
    expect(trigger).not.toHaveClass("px-12");
    expect(trigger.className).not.toMatch(/(^|\s)-mx-|hover:bg-/);
    await userEvent.click(trigger);
    expect(screen.getByText("View organization").parentElement).toHaveClass("pl-24");
    expect(screen.getByText("View organization").parentElement).not.toHaveClass("pr-12");
  });
});
