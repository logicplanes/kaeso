import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { drawReleaseArt, ReleaseArt, releaseArtSvg } from "./release-art.js";

describe("ReleaseArt", () => {
  it("draws the same picture for the same seed and another for another seed", () => {
    const a = drawReleaseArt("0.3.0", 640, 360);
    expect(drawReleaseArt("0.3.0", 640, 360)).toEqual(a);
    const b = drawReleaseArt("0.3.1", 640, 360);
    expect(b.lines[0]?.d).not.toBe(a.lines[0]?.d);
    expect(a.lines).toHaveLength(34);
    for (const l of a.lines) {
      expect(l.opacity).toBeGreaterThan(0);
      expect(l.opacity).toBeLessThanOrEqual(0.7);
    }
  });

  it("is an image with a label and shows its title", () => {
    render(<ReleaseArt seed="0.3.0" label="Kaeso 0.3.0" title="0.3.0" eyebrow="What's new in Kaeso" shape="wide" />);
    const art = screen.getByRole("img", { name: "Kaeso 0.3.0" });
    expect(art).toHaveAttribute("viewBox", "0 0 840 360");
    expect(art).toHaveTextContent("0.3.0");
    expect(art).toHaveTextContent("What's new in Kaeso");
    expect(art.querySelectorAll("path")).toHaveLength(34);
  });

  it("leaves out text it isn't given", () => {
    render(<ReleaseArt seed="x" label="Artwork" />);
    expect(screen.getByRole("img", { name: "Artwork" }).querySelectorAll("text")).toHaveLength(0);
  });

  it("writes the same artwork as a file of its own, in either theme", () => {
    const light = releaseArtSvg({ seed: "0.3.0", label: "Kaeso <0.3.0>", title: "0.3.0", eyebrow: "Kaeso & friends" });
    expect(light.startsWith('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 360"')).toBe(true);
    expect(light).toContain('aria-label="Kaeso &lt;0.3.0&gt;"');
    expect(light).toContain("Kaeso &amp; friends");
    expect(light).toContain(drawReleaseArt("0.3.0", 640, 360).lines[0]?.d);
    const dark = releaseArtSvg({ seed: "0.3.0", label: "Kaeso 0.3.0", shape: "wide", theme: "dark" });
    expect(dark).toContain('viewBox="0 0 840 360"');
    expect(dark).not.toContain("<text");
    expect(dark).not.toBe(light);
  });
});
