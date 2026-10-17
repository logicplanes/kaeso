import { describe, expect, it } from "vitest";
import type { Block, Inline } from "../../src/markdown/ast.ts";
import { compile, fenceInfo, lex, safeExternal, type Resolver, type Target } from "./compile.ts";

function md(body: string, targets: Record<string, Target> = {}) {
  const lexed = lex(body);
  const resolver: Resolver = {
    anchors: new Set(lexed.headings.map((h) => h.id)),
    resolve: (href) => targets[href] ?? { kind: "problem", message: `links to ${href}, which doesn't exist` },
  };
  return compile(lexed, resolver);
}
const json = (x: unknown) => JSON.stringify(x);
const texts = (bs: Block[]): string => json(bs);

describe("safety", () => {
  it("never keeps raw HTML: tags stay text", () => {
    const out = md(
      '<img src=x onerror="alert(1)"> and <script>alert(2)</script>\n\n<div onclick="x()">block</div>\n\nA <b>bold</b> <iframe src="https://example.com"></iframe>',
    );
    const s = texts(out.blocks);
    expect(s).toContain('<img src=x onerror=\\"alert(1)\\">');
    expect(s).toContain("<script>alert(2)</script>");
    expect(s).toContain("<div onclick=");
    for (const b of out.blocks) expect(b.t).toBe("paragraph");
    const kinds = new Set<string>();
    const walk = (xs: Inline[]) => xs.forEach((x) => (kinds.add(x.t), "c" in x && walk(x.c)));
    for (const b of out.blocks) if (b.t === "paragraph") walk(b.c);
    expect([...kinds]).toEqual(["text"]);
  });

  it("drops javascript:, data:, vbscript: and file: links, however they are written", () => {
    for (const href of [
      "javascript:alert(1)",
      "JaVaScRiPt:alert(1)",
      " javascript:alert(1)",
      "java&#x0A;script:alert(1)",
      "data:text/html,<b>x</b>",
      "vbscript:msgbox",
      "file:///etc/passwd",
      "//evil.example.com/x",
    ]) {
      const out = md(`[click](${href})`);
      expect(json(out.blocks)).not.toContain('"t":"link"');
      expect(json(out.blocks)).toContain("click");
      expect(out.problems.length).toBe(1);
    }
    const tab = md("[click](java\tscript:alert(1))");
    expect(json(tab.blocks)).not.toContain('"t":"link"');
  });

  it("keeps http, https and mailto links, normalized", () => {
    const out = md("[a](https://kaeso.app/docs) [b](mailto:ops@example.com) <https://kaeso.app> www.kaeso.app");
    const links = json(out.blocks).match(/"href":"[^"]*"/g);
    expect(links).toEqual([
      '"href":"https://kaeso.app/docs"',
      '"href":"mailto:ops@example.com"',
      '"href":"https://kaeso.app/"',
      '"href":"http://www.kaeso.app/"',
    ]);
    expect(out.problems).toEqual([]);
  });

  it("shows only images from content/, and asks for alt text", () => {
    const remote = md("![chart](https://example.com/c.png)");
    expect(json(remote.blocks)).not.toContain('"t":"img"');
    expect(remote.problems[0]).toMatch(/from outside content/);
    const local = md("![](./flow.png)", { "./flow.png": { kind: "file", ref: "content/flow.png", width: 640, height: 120 } });
    expect(json(local.blocks)).toContain('"t":"img","src":"content/flow.png","alt":"","width":640,"height":120');
    expect(local.problems[0]).toMatch(/no alt text/);
  });

  it("reports a link to a missing page and draws its words", () => {
    const out = md("See [keys](./keys.md).", { "./keys.md": { kind: "problem", message: "links to ./keys.md, which doesn't exist" } });
    expect(out.problems).toEqual(["links to ./keys.md, which doesn't exist"]);
    expect(json(out.blocks)).toContain('"v":"See keys."');
  });
});

