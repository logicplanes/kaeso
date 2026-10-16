import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Link } from "./link.js";

describe("Link", () => {
  it("opens an external address in a new tab and says so", () => {
    render(<Link href="https://github.com/logicplanes/kaeso">Read the gateway setup guide</Link>);
    const link = screen.getByRole("link", { name: "Read the gateway setup guide (opens in a new tab)" });
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("keeps the external mark on the line of the last word when the text wraps", () => {
    render(<Link href="https://github.com/logicplanes/kaeso">Read the gateway setup guide</Link>);
    const mark = screen.getByRole("link").querySelector("svg")!.parentElement!;
    expect(mark.textContent).toBe("⁠");
    expect(mark).toHaveClass("whitespace-nowrap");
  });

  it("stays in the page for an address of the app", () => {
    render(<Link href="/keys">API keys</Link>);
    const link = screen.getByRole("link", { name: "API keys" });
    expect(link).not.toHaveAttribute("target");
    expect(link.querySelector("svg")).toBeNull();
  });
});
