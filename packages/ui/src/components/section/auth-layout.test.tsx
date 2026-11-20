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

  it("ends at its description when there is no form, without space held for one", () => {
    render(<AuthLayout title="This link doesn't work" description="Ask whoever shared it for a new one." />);
    const column = screen.getByRole("heading", { level: 1 }).parentElement;
    expect(column?.lastElementChild).toBe(screen.getByText("Ask whoever shared it for a new one."));
  });

  it("centers its footer across the column, so a footer that wraps on a phone stays centered", () => {
    render(
      <AuthLayout title="Connect Claude Code" footer={<span>Signed in as owner@example.com · returns to app.example.com</span>}>
        fields
      </AuthLayout>,
    );
    expect(screen.getByText("Signed in as owner@example.com · returns to app.example.com").parentElement).toHaveClass("w-full", "max-w-auth", "text-center");
  });

  it("holds no form slot for children that render nothing", () => {
    render(
      <AuthLayout title="Signed in again" description="You can close this window and continue in Kaeso.">
        {false}
        {null}
      </AuthLayout>,
    );
    expect(screen.getByRole("heading", { level: 1 }).parentElement?.lastElementChild).toHaveTextContent("You can close this window and continue in Kaeso.");
  });
});
