import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Icon } from "./icon.js";
import { icons } from "../../icons/vocabulary.js";
import { ICON_GROUPS } from "../../icons/groups.js";

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

  it("fills Octicons' 16px drawing in the text's color at every size", () => {
    const { container } = render(
      <>
        <Icon icon={icons.key} size="xs" />
        <Icon icon={icons.key} size="xl" />
      </>,
    );
    const drawn = [...container.querySelectorAll("svg")].map((svg) => [svg.getAttribute("width"), svg.getAttribute("viewBox"), svg.getAttribute("fill")]);
    expect(drawn).toEqual([
      ["14", "0 0 16 16", "currentColor"],
      ["24", "0 0 16 16", "currentColor"],
    ]);
  });

  it("files every name of the vocabulary under exactly one group of the reference", () => {
    const filed: string[] = Object.values(ICON_GROUPS).flat();
    expect([...filed].sort()).toEqual(Object.keys(icons).sort());
  });

  it("draws every Kaeso object differently, so two of them never look alike", () => {
    const objects = ICON_GROUPS["Kaeso objects"];
    expect(new Set(objects.map((name) => icons[name])).size).toBe(objects.length);
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
