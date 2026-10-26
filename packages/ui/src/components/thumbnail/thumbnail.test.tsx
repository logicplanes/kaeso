import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot } from "react-dom/client";
import { Thumbnail } from "./thumbnail.js";

const loaded = (width: number) => vi.spyOn(HTMLImageElement.prototype, "naturalWidth", "get").mockReturnValue(width);

afterEach(() => vi.restoreAllMocks());

describe("Thumbnail", () => {
  it("shows the image with its text alternative, cropped to a 120px square by default, outlined like the other wells", () => {
    loaded(480);
    const ref = createRef<HTMLSpanElement>();
    render(<Thumbnail ref={ref} src="data:image/png;base64,iVBOR" alt="A receipt from Northwind Labs" className="mt-8" />);
    const img = screen.getByRole("img", { name: "A receipt from Northwind Labs" });
    expect(img).toHaveAttribute("src", "data:image/png;base64,iVBOR");
    expect(img).toHaveAttribute("loading", "lazy");
    expect(img).toHaveClass("object-cover");
    expect(ref.current).toContainElement(img);
    expect(ref.current).toHaveClass("size-120", "rounded-well", "mt-8");
    expect(ref.current?.querySelector("span[aria-hidden]")).toHaveClass("shadow-ring", "pointer-events-none");
  });

  it("takes its size from the scale, with a 12px fallback at 64px", () => {
    loaded(480);
    const { container, rerender } = render(<Thumbnail src="/a.png" alt="Photo of a receipt" size="sm" />);
    expect(container.firstElementChild).toHaveClass("size-64");
    fireEvent.error(screen.getByRole("img"));
    expect(screen.getByRole("img", { name: "Photo of a receipt" })).toHaveClass("text-caption");
    rerender(<Thumbnail src="/b.png" alt="Photo" size="lg" />);
    expect(container.firstElementChild).toHaveClass("size-160");
  });

  it("shows the text alternative in place of an image that can't load, and tries again for a new address", () => {
    loaded(480);
    const { rerender } = render(<Thumbnail src="/missing.png" alt="A chart of last week's spend" />);
    fireEvent.error(screen.getByRole("img"));
    const fallback = screen.getByRole("img", { name: "A chart of last week's spend" });
    expect(fallback.tagName).toBe("SPAN");
    expect(fallback).toHaveTextContent("A chart of last week's spend");
    rerender(<Thumbnail src="/found.png" alt="A chart of last week's spend" />);
    expect(screen.getByRole("img").tagName).toBe("IMG");
  });

  it("shows the text alternative for an image that failed before the page came alive", () => {
    loaded(0);
    vi.spyOn(HTMLImageElement.prototype, "complete", "get").mockReturnValue(true);
    const thumbnail = <Thumbnail src="/missing.png" alt="A chart of last week's spend" />;
    const container = document.createElement("div");
    container.innerHTML = renderToString(thumbnail);
    document.body.append(container);
    let root: ReturnType<typeof hydrateRoot> | undefined;
    act(() => {
      root = hydrateRoot(container, thumbnail);
    });
    expect(screen.getByRole("img", { name: "A chart of last week's spend" }).tagName).toBe("SPAN");
    act(() => root?.unmount());
    container.remove();
  });

  it("keeps an image it renders itself until the image reports an error", () => {
    loaded(0);
    vi.spyOn(HTMLImageElement.prototype, "complete", "get").mockReturnValue(true);
    render(<Thumbnail src="/receipt.png" alt="A receipt" />);
    expect(screen.getByRole("img", { name: "A receipt" }).tagName).toBe("IMG");
  });
});
