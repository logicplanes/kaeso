import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
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
    await userEvent.click(screen.getByRole("button", { name: "2 keys" }));
    expect(screen.getByRole("tree", { name: "JSON" })).toHaveTextContent('"input": 12');
    expect(screen.getByRole("treeitem", { name: /"usage"/ })).toHaveAttribute("aria-expanded", "true");
  });

  it("is one tab stop that the arrow keys walk, opening, closing and climbing back to the parent", async () => {
    const user = userEvent.setup();
    render(
      <>
        <JsonView value={{ usage: { input: 12, output: 40 }, model: "gpt-5" }} defaultDepth={1} />
        <button type="button">After</button>
      </>,
    );
    await user.tab();
    const root = screen.getAllByRole("treeitem")[0]!;
    expect(root).toHaveFocus();
    expect(root).toHaveAttribute("aria-level", "1");
    await user.keyboard("{ArrowDown}");
    const usage = screen.getByRole("treeitem", { name: /"usage"/ });
    expect(usage).toHaveFocus();
    expect(usage).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{ArrowRight}");
    expect(usage).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("treeitem", { name: /"input"/ })).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(usage).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(usage).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{End}");
    expect(screen.getByRole("treeitem", { name: /"model"/ })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    await user.tab({ shift: true });
    expect(screen.getByRole("treeitem", { name: /"model"/ })).toHaveFocus();
  });

  it("highlights the query in keys and strings", () => {
    const { container } = render(<JsonView value={body} defaultDepth={3} query="CHECK" />);
    expect([...container.querySelectorAll("mark")].map((m) => m.textContent)).toEqual(["check"]);
    render(<JsonView value={{ checkout: "no match here" }} query="checkout" />);
    expect(screen.getAllByText("checkout", { selector: "mark" })).toHaveLength(1);
  });

  it("copies the path of a value with the button or the keyboard, and says so", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<JsonView value={body} defaultDepth={3} />);
    const row = screen.getByRole("treeitem", { name: /"content"/ });
    await user.click(row.querySelector("button")!);
    expect(await navigator.clipboard.readText()).toBe("messages[0].content");
    expect(row.querySelector("button")).toHaveAttribute("title", "Path copied");
    await act(() => vi.advanceTimersByTimeAsync(60));
    expect(screen.getByRole("status")).toHaveTextContent("Path copied");
    const model = screen.getByRole("treeitem", { name: /"model"/ });
    model.focus();
    await user.keyboard("{Control>}c{/Control}");
    expect(await navigator.clipboard.readText()).toBe("model");
    vi.useRealTimers();
  });
});

describe("JsonView keys", () => {
  it("opens and closes with Enter and Space and walks back up", async () => {
    const user = userEvent.setup();
    render(<JsonView value={{ a: { b: 1 }, c: [1, 2] }} defaultDepth={1} />);
    await user.tab();
    await user.keyboard("{End}");
    const c = screen.getByRole("treeitem", { name: /"c"/ });
    expect(c).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(c).toHaveAttribute("aria-expanded", "true");
    await user.keyboard(" ");
    expect(c).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{ArrowUp}{Home}");
    expect(screen.getAllByRole("treeitem")[0]).toHaveFocus();
    await user.keyboard("{ArrowUp}x");
    expect(screen.getAllByRole("treeitem")[0]).toHaveFocus();
  });
});
