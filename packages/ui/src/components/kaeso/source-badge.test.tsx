import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { SOURCE_MARK_LOOKS, SOURCE_MARKS, SOURCE_NAMES, SourceBadge } from "./source-badge.js";
import { PROVIDER_MARKS } from "./provider-marks.js";

describe("SourceBadge", () => {
  it("is an image named after the kind's product, drawn with its mark", () => {
    render(<SourceBadge kind="claude_code" />);
    const tile = screen.getByRole("img", { name: "Claude Code" });
    expect(tile.querySelector("path")).toHaveAttribute("d", SOURCE_MARKS.claude_code);
    expect(tile).toHaveClass("size-20");
  });

  it("draws each client's mark in its maker's colours", () => {
    const colours = (kind: string) => {
      const { container, unmount } = render(<SourceBadge kind={kind} />);
      const svg = container.querySelector("svg")!;
      const found = [svg, ...svg.querySelectorAll("path, stop")].flatMap((el) => [...el.classList].filter((c) => c.startsWith("text-")));
      unmount();
      return [...new Set(found)];
    };
    expect(colours("claude_code")).toEqual(["text-mark-claude"]);
    expect(colours("gemini_cli")).toEqual(["text-mark-gemini-agent-platform-red", "text-mark-gemini-agent-platform-blue"]);
    expect(colours("antigravity")).toEqual(["text-mark-google-blue", "text-mark-google-green", "text-mark-google-yellow", "text-mark-google-red"]);
    expect(colours("opentelemetry")).toEqual(["text-mark-opentelemetry-orange", "text-mark-opentelemetry-blue"]);
    expect(colours("openrouter")).toEqual(["text-mark-openrouter"]);
    expect(colours("codex")).toEqual(["text-fg"]);
    expect(colours("grok")).toEqual(["text-mark-xai"]);
  });

  it("colours the published marks without changing them, and splits OpenTelemetry's into its two colours shape for shape", () => {
    for (const [kind, look] of Object.entries(SOURCE_MARK_LOOKS)) {
      if (kind === "opentelemetry") continue;
      expect(look.d ?? look.parts?.[0]?.d, kind).toBe(SOURCE_MARKS[kind]);
    }
    const shapes = (d: string) => (d.match(/z/gi) ?? []).length;
    const otel = SOURCE_MARK_LOOKS.opentelemetry!.parts!;
    expect(otel).toHaveLength(2);
    expect(shapes(otel[0]!.d) + shapes(otel[1]!.d)).toBe(shapes(SOURCE_MARKS.opentelemetry!));
    expect(Object.keys(SOURCE_MARK_LOOKS).sort()).toEqual(Object.keys(SOURCE_MARKS).sort());
  });

  it("shows its name beside the tile, which then hides from assistive technology", () => {
    render(<SourceBadge kind="codex" name="Codex, build server" withName size="sm" />);
    expect(screen.getByText("Codex, build server")).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(document.querySelector("[data-source=codex]")).toHaveClass("size-16");
  });

  it("hides a decorative tile from assistive technology", () => {
    render(<SourceBadge kind="gemini_cli" decorative />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(document.querySelector("[data-source=gemini_cli]")).toHaveAttribute("aria-hidden", "true");
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
    expect(SOURCE_MARKS.grok).toBe(PROVIDER_MARKS.xai?.d);
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
