import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Avatar, AvatarGroup, initials } from "./avatar";

describe("Avatar", () => {
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
});
