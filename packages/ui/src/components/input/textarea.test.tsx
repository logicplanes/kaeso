import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Textarea } from "./textarea.js";

afterEach(() => vi.restoreAllMocks());

describe("Textarea", () => {
  it("stops growing at exactly maxRows of its own line height, so no line is cut at the bottom", () => {
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(600);
    render(
      <Textarea
        aria-label="System prompt"
        mono
        maxRows={8}
        style={{ lineHeight: "18px", paddingTop: "10px", paddingBottom: "10px" }}
        defaultValue="You answer in one sentence."
      />,
    );
    expect(screen.getByRole("textbox", { name: "System prompt" }).style.height).toBe(`${8 * 18 + 20}px`);
  });
});
