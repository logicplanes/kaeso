import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { GrantSummary } from "./grant-summary.js";

describe("GrantSummary", () => {
  it("lists every line under its name, with the note below", () => {
    render(<GrantSummary aria-label="What it can do" items={["Manages projects", "Reads the rest"]} footer="For 90 days" />);
    const list = screen.getByRole("list", { name: "What it can do" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["Manages projects", "Reads the rest"]);
    expect(screen.getByText("For 90 days")).toBeInTheDocument();
  });

  it("leaves the note out when there is none", () => {
    const { container } = render(<GrantSummary aria-label="Rights" items={["Reads the instance"]} />);
    expect(container.querySelector("p")).toBeNull();
  });

  it("puts an action next to the note", () => {
    render(<GrantSummary aria-label="Rights" items={["Reads the instance"]} footer="For 7 days" action={<button type="button">Customize</button>} />);
    expect(screen.getByRole("button", { name: "Customize" })).toBeInTheDocument();
    expect(screen.getByText("For 7 days")).toBeInTheDocument();
  });
});
