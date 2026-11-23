import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SOURCE_MARKS, SOURCE_NAMES, SourceBadge } from "./source-badge.js";
import { PROVIDER_MARKS } from "./provider-marks.js";

describe("SourceBadge", () => {
  it("is an image named after the kind's product, drawn with its mark", () => {
    render(<SourceBadge kind="claude_code" />);
    const tile = screen.getByRole("img", { name: "Claude Code" });
    expect(tile.querySelector("path")).toHaveAttribute("d", SOURCE_MARKS.claude_code);
    expect(tile).toHaveClass("size-20");
  });

  it("shows its name beside the tile, which then hides from assistive technology", () => {
    render(<SourceBadge kind="codex" name="Codex, build server" withName size="sm" />);
    expect(screen.getByText("Codex, build server")).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(document.querySelector("[data-source=codex]")).toHaveClass("size-16");
  });

  it("shows an icon for kinds without a published mark, and the source icon for unknown ones", () => {
    const { container } = render(
      <>
        <SourceBadge kind="litellm" />
        <SourceBadge kind="http" />
        <SourceBadge kind="carrier_pigeon" name="  " />
      </>,
    );
    expect(screen.getByRole("img", { name: "LiteLLM" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "HTTP" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "carrier_pigeon" })).toBeInTheDocument();
    expect(container.querySelectorAll("svg")).toHaveLength(3);
  });

  it("draws a different mark for every kind that has one, and OpenRouter's as its provider mark", () => {
    const marks = Object.values(SOURCE_MARKS);
    expect(marks.every((d) => d.length > 20)).toBe(true);
    expect(new Set(marks).size).toBe(marks.length);
    expect(SOURCE_MARKS.openrouter).toBe(PROVIDER_MARKS.openrouter?.d);
    expect(Object.keys(SOURCE_NAMES).filter((k) => !SOURCE_MARKS[k])).toEqual(["http", "litellm"]);
  });
});

describe("SourceBadge sizes", () => {
  it("match a provider tile's", async () => {
    const { KAESO_IDENTITY_VARIANTS } = await import("./identity.js");
    const { KAESO_SOURCE_BADGE_VARIANTS } = await import("./source-badge.js");
    for (const size of ["sm", "md", "lg", "xl"] as const)
      expect(KAESO_SOURCE_BADGE_VARIANTS.size[size].classes).toBe(KAESO_IDENTITY_VARIANTS.provider[size].classes);
  });
});
