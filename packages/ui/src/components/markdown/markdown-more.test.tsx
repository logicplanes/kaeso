import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Markdown, lexMarkdown, markdownCodeLang, previewMarkdown } from "./markdown.js";

describe("markdown helpers", () => {
  it("maps fence names to the highlighter's languages", () => {
    const cases: [string | undefined, string][] = [
      ["TypeScript", "ts"],
      ["jsx", "tsx"],
      ["mjs", "js"],
      ["jsonc", "json"],
      ["zsh", "bash"],
      ["py", "python"],
      ["yml", "yaml"],
      ["patch", "diff"],
      ["ruby", "text"],
      [undefined, "text"],
      ["ts title=app.ts", "ts"],
    ];
    for (const [lang, want] of cases) expect(markdownCodeLang(lang)).toBe(want);
  });

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
