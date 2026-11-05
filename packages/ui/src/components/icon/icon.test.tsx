import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Icon } from "./icon.js";
import { icons } from "../../icons/vocabulary.js";

describe("Icon", () => {
  it("is hidden by default and keeps its name when labelled, whatever else it is given", () => {
    const { container } = render(
      <>
        <Icon icon={icons.key} />
        <Icon icon={icons.key} label="Key" aria-hidden />
      </>,
    );
    expect(container.querySelectorAll("svg")[0]).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByRole("img", { name: "Key" })).not.toHaveAttribute("aria-hidden");
  });

  it("draws a 1.5px stroke at every size, however large the icon", () => {
    const { container } = render(
      <>
        <Icon icon={icons.key} size="xs" />
        <Icon icon={icons.key} size="xl" />
      </>,
    );
    const drawn = [...container.querySelectorAll("svg")].map((svg) => {
      const grid = Number(svg.querySelector("[stroke-width]")!.getAttribute("stroke-width"));
      return Math.round(((grid * Number(svg.getAttribute("width"))) / 24) * 100) / 100;
    });
    expect(drawn).toEqual([1.5, 1.5]);
  });

  it("draws one icon until it changes, then shrinks the old one out as the new one grows in", () => {
    const { container, rerender } = render(<Icon icon={icons.copy} label="Copy" />);
    expect(container.querySelectorAll("svg")).toHaveLength(1);
    rerender(<Icon icon={icons.check} label="Copied" />);
    const [out, inn] = container.querySelectorAll("svg");
    expect(out).toHaveClass("icon-swap-out");
    expect(inn).toHaveClass("icon-swap-in");
    expect(screen.getByRole("img", { name: "Copied" })).toContainElement(inn as unknown as HTMLElement);
    rerender(<Icon icon={icons.copy} />);
    expect(container.querySelectorAll("svg")).toHaveLength(2);
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });
});
