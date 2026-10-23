import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Collapsible } from "./collapsible.js";

describe("Collapsible", () => {
  it("lines its chevron and hint up with the column it sits in, and its content up with its title", async () => {
    render(
      <Collapsible title="Every right" hint="With where each comes from">
        <p>View organization</p>
      </Collapsible>,
    );
    const trigger = screen.getByRole("button", { name: /Every right/ });
    expect(trigger).toHaveClass("-mx-12", "px-12");
    await userEvent.click(trigger);
    expect(screen.getByText("View organization").parentElement).toHaveClass("pl-24");
    expect(screen.getByText("View organization").parentElement).not.toHaveClass("pr-12");
  });
});
