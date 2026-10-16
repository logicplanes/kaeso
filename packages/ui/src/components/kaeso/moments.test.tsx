import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SecretReveal } from "./moments";

const SECRET = "sk-kso-test-0123456789abcdef";

describe("SecretReveal", () => {
  it("shows the SDK snippets with the secret by default", () => {
    render(<SecretReveal secret={SECRET} baseUrl="https://gateway.example.com" />);
    expect(screen.getByRole("textbox", { name: "Secret" })).toHaveTextContent(SECRET);
    expect(screen.getByRole("tab", { name: "Python" })).toBeInTheDocument();
  });

  it("shows other snippets, or none", () => {
    const { rerender } = render(
      <SecretReveal
        secret={SECRET}
        baseUrl="https://gateway.example.com"
        snippets={[{ label: "curl", lang: "bash", code: `curl -H "Authorization: Bearer ${SECRET}"` }]}
      />,
    );
    expect(screen.queryByRole("tab", { name: "Python" })).not.toBeInTheDocument();
    expect(screen.getByText(/Authorization: Bearer/)).toBeInTheDocument();
    rerender(<SecretReveal secret={SECRET} baseUrl="https://gateway.example.com" snippets={false} />);
    expect(screen.queryByText(/Authorization: Bearer/)).not.toBeInTheDocument();
  });
});
