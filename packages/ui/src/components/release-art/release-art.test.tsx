import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { art } from "@logicplanes/kaeso-tokens";
import { ReleaseArt, releaseArtSvg } from "./release-art.js";
import {
  drawReleaseArt,
  isReleaseArtSpec,
  nextReleaseArt,
  randomReleaseArt,
  releaseArtFor,
  serializeArt,
  RELEASE_ART_STYLES,
  type ArtNode,
  type ReleaseArtSpec,
} from "./drawing.js";

function sequence(...values: number[]) {
  let i = 0;
  return () => values[i++ % values.length] ?? 0;
}

function walk(n: ArtNode, visit: (n: ArtNode) => void) {
  visit(n);
  for (const c of n.children ?? []) walk(c, visit);
}

const textBoxes = [
  { x0: 40, x1: 190, y0: 248, y1: 268 },
  { x0: 38, x1: 250, y0: 282, y1: 332 },
];

describe("release artwork", () => {
  it("draws the same picture for the same spec and a different one for another", () => {
    const spec: ReleaseArtSpec = { version: 1, seed: 42, style: "bauhaus", palette: 0 };
    const a = serializeArt(drawReleaseArt(spec));
    expect(serializeArt(drawReleaseArt(spec))).toBe(a);
    expect(serializeArt(drawReleaseArt({ ...spec, seed: 43 }))).not.toBe(a);
    expect(serializeArt(drawReleaseArt({ ...spec, palette: 1 }))).not.toBe(a);
  });

  it("draws every style in every palette on the ground of its palette, without blur or gradients", () => {
    for (const style of RELEASE_ART_STYLES) {
      for (let palette = 0; palette < art.palettes.length; palette++) {
        const tree = drawReleaseArt({ version: 1, seed: 7 + palette, style, palette }, { shape: palette % 2 ? "wide" : "banner" });
        const svg = serializeArt(tree);
        const own = art.palettes[palette];
        expect(svg).toContain(`fill="${art.grounds[own?.ground ?? "paper"].fill}"`);
        expect(svg).not.toMatch(/Gradient|filter|blur|opacity="0\./);
        let shapes = 0;
        walk(tree, (n) => {
          if (["rect", "circle", "path"].includes(n.tag)) shapes += 1;
        });
        expect(shapes).toBeGreaterThanOrEqual(3);
      }
    }
  });

  it("keeps the corner under the title open in every style", () => {
    for (const style of RELEASE_ART_STYLES) {
      for (let seed = 0; seed < 40; seed++) {
        const tree = drawReleaseArt({ version: 1, seed, style, palette: seed % art.palettes.length });
        const ground = art.grounds[art.palettes[seed % art.palettes.length]?.ground ?? "paper"].fill;
        walk(tree, (n) => {
          if (n.tag !== "circle" || n.attrs.fill === ground) return;
          const cx = Number(n.attrs.cx);
          const cy = Number(n.attrs.cy);
          const r = Number(n.attrs.r);
          for (const box of textBoxes) {
            const nx = Math.min(Math.max(cx, box.x0), box.x1);
            const ny = Math.min(Math.max(cy, box.y0), box.y1);
            expect(Math.hypot(cx - nx, cy - ny), `${style} ${seed}`).toBeGreaterThanOrEqual(r);
          }
        });
      }
    }
  });

  it("only accepts specs it can draw", () => {
    expect(isReleaseArtSpec({ version: 1, seed: 1, style: "spiro", palette: 0 })).toBe(true);
    expect(isReleaseArtSpec({ version: 2, seed: 1, style: "spiro", palette: 0 })).toBe(false);
    expect(isReleaseArtSpec({ version: 1, seed: -1, style: "spiro", palette: 0 })).toBe(false);
    expect(isReleaseArtSpec({ version: 1, seed: 1.5, style: "spiro", palette: 0 })).toBe(false);
    expect(isReleaseArtSpec({ version: 1, seed: 1, style: "glow", palette: 0 })).toBe(false);
    expect(isReleaseArtSpec({ version: 1, seed: 1, style: "spiro", palette: art.palettes.length })).toBe(false);
    expect(isReleaseArtSpec(null)).toBe(false);
    expect(isReleaseArtSpec("spiro")).toBe(false);
  });

  it("picks random specs and a fixed one for a version", () => {
    expect(randomReleaseArt(sequence(0))).toEqual({ version: 1, seed: 0, style: "bauhaus", palette: 0 });
    expect(randomReleaseArt(sequence(0.99999999999))).toEqual({ version: 1, seed: 4294967295, style: "spiro", palette: art.palettes.length - 1 });
    expect(releaseArtFor("0.1.0")).toEqual(releaseArtFor("0.1.0"));
    expect(isReleaseArtSpec(releaseArtFor("0.1.0"))).toBe(true);
  });

  it("never repeats a recent style, palette or any earlier seed", () => {
    const history: ReleaseArtSpec[] = [];
    const next = sequence(0.13, 0.71, 0.42, 0.05, 0.88, 0.37, 0.64);
    for (let i = 0; i < 60; i++) {
      const spec = nextReleaseArt(history, next);
      expect(isReleaseArtSpec(spec)).toBe(true);
      expect(history.slice(-3).map((s) => s.style)).not.toContain(spec.style);
      expect(history.slice(-4).map((s) => s.palette)).not.toContain(spec.palette);
      expect(history.map((s) => s.seed)).not.toContain(spec.seed);
      history.push(spec);
    }
    expect(nextReleaseArt([{ version: 1, seed: 0, style: "bauhaus", palette: 0 }], sequence(0)).seed).toBe(1);
  });

  it("writes attributes the SVG way and escapes text", () => {
    expect(serializeArt({ tag: "g", attrs: { strokeWidth: 2, viewBox: "0 0 1 1" }, children: [{ tag: "text", attrs: {}, text: 'a < b & "c"' }] })).toBe(
      '<g stroke-width="2" viewBox="0 0 1 1"><text>a &lt; b &amp; &quot;c&quot;</text></g>',
    );
    expect(serializeArt({ tag: "rect", attrs: { width: 1 } })).toBe('<rect width="1"/>');
  });
});

describe("ReleaseArt", () => {
  it("is an image with a label that shows its title and eyebrow", () => {
    const { container } = render(<ReleaseArt seed="0.1.0" label="Kaeso 0.1.0" title="0.1.0" eyebrow="What's new in Kaeso" shape="wide" />);
    const img = screen.getByRole("img", { name: "Kaeso 0.1.0" });
    expect(img).toHaveAttribute("viewBox", "0 0 840 360");
    const text = container.querySelector<HTMLElement>("span[aria-hidden]:not(.shadow-ring)")!;
    expect(text).toHaveTextContent("What's new in Kaeso0.1.0");
    expect(text.firstElementChild).toHaveClass("text-caption", "@cell-lg:text-body-lg");
    expect(text.lastElementChild?.className).toMatch(/\btext-(display|title-lg|title|title-sm)\b/);
    expect(text.style.maxWidth).toBe(`${(236 / 840) * 100}cqw`);
  });

  it("steps a title that doesn't fit down the type scale, wrapping only at the smallest", () => {
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(600);
    const wide = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (this: HTMLElement) {
      return /text-(display|title-lg)\b/.test(this.className) ? 500 : 100;
    });
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(200);
    const { container } = render(<ReleaseArt seed="0.3.0" label="Kaeso 0.3.0" title="Veröffentlichung 0.3.0" />);
    const title = container.querySelector("span.font-semibold")!;
    expect(title).toHaveClass("text-title", "whitespace-nowrap");
    wide.mockRestore();
    vi.restoreAllMocks();
  });

  it("draws the stored art when it has one, and the seed's art otherwise", () => {
    const spec: ReleaseArtSpec = { version: 1, seed: 9, style: "albers", palette: 2 };
    const { container, rerender } = render(<ReleaseArt seed="0.1.0" art={spec} label="Artwork" />);
    const stored = container.innerHTML;
    expect(container.querySelectorAll("rect").length).toBeGreaterThanOrEqual(4);
    rerender(<ReleaseArt seed="0.1.0" label="Artwork" />);
    expect(container.innerHTML).not.toBe(stored);
    expect(screen.getByRole("img", { name: "Artwork" }).querySelectorAll("text")).toHaveLength(0);
  });

  it("shows the stored artwork's image in the frame, and draws the picture when the image doesn't load", () => {
    const { container } = render(<ReleaseArt seed="0.1.0" src="https://update.kaeso.app/art/0.1.0.svg" label="Kaeso 0.1.0" title="0.1.0" shape="wide" />);
    const svg = screen.getByRole("img", { name: "Kaeso 0.1.0" });
    const image = svg.querySelector("image");
    expect(image).toHaveAttribute("href", "https://update.kaeso.app/art/0.1.0.svg");
    expect(image).toHaveAttribute("preserveAspectRatio", "xMidYMid slice");
    expect(svg).toHaveAttribute("viewBox", "0 0 840 360");
    expect(container).toHaveTextContent("0.1.0");
    expect(container.querySelectorAll("rect")).toHaveLength(0);
    fireEvent.error(image as Element);
    expect(svg.querySelector("image")).toBeNull();
    expect(container.querySelectorAll("rect").length).toBeGreaterThan(0);
  });

  it("gives every artwork on a page its own ids", () => {
    const spec: ReleaseArtSpec = { version: 1, seed: 3, style: "stripes", palette: 1 };
    const { container } = render(
      <>
        <ReleaseArt seed="a" art={spec} label="One" />
        <ReleaseArt seed="b" art={spec} label="Two" />
      </>,
    );
    const ids = [...container.querySelectorAll("[id]")].map((n) => n.id);
    expect(ids.length).toBeGreaterThan(1);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("writes the same artwork as a file of its own", () => {
    const spec: ReleaseArtSpec = { version: 1, seed: 5, style: "arcs", palette: 4 };
    const svg = releaseArtSvg({ seed: "0.1.0", art: spec, label: "Kaeso <0.1.0>", title: "0.1.0", eyebrow: "Kaeso & friends" });
    expect(
      svg.startsWith('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360" width="1280" height="720" role="img" aria-label="Kaeso &lt;0.1.0&gt;">'),
    ).toBe(true);
    expect(svg).toContain("Kaeso &amp; friends");
    expect(svg).toContain(`fill="${art.grounds.paper.fill}"`);
    expect(releaseArtSvg({ seed: "0.1.0", label: "x", shape: "wide" })).toContain('viewBox="0 0 840 360"');
    expect(releaseArtSvg({ seed: "0.1.0", label: "x" })).not.toContain("<text");
  });
});
