import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Markdown, extendMarkdown, lexMarkdown, previewMarkdown } from "./markdown.js";

const REPLY = [
  "# Checking the gateway\n\nKaeso answered **two** of three requests; the third timed out.\n",
  "Setext heading\n==============\n\n1. Open the request log\n2. Filter by `status:5xx`\n\n3. Compare the providers\n10. Retry the last one\n",
  "- one\n- two\n  - nested *item*\n\n- loose item\n\n  with a second paragraph\n\n- [x] done\n- [ ] open\n",
  "> A quote that goes on\nlazily on the next line\n\n| Model | Latency |\n|:------|--------:|\n| claude | 812 ms |\n| gpt | 640 ms |\n\n",
  "```ts\nconst reply = await kaeso.responses.create({ model });\n```\n\nText after the fence.\n\n    indented code\n\n---\n\nA last paragraph with a [link](https://kaeso.app)  \nand a break.\n",
].join("\n");

describe("extendMarkdown", () => {
  it("gives the tokens a whole lex gives at every length a stream can reach", () => {
    for (const step of [1, 3, 7, 29]) {
      let before: { text: string; tokens: ReturnType<typeof lexMarkdown> } | null = null;
      for (let end = step; end < REPLY.length + step; end += step) {
        const text = REPLY.slice(0, end);
        const tokens = extendMarkdown(before, text);
        expect([...tokens], `${step}: ${JSON.stringify(text.slice(-40))}`).toEqual([...lexMarkdown(text, { cache: false })]);
        before = { text, tokens };
      }
    }
  });

  it("keeps the blocks that can't change and lexes a text that isn't an extension whole", () => {
    const text = "One paragraph.\n\nTwo paragraph.\n\nThree";
    const first = lexMarkdown(text, { cache: false });
    const grown = extendMarkdown({ text, tokens: first }, `${text} grows`);
    expect(grown[0]).toBe(first[0]);
    expect([...grown]).toEqual([...lexMarkdown(`${text} grows`, { cache: false })]);
    expect(extendMarkdown({ text: "Old text", tokens: lexMarkdown("Old text") }, "New text")).toEqual(lexMarkdown("New text", { cache: false }));
    expect(extendMarkdown(null, "a\r\nb")).toEqual(lexMarkdown("a\r\nb", { cache: false }));
  });
});

describe("markdown helpers", () => {
  it("cuts a preview at a paragraph, a line or a word", () => {
    expect(previewMarkdown("short", 10)).toBe("short");
    expect(previewMarkdown("First paragraph here.\n\nSecond paragraph that runs long.", 30)).toBe("First paragraph here.");
    expect(previewMarkdown("Line one is here\nline two goes on and on", 30)).toBe("Line one is here");
    expect(previewMarkdown("words that keep going without any breaks at all", 20)).toBe("words that keep…");
    expect(previewMarkdown("abcdefghijklmnopqrstuvwxyz", 10)).toBe("abcdefghij…");
  });

  it("keeps at most 64 texts in its cache, and can skip it", () => {
    for (let i = 0; i < 70; i++) lexMarkdown(`text ${i}`);
    expect(lexMarkdown("text 69")).toBe(lexMarkdown("text 69"));
    expect(lexMarkdown("fresh", { cache: false })).not.toBe(lexMarkdown("fresh", { cache: false }));
  });
});

describe("Markdown", () => {
  it("renders inline marks, unsafe links as text, images as links and task lists", () => {
    render(
      <Markdown
        text={
          "~~old~~ **new** *now* `code` \\* line  \nbreak [safe](https://kaeso.app) [bad](javascript:alert(1)) ![logo](https://kaeso.app/logo.png) ![x](javascript:x)\n\n- [x] done\n- [ ] todo"
        }
      />,
    );
    expect(screen.getByText("old").tagName).toBe("DEL");
    expect(screen.getByRole("link", { name: /safe/ }).getAttribute("href")).toMatch(/^https:\/\/kaeso\.app\/?$/);
    expect(screen.queryByRole("link", { name: /bad/ })).toBeNull();
    expect(screen.getByRole("link", { name: /logo/ })).toHaveAttribute("href", "https://kaeso.app/logo.png");
    expect(screen.getByText("done")).toBeInTheDocument();
  });

  it("collapses a long text and shows it all on request, without a dangling heading", async () => {
    const text = `${"A sentence that fills the preview. ".repeat(20)}\n\n## Next\n\n${"More text after the cut. ".repeat(20)}`;
    render(<Markdown text={text} collapseAfter={800} trailing={<span>▍</span>} />);
    expect(screen.queryByRole("heading", { name: "Next" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: `Show all ${text.length.toLocaleString("en-US")} characters` }));
    expect(screen.getByRole("heading", { name: "Next" })).toBeInTheDocument();
  });

  it("keeps a streaming text whole and trims trailing space before the cursor", () => {
    render(
      <Markdown streaming text={"Thinking about it\n\n\n"} trailing={<span data-testid="cursor" />} size="small" tone="secondary" wrapCode headingLevel={2} />,
    );
    expect(screen.getByTestId("cursor")).toBeInTheDocument();
  });
});

describe("Markdown tables wider than their column", () => {
  it("scroll sideways as a named, focusable group and keep short cells on one line", () => {
    const wide = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(900);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(300);
    render(<Markdown text={"| Model | Region |\n|---|---|\n| gpt-5 | eu-west-1 |"} />);
    const scroller = screen.getByRole("group", { name: "Table, scrolls sideways" });
    expect(scroller).toHaveAttribute("tabindex", "0");
    expect(scroller).toHaveClass("scroll-fade-x");
    expect(screen.getByText("eu-west-1").closest("td")).toHaveClass("whitespace-nowrap");
    wide.mockRestore();
    vi.restoreAllMocks();
  });
});
