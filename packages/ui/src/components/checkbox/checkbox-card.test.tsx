import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Checkbox, CheckboxGroup } from "./checkbox.js";

describe("Checkbox cards", () => {
  it("as a card on its own, is a raised row with the label first and the box at the end", () => {
    render(<Checkbox variant="card" label="Tools" description="Also calls the project's tools through its MCP portal." />);
    const box = screen.getByRole("checkbox", { name: "Tools" });
    expect(box).toHaveAccessibleDescription("Also calls the project's tools through its MCP portal.");
    const row = screen.getByText("Tools").closest("label");
    expect(row).toHaveClass("rounded-group", "bg-raised", "shadow-xs", "focus-ring-card-row-within", "hover:bg-raised-hover");
    expect(screen.getByText("Tools").compareDocumentPosition(box) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("Tools")).toHaveClass("text-body-lg", "font-medium");
  });
  it("inside a cards group, the rows share the group's surface and a hairline between them", () => {
    render(
      <CheckboxGroup variant="cards" aria-label="What the log keeps" defaultValue={["content"]}>
        <Checkbox value="content" label="Request content" description="Prompts and completions." />
        <Checkbox value="audio" label="Audio" disabled />
      </CheckboxGroup>,
    );
    const group = screen.getByRole("group", { name: "What the log keeps" });
    expect(group).toHaveClass("rounded-group", "bg-raised", "shadow-xs");
    const row = screen.getByText("Request content").closest("label");
    expect(row).toHaveClass("not-first:hairline-t");
    expect(row).not.toHaveClass("rounded-group");
    expect(screen.getByText("Audio").closest("label")).toHaveClass("cursor-not-allowed");
    expect(screen.getByText("Audio")).toHaveClass("text-fg-disabled");
  });
});
