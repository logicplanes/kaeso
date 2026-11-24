import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { Skeleton } from "./skeleton.js";

describe("Skeleton", () => {
  it("is hidden from assistive technology and keeps an outline in forced colours, where its fill disappears", () => {
    const { container } = render(
      <>
        <Skeleton shape="line" />
        <Skeleton lines={2} />
      </>,
    );
    const shapes = container.querySelectorAll(".animate-pulse-quiet");
    expect(shapes).toHaveLength(3);
    for (const s of shapes) expect(s).toHaveClass("forced-colors:outline");
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });

  it("keeps a trend line's 48px while its numbers load, so the cell doesn't grow when the line draws", () => {
    const { container } = render(<Skeleton shape="trend" />);
    expect(container.firstElementChild).toHaveClass("h-48", "rounded-well");
  });

  it("stands inside a line of text as an inline span, so a heading or a paragraph keeps its height and stays valid HTML", () => {
    const { container } = render(
      <p>
        <Skeleton shape="word" className="w-80" />
      </p>,
    );
    const word = container.querySelector("p > span");
    expect(word).toHaveClass("inline-block", "align-middle", "w-80");
    expect(word).toHaveAttribute("aria-hidden", "true");
  });
});