describe("links", () => {
  it("rewrites links to pages and checks anchors on this page", () => {
    const out = md("## Rate limits\n\n[up](#rate-limits) [gone](#nope) [page](./keys.md#limits)", {
      "./keys.md#limits": { kind: "page", href: "/guides/keys#limits" },
    });
    const s = json(out.blocks);
    expect(s).toContain('"kind":"anchor","href":"#rate-limits"');
    expect(s).toContain('"kind":"page","href":"/guides/keys#limits"');
    expect(out.problems).toEqual(["links to #nope, which is not a heading on this page"]);
  });

  it("keeps reference links as text in pages, and follows them only where references are asked for", () => {
    const body = "See [the logs][logs].\n\n[logs]: https://docs.kaeso.app/console/request-logs";
    const resolver: Resolver = { anchors: new Set(), resolve: () => ({ kind: "problem", message: "no" }) };
    const page = compile(lex(body), resolver);
    expect(page.blocks[0]).toEqual({ t: "paragraph", c: [{ t: "text", v: "See [the logs][logs]." }] });
    expect(json(page.blocks[1])).toContain('"v":"[logs]: "');
    const changelog = compile(lex(body, { references: true }), resolver);
    expect(changelog.blocks).toEqual([
      {
        t: "paragraph",
        c: [
          { t: "text", v: "See " },
          { t: "link", kind: "external", href: "https://docs.kaeso.app/console/request-logs", c: [{ t: "text", v: "the logs" }] },
          { t: "text", v: "." },
        ],
      },
    ]);
  });
});

describe("blocks", () => {
  it("gives headings GitHub's anchors and a table of contents of ## and ###", () => {
    const out = md("## Create a key\n\n### From an SDK\n\n#### Python\n\n## Limits\n\n## Limits");
    expect(out.toc).toEqual([
      { id: "create-a-key", label: "Create a key", depth: 0 },
      { id: "from-an-sdk", label: "From an SDK", depth: 1 },
      { id: "limits", label: "Limits", depth: 0 },
      { id: "limits-1", label: "Limits", depth: 0 },
    ]);
    expect(out.blocks.filter((b) => b.t === "heading").map((b) => (b.t === "heading" ? b.level : 0))).toEqual([2, 3, 4, 2, 2]);
  });

  it("drops a leading # heading (the title comes from the frontmatter) and reports any other", () => {
    const lexed = lex("# Keys\n\nText\n\n# Again");
    expect(lexed.h1).toBe("Keys");
    const out = compile(lexed, { anchors: new Set(), resolve: () => ({ kind: "page", href: "/" }) });
    expect(out.blocks.map((b) => b.t)).toEqual(["paragraph"]);
    expect(out.problems[0]).toMatch(/`# Again` heading below the top/);
  });

  it("turns GitHub's alerts into callouts and other quotes into quotes", () => {
    const out = md("> [!WARNING]\n> Budgets refuse requests.\n\n> [!tip]\n> Lowercase works.\n\n> A plain quote.\n\n> [!NOTE] on the same line is a quote");
    expect(out.blocks.map((b) => (b.t === "callout" ? `callout:${b.kind}` : b.t))).toEqual(["callout:warning", "callout:tip", "quote", "quote"]);
    expect(json(out.blocks[0])).toContain("Budgets refuse requests.");
    expect(json(out.blocks[0])).not.toContain("[!WARNING]");
  });

  it("keeps lists, task lists, tables, code and rules", () => {
    const out = md(
      '3. three\n4. four\n\n- [x] done\n- [ ] open\n\n| Model | Calls |\n|---|--:|\n| fast | 12 |\n\n```py title="client.py"\nprint(1)\n```\n\n---',
    );
    expect(out.blocks.map((b) => b.t)).toEqual(["list", "list", "table", "code", "rule"]);
    const [ordered, tasks, table, code] = out.blocks;
    expect(ordered).toMatchObject({ ordered: true, start: 3 });
    expect(tasks).toMatchObject({
      items: [
        { task: true, checked: true },
        { task: true, checked: false },
      ],
    });
    expect(table).toMatchObject({ align: ["start", "end"] });
    expect(code).toEqual({ t: "code", code: "print(1)", lang: "python", title: "client.py" });
  });

  it("collects the page's words for the search", () => {
    expect(md("## Rate limits\n\nRequests per **minute**.").words).toEqual(["rate", "limits", "requests", "per", "minute"]);
  });
});

describe("fenceInfo and safeExternal", () => {
  it("maps languages to the highlighted ones, the rest to text", () => {
    expect(fenceInfo("sh")).toEqual({ lang: "bash" });
    expect(fenceInfo("TypeScript title='a.ts'")).toEqual({ lang: "ts", title: "a.ts" });
    expect(fenceInfo("toml")).toEqual({ lang: "text" });
    expect(fenceInfo(undefined)).toEqual({ lang: "text" });
  });

  it("allows http, https and mailto only", () => {
    expect(safeExternal(" https://kaeso.app ")).toBe("https://kaeso.app/");
    for (const bad of ["javascript:alert(1)", "data:text/html,x", "/relative", "#top", "ftp://x"]) expect(safeExternal(bad)).toBeNull();
  });
});
