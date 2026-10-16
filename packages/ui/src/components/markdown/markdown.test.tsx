import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Markdown, markdownCodeLang, safeHref } from "./markdown";

const md = (text: string, props: Partial<Parameters<typeof Markdown>[0]> = {}) => render(<Markdown text={text} {...props} />).container;

describe("Markdown", () => {
  it("draws headings, paragraphs, emphasis and inline code", () => {
    const c = md("# Harness\n\nYou are **careful** and _brief_. Use `grep`.\nA second line.");
    expect(screen.getByRole("heading", { name: "Harness", level: 3 })).toBeInTheDocument();
    expect(c.querySelector("strong")).toHaveTextContent("careful");
    expect(c.querySelector("em")).toHaveTextContent("brief");
    expect(c.querySelector("code")).toHaveTextContent("grep");
    expect(c.querySelector("p br")).not.toBeNull();
  });

  it("starts headings at the level asked for", () => {
    md("# Plan\n\n## Steps", { headingLevel: 2 });
    expect(screen.getByRole("heading", { name: "Plan", level: 2 })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Steps", level: 3 })).toBeInTheDocument();
  });

  it("nests lists, numbers ordered ones from their start and marks tasks", () => {
    const c = md("- one\n  - nested\n    1. deep\n- two\n\n3. three\n4. four\n\n- [x] shipped\n- [ ] next");
    const outer = c.querySelector("ul");
    expect(outer?.children).toHaveLength(2);
    expect(outer?.querySelector("li ul li")).toHaveTextContent("nested");
    expect(outer?.querySelector("li ul li ol li")).toHaveTextContent("deep");
    const ordered = [...c.querySelectorAll("ol")].at(-1);
    expect(ordered?.textContent).toContain("3.three");
    expect(ordered?.textContent).toContain("4.four");
    expect(c.textContent).toContain("[x]shipped");
    expect(c.querySelector("input")).toBeNull();
  });

  it("shows fenced code as it was written, in a code block", () => {
    const c = md("```python\nif a < b:\n    print('<b>hi</b>')\n```");
    expect(screen.getByRole("region", { name: "Code" })).toBeInTheDocument();
    expect(c.textContent).toContain("print('<b>hi</b>')");
    expect(c.querySelector("b")).toBeNull();
  });

  it("never renders raw HTML: tags stay text", () => {
    const c = md(
      '<system-reminder>\nStay calm.\n</system-reminder>\n\n<img src=x onerror="alert(1)"> and <script>alert(2)</script>\n\n<div onclick="x()">block</div>\n\n<iframe src="https://example.com"></iframe>',
    );
    expect(c.querySelector("img, script, iframe, system-reminder")).toBeNull();
    expect(c.querySelector("[onerror], [onclick]")).toBeNull();
    expect(c.textContent).toContain("<system-reminder>");
    expect(c.textContent).toContain("</system-reminder>");
    expect(c.textContent).toContain('<img src=x onerror="alert(1)">');
    expect(c.textContent).toContain("<script>alert(2)</script>");
    expect(c.textContent).toContain('<iframe src="https://example.com"></iframe>');
  });

  it("keeps HTML inside emphasis, tables and lists literal too", () => {
    const c = md('**<img src=x onerror=alert(1)>**\n\n| a | b |\n|---|---|\n| <b>x</b> | <svg onload=alert(1)> |\n\n- <a href="javascript:alert(1)">x</a>');
    expect(c.querySelector("img, b, svg")).toBeNull();
    expect(c.querySelectorAll("a")).toHaveLength(0);
    expect(c.textContent).toContain("<svg onload=alert(1)>");
  });

  it("opens only http, https and mailto links, in a new tab without referrer", () => {
    const c = md(
      "[ok](https://kaeso.example.com/docs) [mail](mailto:ops@example.com) [bad](javascript:alert(1)) [data](data:text/html,<b>x</b>) [rel](/orgs) [vb](vbscript:msgbox) [ws](  JaVaScRiPt:alert(1)) <https://kaeso.example.com/auto>",
    );
    const links = [...c.querySelectorAll("a")];
    expect(links.map((a) => a.getAttribute("href"))).toEqual(["https://kaeso.example.com/docs", "mailto:ops@example.com", "https://kaeso.example.com/auto"]);
    for (const a of links) {
      expect(a).toHaveAttribute("target", "_blank");
      expect(a).toHaveAttribute("rel", "noopener noreferrer nofollow");
    }
    expect(c.textContent).toContain("bad");
    expect(c.innerHTML.toLowerCase()).not.toContain("javascript:");
    expect(c.innerHTML).not.toContain("vbscript:");
  });

  it("shows images as links and never loads them", () => {
    const c = md("![chart](https://example.com/c.png) ![x](javascript:alert(1)) ![](https://example.com/a.png)");
    expect(c.querySelector("img")).toBeNull();
    const links = [...c.querySelectorAll("a")];
    expect(links.map((a) => a.getAttribute("href"))).toEqual(["https://example.com/c.png", "https://example.com/a.png"]);
    expect(links[0]).toHaveTextContent("chart");
    expect(c.innerHTML).not.toContain("javascript:");
  });

  it("draws quotes, rules and tables", () => {
    const c = md("> quoted\n\n---\n\n| Model | Calls |\n|---|--:|\n| fast | 12 |");
    expect(c.querySelector("blockquote")).toHaveTextContent("quoted");
    expect(c.querySelector("[role=separator], hr")).not.toBeNull();
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Model" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "12" })).toBeInTheDocument();
  });

  it("shows the start of a long text until Show all", async () => {
    const para = "A paragraph of the system prompt that goes on.\n\n";
    const text = `# Start\n\n${para.repeat(1200)}# End`;
    const c = md(text, { collapseAfter: 20_000 });
    expect(screen.getByRole("heading", { name: "Start" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "End" })).toBeNull();
    expect(c.textContent?.length).toBeLessThan(20_000);
    await userEvent.click(screen.getByRole("button", { name: /^Show all/ }));
    expect(screen.getByRole("heading", { name: "End" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("never collapses while streaming or with collapseAfter 0", () => {
    const text = "word ".repeat(10_000);
    md(text, { collapseAfter: 1000, streaming: true });
    md(text, { collapseAfter: 0 });
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("follows a streaming text: blocks grow and the caret stays after the last word", () => {
    const caret = <span data-testid="caret" />;
    const { container, rerender } = render(<Markdown text="Checking the" streaming trailing={caret} />);
    expect(container.querySelector("p")?.lastElementChild).toBe(screen.getByTestId("caret"));
    rerender(<Markdown text={"Checking the logs.\n\n- first"} streaming trailing={caret} />);
    expect(container.querySelectorAll("p")).toHaveLength(1);
    expect(container.querySelector("p [data-testid=caret]")).toBeNull();
    expect(container.querySelector("li [data-testid=caret]")).not.toBeNull();
    rerender(<Markdown text={"Checking the logs.\n\n- first\n\n```bash\nkaeso logs"} streaming trailing={caret} />);
    expect(container.textContent).toContain("kaeso logs");
    expect(screen.queryAllByTestId("caret")).toHaveLength(0);
  });
});

describe("safeHref", () => {
  it("allows http, https and mailto only", () => {
    expect(safeHref(" https://kaeso.example.com ")).toBe("https://kaeso.example.com/");
    expect(safeHref("http://localhost:3000/x")).toBe("http://localhost:3000/x");
    expect(safeHref("mailto:a@b.c")).toBe("mailto:a@b.c");
    for (const bad of [
      "javascript:alert(1)",
      "JaVaScRiPt:alert(1)",
      " javascript:alert(1)",
      "java\nscript:alert(1)",
      "data:text/html,x",
      "/relative",
      "#top",
      "file:///etc/passwd",
      "vbscript:x",
    ])
      expect(safeHref(bad)).toBeNull();
  });
});

describe("markdownCodeLang", () => {
  it("maps fence names to the highlighter's languages", () => {
    expect(markdownCodeLang("TypeScript")).toBe("ts");
    expect(markdownCodeLang("sh title=run")).toBe("bash");
    expect(markdownCodeLang("rust")).toBe("text");
    expect(markdownCodeLang(undefined)).toBe("text");
  });
});
