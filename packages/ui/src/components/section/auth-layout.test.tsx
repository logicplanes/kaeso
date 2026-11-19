import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { AuthLayout } from "./section.js";

describe("AuthLayout", () => {
  it("breaks a long address or name in its title and description instead of running off a phone's screen", () => {
    const address = "someone.with.a.very.long.address@subdomain.example.com";
    render(
      <AuthLayout title="Join Internationale Forschungs- und Entwicklungsgesellschaft" description={address}>
        fields
      </AuthLayout>,
    );
    expect(screen.getByRole("heading", { level: 1 })).toHaveClass("wrap-break-word");
    expect(screen.getByText(address)).toHaveClass("wrap-break-word");
  });
});
