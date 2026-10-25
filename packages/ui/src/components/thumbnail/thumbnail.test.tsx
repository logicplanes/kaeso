import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Thumbnail } from "./thumbnail.js";

describe("Thumbnail", () => {
  it("shows the image with its text alternative, cropped to a 120px square by default", () => {
    const ref = createRef<HTMLSpanElement>();
    render(<Thumbnail ref={ref} src="data:image/png;base64,iVBOR" alt="A receipt from Northwind Labs" className="mt-8" />);
    const img = screen.getByRole("img", { name: "A receipt from Northwind Labs" });
    expect(img).toHaveAttribute("src", "data:image/png;base64,iVBOR");
    expect(img).toHaveAttribute("loading", "lazy");
    expect(img).toHaveClass("object-cover");
    expect(ref.current).toContainElement(img);
    expect(ref.current).toHaveClass("size-120", "rounded-well", "mt-8");
  });

  it("takes its size from the scale", () => {
    const { container, rerender } = render(<Thumbnail src="/a.png" alt="Photo" size="sm" />);
    expect(container.firstElementChild).toHaveClass("size-64");
    rerender(<Thumbnail src="/a.png" alt="Photo" size="lg" />);
    expect(container.firstElementChild).toHaveClass("size-160");
  });

  it("shows the text alternative in place of an image that can't load, and tries again for a new address", () => {
    const { rerender } = render(<Thumbnail src="/missing.png" alt="A chart of last week's spend" />);
    fireEvent.error(screen.getByRole("img"));
    const fallback = screen.getByRole("img", { name: "A chart of last week's spend" });
    expect(fallback.tagName).toBe("SPAN");
    expect(fallback).toHaveTextContent("A chart of last week's spend");
    rerender(<Thumbnail src="/found.png" alt="A chart of last week's spend" />);
    expect(screen.getByRole("img").tagName).toBe("IMG");
  });
});
