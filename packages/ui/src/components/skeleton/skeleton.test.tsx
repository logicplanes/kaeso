import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { Skeleton, SkeletonLine } from "./skeleton.js";

describe("Skeleton", () => {
  it("is hidden from assistive technology and keeps an outline in forced colours, where its fill disappears", () => {
    const { container } = render(
      <>
        <Skeleton shape="line" />
        <SkeletonLine lines={2} />
      </>,
    );
    const shapes = container.querySelectorAll(".animate-pulse-quiet");
    expect(shapes).toHaveLength(3);
    for (const s of shapes) expect(s).toHaveClass("forced-colors:outline");
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });
});
