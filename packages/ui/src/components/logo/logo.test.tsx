import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Logo } from "./logo.js";
import { AuthLayout } from "../section/section.js";
import { Sidebar } from "../sidebar/sidebar.js";

describe("the navigation wordmark", () => {
  it("draws Logo.Wordmark at nav size 60 by 12, named, decorative or styled, with its ref", () => {
    const ref = createRef<SVGSVGElement>();
    render(
      <>
        <Logo.Wordmark ref={ref} size="nav" />
        <Logo.Wordmark size="nav" title="Kaeso home" className="text-fg" style={{ opacity: 0.5 }} />
        <Logo.Wordmark size="nav" decorative />
      </>,
    );
    const [plain, named] = screen.getAllByRole("img");
    expect(plain).toHaveAccessibleName("Kaeso");
    expect(plain).toHaveAttribute("viewBox", "72 69.3 881.9 177.7");
    expect(plain).toHaveClass("w-60", "h-12");
    expect(plain).not.toHaveAttribute("style");
    expect(ref.current).toBe(plain);
    expect(named).toHaveAccessibleName("Kaeso home");
    expect(named).toHaveClass("block", "shrink-0", "text-fg");
    expect(named).toHaveStyle({ opacity: "0.5" });
    expect(named).toHaveClass("w-60");
    expect(document.querySelectorAll('svg[aria-hidden="true"]')).toHaveLength(1);
  });

  it("is the same drawing on the sign-in layout and in the sidebar as Logo.Wordmark at nav size", () => {
    const { container: logo } = render(<Logo.Wordmark size="nav" />);
    const { container: auth } = render(<AuthLayout title="Sign in">fields</AuthLayout>);
    const { container: sidebar } = render(
      <Sidebar.Provider shortcut={false}>
        <Sidebar aria-label="Console" resizable={false}>
          <Sidebar.Brand label="Kaeso" />
        </Sidebar>
      </Sidebar.Provider>,
    );
    const drawing = logo.querySelector("svg")!.outerHTML;
    expect(auth.querySelector("svg")!.outerHTML).toBe(drawing);
    expect(sidebar.querySelector("svg")!.outerHTML.replace(' class="block h-12 w-60 shrink-0 text-fg"', ' class="block h-12 w-60 shrink-0"')).toBe(drawing);
  });
});
