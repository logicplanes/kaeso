import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { Figure } from "./figure.js";

describe("Figure", () => {
  it("shows its content in a named region with the caption under it", () => {
    render(
      <Figure label="Diagram: route of a request" caption="Route of a request">
        <img src="/route.svg" alt="Route of a request: app, Kaeso, provider" />
      </Figure>,
    );
    const figure = screen.getByRole("figure");
    const region = screen.getByRole("region", { name: "Diagram: route of a request" });
    expect(figure).toContainElement(region);
    expect(region).toContainElement(screen.getByRole("img", { name: "Route of a request: app, Kaeso, provider" }));
    expect(figure.querySelector("figcaption")).toHaveTextContent("Route of a request");
  });

  it("leaves the caption out when none is given and keeps layout classes on the figure", () => {
    const { container } = render(
      <Figure label="Diagram" surface="plain" className="mt-16">
        <span>Drawing</span>
      </Figure>,
    );
    expect(container.querySelector("figcaption")).toBeNull();
    expect(container.firstElementChild).toHaveClass("mt-16");
  });

  afterEach(() => vi.restoreAllMocks());

  it("starts a figure wider than its column scrolled to the middle, so a top-down diagram shows its main path", () => {
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(300);
    render(
      <Figure label="Diagram: a wide one">
        <span>Drawing</span>
      </Figure>,
    );
    expect(screen.getByRole("region", { name: "Diagram: a wide one" }).scrollLeft).toBe(250);
  });

  it("starts a wide figure on the part it is about, as far as the edges allow", () => {
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(300);
    const { rerender } = render(
      <Figure label="Diagram: focused" focus={0.6}>
        <span>Drawing</span>
      </Figure>,
    );
    const region = () => screen.getByRole("region", { name: "Diagram: focused" });
    expect(region().scrollLeft).toBe(330);
    rerender(
      <Figure label="Diagram: focused" focus={0.1}>
        <span>Drawing</span>
      </Figure>,
    );
    expect(region().scrollLeft).toBe(0);
    rerender(
      <Figure label="Diagram: focused" focus={0.95}>
        <span>Drawing</span>
      </Figure>,
    );
    expect(region().scrollLeft).toBe(500);
  });
});
