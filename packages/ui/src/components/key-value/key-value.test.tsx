import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { KeyValue } from "./key-value.js";

const items = [
  { label: "Format", value: "OpenAI Responses → Anthropic Messages" },
  { label: "Upstream request", value: "req_011Cg1rpXo2L5DAnHsAAaHg2", mono: true, copy: "req_011Cg1rpXo2L5DAnHsAAaHg2" },
  { label: "Message", value: "The connection to the provider failed before the response was complete.", wrap: true, span: "row" as const },
];

describe("KeyValue", () => {
  it("lays label and value side by side by default", () => {
    const { container } = render(<KeyValue items={items} />);
    expect(container.querySelector("dl")).toHaveClass("grid-cols-kv");
    expect(screen.getByText("Format").tagName).toBe("DT");
  });

  it("puts pairs in columns that follow its own width in the grid layout", () => {
    const { container } = render(<KeyValue layout="grid" columns={4} items={items} />);
    expect(container.firstElementChild).toHaveClass("@container");
    const dl = container.querySelector("dl");
    expect(dl).toHaveClass("grid", "grid-cols-2", "@kv:grid-cols-4");
    expect(screen.getByText("Message").closest("dt")?.parentElement).toHaveClass("col-span-full");
    expect(screen.getByText("Format").closest("dt")?.nextElementSibling).toHaveClass("min-h-24");
  });

  it("keeps a long text value on one line and copies what it says", () => {
    render(<KeyValue layout="grid" items={items} />);
    expect(screen.getByText("OpenAI Responses → Anthropic Messages")).toHaveClass("truncate");
    expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument();
    expect(screen.getByText(/The connection to the provider/)).toHaveClass("break-words");
  });

  it("keeps its rows at their own height beside a taller neighbour instead of stretching to fill", () => {
    const { container } = render(<KeyValue items={items} />);
    expect(container.querySelector("dl")).toHaveClass("content-start", "self-start");
  });

  it("lets a cut value of any kind, and a cut grid label, be read in full from the keyboard", () => {
    const spy = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(999);
    render(<KeyValue layout="grid" items={[{ label: "Upstream request identifier", value: <span>req_011Cg1rpXo2L5DAnHsAAaHg2</span> }]} />);
    expect(screen.getByText("Upstream request identifier")).toHaveAttribute("tabindex", "0");
    expect(screen.getByText("req_011Cg1rpXo2L5DAnHsAAaHg2").parentElement).toHaveAttribute("tabindex", "0");
    spy.mockRestore();
  });
});
