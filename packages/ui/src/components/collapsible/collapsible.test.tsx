import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Collapsible } from "./collapsible.js";

describe("Collapsible", () => {
  it("is a pill row whose title sits 12px in, with the chevron at its end, and its content on the title's line", async () => {
    render(
      <Collapsible title="Every right" hint="With where each comes from">
        <p>View organization</p>
      </Collapsible>,
    );
    const trigger = screen.getByRole("button", { name: /Every right/ });
    expect(trigger).toHaveClass("rounded-pill", "px-12", "hover:bg-fill-1", "focus-ring-inset", "coarse:min-h-44");
    expect(trigger.className).not.toMatch(/(^|\s)-mx-/);
    expect(trigger.lastElementChild?.querySelector("svg")).not.toBeNull();
    expect(trigger.lastElementChild).toHaveClass("-mx-6");
    const hint = screen.getByText("With where each comes from");
    expect(hint).toHaveClass("tabular-nums", "whitespace-nowrap");
    expect(hint.parentElement).toHaveClass("items-baseline");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("View organization").parentElement).toHaveClass("px-12");
  });
});
