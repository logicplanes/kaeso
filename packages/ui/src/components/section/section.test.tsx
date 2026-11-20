import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Button } from "../button/button.js";
import { Section } from "./section.js";

describe("Section", () => {
  it("keeps its actions inside the header's width, wrapping them on a narrow screen", () => {
    render(
      <Section title="Breakdown" description="Requests, errors, tokens and cost in the period" actions={<Button size="sm">Download CSV</Button>}>
        <p>Rows</p>
      </Section>,
    );
    const actions = screen.getByRole("button", { name: "Download CSV" }).parentElement;
    expect(actions).toHaveClass("max-w-full", "min-w-0", "flex-wrap");
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
});
