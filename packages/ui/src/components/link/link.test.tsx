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

  it("hands an app link, such as an authenticator's, to its app in the same tab", () => {
    render(<Link href="otpauth://totp/Kaeso:owner%40example.com?secret=JBSWY3DPEHPK3PXP">Open in an authenticator app on this device</Link>);
    const link = screen.getByRole("link", { name: "Open in an authenticator app on this device" });
    expect(link).not.toHaveAttribute("target");
    expect(link.querySelector("svg")).toBeNull();
  });

  it("keeps the external mark on the line of the last word when the text wraps", () => {
    render(<Link href="https://github.com/logicplanes/kaeso">Read the gateway setup guide</Link>);
    const mark = screen.getByRole("link").querySelector("svg")!.parentElement!;
    expect(mark.textContent).toBe("⁠");
    expect(mark).toHaveClass("whitespace-nowrap");
  });

  it("links a heading to itself without adding the mark to its name", () => {
    render(
      <h2 id="create-a-key">
        <Link href="#create-a-key" variant="heading">
          Create a key
        </Link>
      </h2>,
    );
    const link = screen.getByRole("link", { name: "Create a key" });
    expect(link).toHaveAttribute("href", "#create-a-key");
    expect(link).toHaveTextContent("Create a key#");
    expect(link.lastElementChild).toHaveAttribute("aria-hidden", "true");
    expect(link).toHaveClass("text-inherit", "no-underline");
  });

  it("draws a page to read next as a filled well with its name and sentence", () => {
    render(
      <Link href="/console/api-keys" variant="card">
        <span>API keys</span>
        <span>Limit a key to the models an app needs.</span>
      </Link>,
    );
    const card = screen.getByRole("link", { name: "API keys Limit a key to the models an app needs." });
    expect(card).toHaveClass("bg-fill-1", "rounded-well", "flex-col");
    expect(card).not.toHaveTextContent("#");
  });

  it("stays in the page for an address of the app", () => {
    render(<Link href="/keys">API keys</Link>);
    const link = screen.getByRole("link", { name: "API keys" });
    expect(link).not.toHaveAttribute("target");
    expect(link.querySelector("svg")).toBeNull();
  });
});
