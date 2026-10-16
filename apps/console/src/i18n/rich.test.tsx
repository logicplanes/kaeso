import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { tRich } from "./rich";

describe("sentences with elements in them", () => {
  it("places the element where the sentence names it, with plain values around it", () => {
    render(<p>{tRich("test.rich", "{count} keys in {project}; {link}", { count: 3, project: "Atlas", link: <a href="/keys">see them</a> })}</p>);
    expect(screen.getByText(/3 keys in Atlas;/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "see them" })).toHaveAttribute("href", "/keys");
  });
});
