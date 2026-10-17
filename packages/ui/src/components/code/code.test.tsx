import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Code } from "./code.js";

describe("Code", () => {
  it("keeps short code in one box, a pill for one word", () => {
    render(<Code>gpt-5-mini</Code>);
    const code = screen.getByText("gpt-5-mini");
    expect(code.tagName).toBe("CODE");
    expect(code).toHaveClass("inline-block", "rounded-pill");
  });

  it("lets long code break with the sentence around it, each line keeping its fill", () => {
    render(
      <p>
        Run <Code flow="text">docker compose exec gateway kaeso openapi</Code> to print it.
      </p>,
    );
    const code = screen.getByText("docker compose exec gateway kaeso openapi");
    expect(code).toHaveClass("inline", "box-decoration-clone", "rounded-code");
    expect(code).not.toHaveClass("inline-block");
  });

  it("pads inside the chip only, never outside it", () => {
    render(
      <>
        <Code>10s</Code>
        <Code flow="text">5m</Code>
      </>,
    );
    for (const name of ["10s", "5m"]) {
      const classes = screen.getByText(name).className.split(" ");
      expect(classes).toContain("px-4");
      expect(classes.filter((c) => /^-?m[xylrtb]?-/.test(c))).toEqual([]);
    }
  });

  it("stays an inline box in the text flow, so punctuation after it stays beside it", () => {
    render(
      <p>
        Sizes such as <Code flow="text">32MiB</Code>, <Code flow="text">1MB</Code>, or a number.
      </p>,
    );
    const chip = screen.getByText("32MiB");
    expect(chip).toHaveClass("inline");
    expect(chip.nextSibling?.textContent).toBe(", ");
  });
});
