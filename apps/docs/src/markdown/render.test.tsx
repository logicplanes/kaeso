import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Block } from "./ast";
import { Markdown } from "./render";

const draw = (blocks: Block[]) => render(<Markdown blocks={blocks} />).container;

describe("Markdown", () => {
  it("draws headings with their anchors, paragraphs and inline formatting", () => {
    const c = draw([
      { t: "heading", level: 2, id: "create-a-key", c: [{ t: "text", v: "Create a key" }] },
      {
        t: "paragraph",
        c: [
          { t: "strong", c: [{ t: "text", v: "Keys" }] },
          { t: "text", v: " are " },
          { t: "em", c: [{ t: "text", v: "cheap" }] },
          { t: "code", v: "sk-kso-…" },
        ],
      },
    ]);
    expect(screen.getByRole("heading", { level: 2, name: "Create a key" })).toHaveAttribute("id", "create-a-key");
    expect(c.querySelector("strong")).toHaveTextContent("Keys");
    expect(c.querySelector("em")).toHaveTextContent("cheap");
    expect(c.querySelector("code")).toHaveTextContent("sk-kso-…");
  });

  it("lets long identifiers wrap only after their separators, and keeps their text", () => {
    const c = draw([
      {
        t: "paragraph",
        c: [
          { t: "code", v: "sampling.max_output_tokens" },
          { t: "code", v: "Role::System" },
          { t: "code", v: "/v1/messages" },
        ],
      },
    ]);
    const [field, role, path] = [...c.querySelectorAll("code")];
    expect(field).toHaveTextContent("sampling.max_output_tokens");
    expect([...(field?.childNodes ?? [])].map((n) => (n.nodeName === "WBR" ? "|" : n.textContent)).join("")).toBe("sampling.|max_|output_|tokens");
    expect([...(role?.childNodes ?? [])].map((n) => (n.nodeName === "WBR" ? "|" : n.textContent)).join("")).toBe("Role::|System");
    expect([...(path?.childNodes ?? [])].map((n) => (n.nodeName === "WBR" ? "|" : n.textContent)).join("")).toBe("/v1/|messages");
  });

  it("renders text that looks like HTML as text", () => {
    const c = draw([{ t: "paragraph", c: [{ t: "text", v: '<img src=x onerror="alert(1)"><script>alert(2)</script>' }] }]);
    expect(c.querySelector("img, script")).toBeNull();
    expect(c.textContent).toContain('<img src=x onerror="alert(1)">');
  });

  it("links pages in place and other sites in a new tab", () => {
    draw([
      {
        t: "paragraph",
        c: [
          { t: "link", kind: "page", href: "/guides/keys#limits", c: [{ t: "text", v: "limits" }] },
          { t: "link", kind: "external", href: "https://kaeso.app/", c: [{ t: "text", v: "kaeso.app" }] },
          { t: "link", kind: "anchor", href: "#budgets", c: [{ t: "text", v: "budgets" }] },
        ],
      },
    ]);
    expect(screen.getByRole("link", { name: "limits" })).toHaveAttribute("href", "/guides/keys#limits");
    expect(screen.getByRole("link", { name: "limits" })).not.toHaveAttribute("target");
    expect(screen.getByRole("link", { name: /kaeso\.app/ })).toHaveAttribute("target", "_blank");
    expect(screen.getByRole("link", { name: /kaeso\.app/ })).toHaveAttribute("rel", "noopener noreferrer");
    expect(screen.getByRole("link", { name: "budgets" })).toHaveAttribute("href", "#budgets");
  });

  it("opens a bare address in place, as the text it is", () => {
    draw([
      { t: "paragraph", c: [{ t: "link", kind: "external", href: "https://kaeso.example.com/v1", c: [{ t: "text", v: "https://kaeso.example.com/v1" }] }] },
    ]);
    const link = screen.getByRole("link", { name: "https://kaeso.example.com/v1" });
    expect(link).not.toHaveAttribute("target");
    expect(link.querySelector("svg")).toBeNull();
  });

  it("gives images their size and alt text", () => {
    draw([{ t: "paragraph", c: [{ t: "img", src: "/assets/flow.svg", alt: "The request flow", width: 640, height: 120 }] }]);
    const img = screen.getByRole("img", { name: "The request flow" });
    expect(img).toHaveAttribute("width", "640");
    expect(img).toHaveAttribute("height", "120");
    expect(img).toHaveAttribute("loading", "lazy");
  });

  it("draws callouts with their kind as the title, and quotes", () => {
    const c = draw([
      { t: "callout", kind: "warning", c: [{ t: "paragraph", c: [{ t: "text", v: "Budgets refuse requests." }] }] },
      { t: "callout", kind: "caution", c: [{ t: "paragraph", c: [{ t: "text", v: "Archiving can't be undone." }] }] },
      { t: "quote", c: [{ t: "paragraph", c: [{ t: "text", v: "Keys are cheap." }] }] },
    ]);
    expect(screen.getByText("Warning")).toBeInTheDocument();
    expect(screen.getByText("Caution")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(c.querySelector("blockquote")).toHaveTextContent("Keys are cheap.");
  });

  it("numbers ordered lists from their start and shows task items as read-only checkboxes", () => {
    draw([
      { t: "list", ordered: true, start: 3, items: [{ c: [{ t: "paragraph", tight: true, c: [{ t: "text", v: "three" }] }] }] },
      {
        t: "list",
        ordered: false,
        start: 1,
        items: [{ task: true, checked: true, c: [{ t: "paragraph", tight: true, c: [{ t: "text", v: "Set an expiry" }] }] }],
      },
    ]);
    expect(screen.getAllByRole("list")[0]?.tagName).toBe("OL");
    expect(screen.getByText("3.")).toBeInTheDocument();
    const box = screen.getByRole("checkbox", { name: "Set an expiry" });
    expect(box).toBeChecked();
    expect(box).toHaveAttribute("aria-readonly", "true");
  });

  it("draws tables in a region that scrolls sideways", () => {
    draw([
      {
        t: "table",
        align: ["start", "end"],
        head: [[{ t: "text", v: "Level" }], [{ t: "text", v: "Requests" }]],
        rows: [[[{ t: "text", v: "Key" }], [{ t: "text", v: "60" }]]],
      },
    ]);
    expect(screen.getByRole("columnheader", { name: "Requests" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "60" })).toBeInTheDocument();
  });
});
