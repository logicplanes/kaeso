import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sankey } from "./sankey.js";

const nodes = [
  { id: "app", label: "Checkout assistant" },
  { id: "openai", label: "OpenAI" },
  { id: "anthropic", label: "Anthropic" },
];
const links = [
  { source: "app", target: "openai", value: 1200 },
  { source: "app", target: "anthropic", value: 300 },
];

afterEach(() => vi.restoreAllMocks());

describe("Sankey", () => {
  it("draws a node per id and a link per flow, and lights the links of a hovered node", () => {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(640);
    const { container } = render(<Sankey aria-label="Routes" nodes={nodes} links={links} height={240} />);
    expect(container.querySelectorAll("rect")).toHaveLength(3);
    const paths = container.querySelectorAll("path");
    expect(paths).toHaveLength(2);
    expect(paths[0]).toHaveAttribute("stroke-opacity", "0.24");
    expect(screen.getByText("Checkout assistant → OpenAI: 1,200")).toBeInTheDocument();
    fireEvent.mouseEnter(container.querySelectorAll("rect")[2]!.parentElement!);
    const lit = [...container.querySelectorAll("path")].map((p) => p.getAttribute("stroke-opacity"));
    expect(lit.sort()).toEqual(["0.06", "0.5"]);
    fireEvent.mouseLeave(container.querySelectorAll("rect")[2]!.parentElement!);
    expect(container.querySelectorAll("path")[0]).toHaveAttribute("stroke-opacity", "0.24");
  });

  it("draws nothing before it has a width and offers the flows as a table", async () => {
    const { container } = render(<Sankey aria-label="Routes" nodes={nodes} links={links} formatValue={(v) => `${v} req`} />);
    expect(container.querySelector("svg")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "View as table" }));
    expect(screen.getByRole("table")).toHaveTextContent("Checkout assistantAnthropic300 req");
  });

  it("names an unknown node by its id in the table", async () => {
    render(<Sankey aria-label="Routes" nodes={[]} links={[{ source: "a", target: "b", value: 1 }]} />);
    await userEvent.click(screen.getByRole("button", { name: "View as table" }));
    expect(screen.getByRole("table")).toHaveTextContent("ab1");
  });
});
