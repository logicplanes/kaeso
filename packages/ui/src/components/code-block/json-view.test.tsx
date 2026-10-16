import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { JsonView } from "./json-view.js";

const body = {
  model: "gpt-5",
  stream: true,
  temperature: 0.2,
  user: null,
  messages: [{ role: "user", content: "Summarise the checkout logs" }],
};

describe("JsonView", () => {
  it("shows every kind of value and collapses below the default depth", () => {
    render(<JsonView value={body} defaultDepth={1} />);
    const view = screen.getByLabelText("JSON");
    expect(view).toHaveTextContent('"model": "gpt-5"');
    expect(view).toHaveTextContent('"stream": true');
    expect(view).toHaveTextContent('"temperature": 0.2');
    expect(view).toHaveTextContent('"user": null');
    expect(screen.getByRole("button", { name: "1 item" })).toBeInTheDocument();
  });

  it("expands and collapses a branch, and counts keys", async () => {
    render(<JsonView value={{ usage: { input: 12, output: 40 } }} defaultDepth={0} surface="plain" />);
    await userEvent.click(screen.getByRole("button", { name: "1 key" }));
    expect(screen.getByRole("button", { name: "2 keys" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Expand usage" }));
    expect(screen.getByRole("region", { name: "JSON" })).toHaveTextContent('"input": 12');
    expect(screen.getByRole("button", { name: "Collapse usage" })).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(screen.getByRole("button", { name: "Collapse" }));
    expect(screen.getByRole("button", { name: "1 key" })).toBeInTheDocument();
  });

  it("highlights the query in keys and strings", () => {
    const { container } = render(<JsonView value={body} defaultDepth={3} query="CHECK" />);
    expect([...container.querySelectorAll("mark")].map((m) => m.textContent)).toEqual(["check"]);
    render(<JsonView value={{ checkout: "no match here" }} query="checkout" />);
    expect(screen.getAllByText("checkout", { selector: "mark" })).toHaveLength(1);
  });

  it("copies the path of a value", async () => {
    const user = userEvent.setup();
    render(<JsonView value={body} defaultDepth={3} />);
    const row = screen.getByText((_, el) => el?.tagName === "SPAN" && el.textContent === '"content": "Summarise the checkout logs"').closest("div")!;
    await user.click(within(row).getByRole("button", { name: "Copy path messages[0].content" }));
    expect(await navigator.clipboard.readText()).toBe("messages[0].content");
    expect(within(row).getByRole("button", { name: "Path copied" })).toBeInTheDocument();
  });
});
