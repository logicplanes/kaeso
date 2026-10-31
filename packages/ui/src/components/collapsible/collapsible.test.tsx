import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Accordion, Collapsible } from "./collapsible.js";

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

describe("Accordion", () => {
  it("uses the same row as Collapsible, and opens several at once when asked", async () => {
    render(
      <Accordion
        multiple
        items={[
          { value: "a", title: "How long are logs kept?", content: "30 days" },
          { value: "b", title: "Is request content stored?", content: "Only when content logging is on" },
        ]}
      />,
    );
    const [a, b] = screen.getAllByRole("button");
    expect(a).toHaveClass("rounded-pill", "px-12", "hover:bg-fill-1");
    await userEvent.click(a!);
    await userEvent.click(b!);
    expect(a).toHaveAttribute("aria-expanded", "true");
    expect(b).toHaveAttribute("aria-expanded", "true");
  });
});
