import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Button } from "../button/button.js";
import { Segmented } from "../segmented/segmented.js";
import { Section } from "./section.js";

describe("Section", () => {
  it("keeps its actions inside the header's width, wrapping them on a narrow screen", () => {
    render(
      <Section title="Breakdown" description="Requests, errors, tokens and cost in the period" actions={<Button size="sm">Download CSV</Button>}>
        <p>Rows</p>
      </Section>,
    );
    const actions = screen.getByRole("button", { name: "Download CSV" }).parentElement;
    expect(actions).toHaveClass("max-w-full", "flex-wrap");
  });

  it("lets a control among its actions narrow to the header, so a long one scrolls inside itself instead of the page", () => {
    const options = ["day", "model", "key", "member", "route", "client"].map((v) => ({ value: v, label: `By ${v}` }));
    render(
      <Section
        title="Breakdown"
        actions={
          <span className="flex flex-wrap items-center gap-8">
            <Segmented aria-label="Group by" size="sm" options={options} />
            <Button size="sm">Download CSV</Button>
          </span>
        }
      >
        <p>Rows</p>
      </Section>,
    );
    const group = screen.getByRole("radiogroup", { name: "Group by" });
    expect(group).toHaveClass("max-w-full");
    expect(group.parentElement?.parentElement).toHaveClass("*:min-w-0", "*:max-w-full");
  });

  it("lines its actions up with the description when there is one, and with the title when there isn't", () => {
    const { rerender } = render(
      <Section title="Models" description="Every model its owner may use" actions={<Button size="sm">Edit</Button>}>
        <p>Rows</p>
      </Section>,
    );
    expect(screen.getByRole("heading", { name: "Models" }).closest("header")).toHaveClass("items-end");
    rerender(
      <Section title="Models" actions={<Button size="sm">Edit</Button>}>
        <p>Rows</p>
      </Section>,
    );
    expect(screen.getByRole("heading", { name: "Models" }).closest("header")).toHaveClass("items-center");
    expect(screen.getByRole("heading", { name: "Models" }).closest("header")).not.toHaveClass("items-end");
  });

  it("keeps a section's actions beside a long title instead of dropping them under it at the start", () => {
    render(
      <Section
        spacing="settings"
        title="Internationale Forschungs- und Entwicklungsgesellschaft für Sprachmodelle"
        actions={<button type="button">Leave organization</button>}
      >
        <p>Rows</p>
      </Section>,
    );
    const titleBlock = screen.getByRole("heading", { name: /^Internationale/ }).parentElement!;
    expect(titleBlock, "the title grows beside the actions and keeps 240px before they wrap under it").toHaveClass("min-w-0", "sm:grow", "sm:basis-240");
    expect(titleBlock).not.toHaveClass("sm:flex-1");
  });

  it("lets a section's description run to the actions, so an 80-character description stays on one line", () => {
    render(
      <Section spacing="settings" title="Storage" description="Die gesamte Installation; jede Organisation sieht ihren eigenen unter Protokollierung.">
        <p>Rows</p>
      </Section>,
    );
    expect(screen.getByText(/^Die gesamte Installation/)).not.toHaveClass("max-w-prose");
  });
});
