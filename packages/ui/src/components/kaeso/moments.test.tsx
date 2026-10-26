import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SecretReveal, sdkSnippets } from "./moments.js";

const SECRET = "sk-kso-test-0123456789abcdef";

describe("SecretReveal", () => {
  it("puts the key in a .env file and has the SDK snippets read it from the environment", () => {
    render(<SecretReveal secret={SECRET} baseUrl="https://kaeso.example.com" />);
    expect(screen.getByRole("textbox", { name: "Key" })).toHaveTextContent(SECRET);
    expect(screen.getByRole("tab", { name: ".env" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText(`KAESO_API_KEY=${SECRET}`)).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Python" })).toBeInTheDocument();
  });

  it("warns that the key is shown once before the button that copies it", () => {
    render(<SecretReveal secret={SECRET} baseUrl="https://kaeso.example.com" snippets={false} />);
    const warning = screen.getByText("Shown once. Store it now.");
    const copy = screen.getByRole("button", { name: "Copy key" });
    expect(warning.compareDocumentPosition(copy) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("keeps every line of the SDK snippets short enough to read without scrolling, with a full key in it", () => {
    const key = "sk-kso-8rZzM2dDGU5seMyY0G3TFPuTZjhQWXaUQACHBZyM6RMZJQYn";
    for (const tab of sdkSnippets({ baseUrl: "https://kaeso.example.com", apiKey: key, model: "claude-sonnet-4-5" })) {
      expect(tab.code).toContain(key);
      for (const line of tab.code.split("\n")) expect(line.length, `${tab.label}: ${line}`).toBeLessThanOrEqual(80);
    }
  });

  it("shows other snippets, or none", () => {
    const { rerender } = render(
      <SecretReveal
        secret={SECRET}
        baseUrl="https://kaeso.example.com"
        snippets={[{ label: "curl", lang: "bash", code: `curl -H "Authorization: Bearer ${SECRET}"` }]}
      />,
    );
    expect(screen.queryByRole("tab", { name: "Python" })).not.toBeInTheDocument();
    expect(screen.getByText(/Authorization: Bearer/)).toBeInTheDocument();
    rerender(<SecretReveal secret={SECRET} baseUrl="https://kaeso.example.com" snippets={false} />);
    expect(screen.queryByText(/Authorization: Bearer/)).not.toBeInTheDocument();
  });
});
