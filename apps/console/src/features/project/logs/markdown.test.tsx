import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LONG_TEXT, Markdown, lex, safeHref } from "./markdown";

const md = (text: string) => render(<Markdown text={text} />).container;

describe("Markdown", () => {
  it("draws headings, paragraphs, emphasis and inline code", () => {
    const c = md("# Harness\n\nYou are **careful** and _brief_. Use `grep`.\nA second line.");
    expect(screen.getByRole("heading", { name: "Harness", level: 3 })).toBeInTheDocument();
    expect(c.querySelector("strong")).toHaveTextContent("careful");
    expect(c.querySelector("em")).toHaveTextContent("brief");
    expect(c.querySelector("code")).toHaveTextContent("grep");
    expect(c.querySelector("p br")).not.toBeNull();
  });

  it("nests lists and numbers ordered ones from their start", () => {
    const c = md("- one\n  - nested\n    1. deep\n- two\n\n3. three\n4. four");
    const outer = c.querySelector("ul");
    expect(outer?.children).toHaveLength(2);
    expect(outer?.querySelector("li ul li")).toHaveTextContent("nested");
    expect(outer?.querySelector("li ul li ol li")).toHaveTextContent("deep");
    const ordered = [...c.querySelectorAll("ol")].at(-1);
    expect(ordered?.textContent).toContain("3.three");
    expect(ordered?.textContent).toContain("4.four");
  });

  it("keeps a tight list's items as lines and a loose list's items as paragraphs", () => {
    const tight = md("- plain **bold** item\n- second").querySelector("ul");
    expect(tight?.querySelector("p")).toBeNull();
    expect(tight?.querySelector("li strong")).toHaveTextContent("bold");
    expect(tight?.children[0]).toHaveTextContent("plain bold item");
    const loose = md("- first\n\n- second").querySelector("ul");
    expect([...(loose?.querySelectorAll("li p") ?? [])].map((p) => p.textContent)).toEqual(["first", "second"]);
  });

  it("shows fenced code as it was written", () => {
    const c = md("```python\nif a < b:\n    print('<b>hi</b>')\n```");
    expect(c.textContent).toContain("print('<b>hi</b>')");
    expect(c.querySelector("b")).toBeNull();
  });

  it("never renders raw HTML: tags stay text", () => {
    const c = md(
      '<system-reminder>\nStay calm.\n</system-reminder>\n\n<img src=x onerror="alert(1)"> and <script>alert(2)</script>\n\n<div onclick="x()">block</div>',
    );
    expect(c.querySelector("img")).toBeNull();
    expect(c.querySelector("script")).toBeNull();
    expect(c.querySelector("[onerror], [onclick]")).toBeNull();
    expect(c.querySelector("system-reminder")).toBeNull();
    expect(c.textContent).toContain("<system-reminder>");
    expect(c.textContent).toContain('<img src=x onerror="alert(1)">');
    expect(c.textContent).toContain("<script>alert(2)</script>");
  });

  it("opens only http, https and mailto links, in a new tab without referrer", () => {
    const c = md(
      "[ok](https://kaeso.example.com/docs) [mail](mailto:ops@example.com) [bad](javascript:alert(1)) [data](data:text/html,<b>x</b>) [rel](/orgs) [vb](vbscript:msgbox)",
    );
    const links = [...c.querySelectorAll("a")];
    expect(links.map((a) => a.getAttribute("href"))).toEqual(["https://kaeso.example.com/docs", "mailto:ops@example.com"]);
    for (const a of links) {
      expect(a).toHaveAttribute("target", "_blank");
      expect(a).toHaveAttribute("rel", "noopener noreferrer nofollow");
    }
    expect(c.textContent).toContain("bad");
    expect(c.innerHTML).not.toContain("javascript:");
  });

  it("shows images as links and never loads them", () => {
    const c = md("![chart](https://example.com/c.png) ![x](javascript:alert(1))");
    expect(c.querySelector("img")).toBeNull();
    expect(c.querySelector("a")).toHaveAttribute("href", "https://example.com/c.png");
    expect(c.innerHTML).not.toContain("javascript:");
  });

  it("draws quotes, rules and tables", () => {
    const c = md("> quoted\n\n---\n\n| Model | Calls |\n|---|--:|\n| fast | 12 |");
    expect(c.querySelector("blockquote")).toHaveTextContent("quoted");
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Model" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "12" })).toBeInTheDocument();
  });

  it("shows the start of a long text until Show all", async () => {
    const para = "A paragraph of the system prompt that goes on.\n\n";
    const text = `# Start\n\n${para.repeat(Math.ceil((LONG_TEXT * 2) / para.length))}# End`;
    const c = md(text);
    expect(screen.getByRole("heading", { name: "Start" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "End" })).toBeNull();
    expect(c.textContent?.length).toBeLessThan(LONG_TEXT);
    await userEvent.click(screen.getByRole("button", { name: /^Show all/ }));
    expect(screen.getByRole("heading", { name: "End" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("leaves a heading cut off from its text out of the preview", async () => {
    md(`${"Words of the prompt. ".repeat(750)}\n\n## Dangling\n\n${"More words. ".repeat(1000)}`);
    expect(screen.queryByRole("heading", { name: "Dangling" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: /^Show all/ }));
    expect(screen.getByRole("heading", { name: "Dangling" })).toBeInTheDocument();
  });

  it("keeps the first block of a preview that holds only rules", () => {
    const c = md(`${"---\n\n".repeat(3200)}${"Tail words. ".repeat(1000)}`);
    expect(c.querySelectorAll("[role=separator], hr")).toHaveLength(1);
  });
});

describe("Markdown, the rest", () => {
  it("keeps escapes, strikes through deletions and marks task items", () => {
    const c = md("\\*not emphasis\\* and ~~gone~~\n\n- [x] shipped\n- [ ] refunded");
    expect(c.querySelector("em")).toBeNull();
    expect(c.textContent).toContain("*not emphasis*");
    expect(c.querySelector("del")).toHaveTextContent("gone");
    expect(c.textContent).toContain("[x]shipped");
    expect(c.textContent).toContain("[ ]refunded");
  });

  it("draws small headings for small text", () => {
    render(<Markdown text={"# Big\n\n### Small"} size="small" tone="secondary" />);
    expect(screen.getByRole("heading", { name: "Big", level: 3 })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Small", level: 5 })).toBeInTheDocument();
  });

  it("reads the same text once and forgets the oldest", () => {
    const first = lex("cached once");
    expect(lex("cached once")).toBe(first);
    for (let i = 0; i < 64; i++) lex(`filler ${i}`);
    expect(lex("cached once")).not.toBe(first);
  });
});

describe("Markdown, edge cases", () => {
  it("shows code in an unknown or missing language as plain text", () => {
    const c = md("```rust extra\nfn main() {}\n```\n\n    indented();");
    expect(c.textContent).toContain("fn main() {}");
    expect(c.textContent).toContain("indented();");
  });

  it("keeps a link's title and shows an image without alt text by its address", () => {
    const c = md('[docs](https://kaeso.example.com/docs "Kaeso docs") ![](https://example.com/c.png)');
    const links = [...c.querySelectorAll("a")];
    expect(links[0]).toHaveAttribute("title", "Kaeso docs");
    expect(links[1]).not.toHaveAttribute("title");
    expect(links[1]).toHaveTextContent("https://example.com/c.png");
  });

  it("marks task items in a loose list once", () => {
    const c = md("- [x] shipped\n\n- [ ] refunded");
    expect(c.querySelectorAll("li")).toHaveLength(2);
    expect(c.querySelector("input")).toBeNull();
    expect(c.textContent).toBe("[x]shipped[ ]refunded");
  });

  it("draws a minor heading in body text", () => {
    md("### Details");
    expect(screen.getByRole("heading", { name: "Details", level: 5 })).toBeInTheDocument();
  });

  it("cuts a long text without paragraphs at a fixed length", async () => {
    const text = "word ".repeat(Math.ceil((LONG_TEXT * 1.5) / 5));
    const c = md(text);
    expect(c.textContent?.length).toBeLessThan(LONG_TEXT);
    expect(c.textContent).toContain("Show all");
    await userEvent.click(screen.getByRole("button", { name: /^Show all/ }));
    expect(c.textContent?.trim()).toBe(text.trim());
  });

  it("doesn't end the preview on a heading that would have nothing under it", () => {
    const para = "A paragraph of the system prompt that goes on.\n\n";
    const text = `${para.repeat(Math.floor(15_000 / para.length))}## Cut here\n\n${"word ".repeat(Math.ceil(LONG_TEXT / 5))}`;
    md(text);
    expect(screen.queryByRole("heading", { name: "Cut here" })).toBeNull();
    expect(screen.getByRole("button", { name: /^Show all/ })).toBeInTheDocument();
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
    ])
      expect(safeHref(bad)).toBeNull();
  });
});
