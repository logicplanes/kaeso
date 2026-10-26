import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Text } from "./text.js";
import { SwapText } from "../swap-text/swap-text.js";

function overflowing() {
  vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(120);
}

afterEach(() => vi.restoreAllMocks());

describe("Text truncate", () => {
  it("becomes a focus stop with the house focus ring only when its full text has a tooltip", () => {
    overflowing();
    render(<Text truncate>Northwind Labs Research and Development</Text>);
    const text = screen.getByText("Northwind Labs Research and Development");
    expect(text).toHaveAttribute("tabindex", "0");
    expect(text).toHaveClass("focus-ring", "rounded-code");
  });
  it("never leaves a dead tab stop when the content is not plain text", () => {
    overflowing();
    render(
      <Text truncate>
        <b>Northwind</b> Labs
      </Text>,
    );
    expect(screen.getByText("Northwind").parentElement).not.toHaveAttribute("tabindex");
  });
  it("stays out of the tab order when it fits", () => {
    render(<Text truncate>Keys</Text>);
    expect(screen.getByText("Keys")).not.toHaveAttribute("tabindex");
  });
});

describe("SwapText", () => {
  it("keeps every reserved label on one line, so a longer translation never doubles the row's height", () => {
    render(<SwapText reserve={["Wird gestreamt", "Fertig"]}>Fertig</SwapText>);
    expect(screen.getAllByText("Fertig")[0]!.parentElement).toHaveClass("inline-grid", "whitespace-nowrap");
  });
});
