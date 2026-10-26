import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { Avatar, AvatarGroup, initials } from "./avatar.js";

describe("Avatar", () => {
  let loaded: ReturnType<typeof vi.spyOn> | undefined;
  beforeEach(() => {
    loaded = vi.spyOn(HTMLImageElement.prototype, "naturalWidth", "get").mockReturnValue(64);
  });
  afterEach(() => loaded?.mockRestore());

  it("shows the initials for a picture that failed before the page became interactive", () => {
    loaded?.mockReturnValue(0);
    render(<Avatar name="Ada Okafor" src="https://example.com/missing.png" />);
    expect(screen.getByRole("img", { name: "Ada Okafor" })).toHaveTextContent("AO");
  });

  it("takes the first letters of the first and last word", () => {
    expect(initials("Ada Okafor")).toBe("AO");
    expect(initials("  ada  van der okafor ")).toBe("AO");
    expect(initials("Northwind")).toBe("N");
    expect(initials("")).toBe("?");
    expect(initials("Ada Okafor", 1)).toBe("A");
  });

  it("shows one letter at 20px, where two don't fit the circle", () => {
    render(<Avatar name="Ada Okafor" size={20} />);
    expect(screen.getByRole("img", { name: "Ada Okafor" })).toHaveTextContent(/^A$/);
  });

  it("names a group by its people and counts the rest", () => {
    render(<AvatarGroup people={[{ name: "Ada Okafor" }, { name: "Lena Park" }, { name: "Tomás Reyes" }, { name: "Mei Chen" }]} max={2} />);
    const g = screen.getByRole("group", { name: "Ada Okafor, Lena Park, Tomás Reyes, Mei Chen" });
    expect(g).toHaveTextContent("+2");
  });

  it("tries a new picture after the last one failed", () => {
    const { container, rerender } = render(<Avatar name="Ada Okafor" src="https://example.com/ada-old.png" />);
    fireEvent.error(container.querySelector("img")!);
    expect(screen.getByRole("img", { name: "Ada Okafor" })).toHaveTextContent("AO");
    rerender(<Avatar name="Ada Okafor" src="https://example.com/ada.png" />);
    expect(container.querySelector("img")).toHaveAttribute("src", "https://example.com/ada.png");
    expect(screen.getByRole("img", { name: "Ada Okafor" })).not.toHaveTextContent("AO");
  });

  it("shows its content after the avatars", () => {
    render(
      <AvatarGroup people={[{ name: "Ada Okafor" }]}>
        <button type="button">Invite people</button>
      </AvatarGroup>,
    );
    expect(screen.getByRole("group", { name: "Ada Okafor" })).toContainElement(screen.getByRole("button", { name: "Invite people" }));
  });

  it("separates overlapping avatars with a gap cut out of the one below, which works on any surface", () => {
    const { container } = render(<AvatarGroup size={32} people={[{ name: "Ada Okafor" }, { name: "Lin Park" }, { name: "Mira Sato" }]} max={2} />);
    const slots = [...container.querySelectorAll<HTMLElement>("[role=group] > span")];
    expect(slots).toHaveLength(3);
    expect(slots[0]).toHaveClass("cutout");
    expect(slots[0]!.style.getPropertyValue("--k-cut-x-ltr")).toBe("42px");
    expect(slots[0]!.style.getPropertyValue("--k-cut-r")).toBe("18px");
    expect(slots[1]).toHaveClass("cutout", "-ms-6");
    expect(slots[2]).not.toHaveClass("cutout");
    expect(slots[2]).toHaveTextContent("+1");
    expect(slots[2]!.firstElementChild).toHaveClass("mark-fill");
    expect(container.querySelector(".ring-panel")).toBeNull();
  });

  it("draws a hairline around a photo so a light picture keeps its edge", () => {
    const { container } = render(<Avatar name="Ada Okafor" src="https://example.com/ada.png" />);
    expect(container.querySelector("img + .shadow-ring")).toBeInTheDocument();
  });
});
