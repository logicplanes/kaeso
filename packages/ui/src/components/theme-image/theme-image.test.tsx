import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ThemeImage } from "./theme-image.js";

describe("ThemeImage", () => {
  it("draws the light and the dark image with the same text alternative and size", () => {
    render(<ThemeImage light="/flow.light.svg" dark="/flow.dark.svg" alt="Route of a request" width={480} height={120} aria-describedby="flow-text" />);
    const [light, dark] = screen.getAllByRole("img", { name: "Route of a request" });
    expect(light).toHaveAttribute("src", "/flow.light.svg");
    expect(dark).toHaveAttribute("src", "/flow.dark.svg");
    for (const img of [light, dark]) {
      expect(img).toHaveAttribute("width", "480");
      expect(img).toHaveAttribute("height", "120");
      expect(img).toHaveAttribute("aria-describedby", "flow-text");
      expect(img).toHaveAttribute("loading", "lazy");
    }
    expect(light).toHaveClass("theme-light-only");
    expect(dark).toHaveClass("theme-dark-only");
  });

  it("leaves the size to the image when none is given and keeps layout classes on the frame", () => {
    const { container } = render(<ThemeImage light="/a.svg" dark="/b.svg" alt="Logo" className="min-w-320" />);
    expect(container.firstElementChild).toHaveClass("min-w-320");
    expect(screen.getAllByRole("img", { name: "Logo" })[0]).not.toHaveAttribute("width");
  });
});
