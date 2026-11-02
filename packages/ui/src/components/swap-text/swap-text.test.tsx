import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SwapText } from "./swap-text.js";

describe("SwapText", () => {
  it("keeps a label on one line unless it may wrap", () => {
    render(
      <>
        <SwapText data-testid="label" reserve={["Copy page", "Copied"]}>
          Copy page
        </SwapText>
        <SwapText data-testid="hint" align="start" wrap reserve={["12 to 128 characters. A few unrelated words are easy to remember.", "3 more characters"]}>
          3 more characters
        </SwapText>
      </>,
    );
    expect(screen.getByTestId("label")).toHaveClass("whitespace-nowrap");
    expect(screen.getByTestId("hint")).toHaveClass("whitespace-normal");
    expect(screen.getByTestId("hint")).not.toHaveClass("whitespace-nowrap");
    expect(screen.getAllByText("3 more characters")).toHaveLength(2);
  });
});
