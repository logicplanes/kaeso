import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { hueOf, usePictureHues } from "./picture-hues";

const pixels = (...rgba: [number, number, number, number][]) => new Uint8ClampedArray(rgba.flat());

describe("hueOf", () => {
  it("reads the hue of a picture's colored pixels and none of a gray or empty one", () => {
    expect(hueOf(pixels([139, 92, 246, 255], [139, 92, 246, 255]))).toBe(258);
    expect(hueOf(pixels([16, 185, 129, 255], [0, 0, 0, 0]))).toBe(160);
    expect(hueOf(pixels([245, 158, 11, 255]))).toBe(38);
    expect(hueOf(pixels([250, 40, 80, 255]))).toBe(349);
    expect(hueOf(pixels([120, 120, 125, 255]))).toBeNull();
    expect(hueOf(pixels([0, 0, 0, 0]))).toBeNull();
  });
});

function Hues({ pictures }: { pictures: { id: string; src: string | null }[] }) {
  const hues = usePictureHues(pictures);
  return <output>{[...hues].map(([id, hue]) => `${id}:${hue}`).join(" ")}</output>;
}

describe("usePictureHues", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("reads each picture once, and skips pictures that don't load or can't be drawn", async () => {
    const loads: string[] = [];
    class FakeImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(src: string) {
        loads.push(src);
        queueMicrotask(() => (src.includes("broken") ? this.onerror?.() : this.onload?.()));
      }
    }
    vi.stubGlobal("Image", FakeImage);
    let draw = true;
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
      () => (draw ? { drawImage: () => {}, getImageData: () => ({ data: pixels([16, 185, 129, 255]) }) } : null) as unknown as CanvasRenderingContext2D,
    );
    const { rerender } = render(
      <Hues
        pictures={[
          { id: "a", src: "/green.webp" },
          { id: "b", src: "/broken.webp" },
          { id: "c", src: null },
        ]}
      />,
    );
    await act(async () => {});
    expect(screen.getByRole("status")).toHaveTextContent("a:160");
    draw = false;
    rerender(
      <Hues
        pictures={[
          { id: "a", src: "/green.webp" },
          { id: "d", src: "/blank.webp" },
        ]}
      />,
    );
    await act(async () => {});
    expect(screen.getByRole("status")).toHaveTextContent(/^a:160$/);
    expect(loads).toEqual(["/green.webp", "/broken.webp", "/blank.webp"]);
  });
});
