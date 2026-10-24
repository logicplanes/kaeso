import { describe, expect, it } from "vitest";
import type { Block, Inline } from "../../src/markdown/ast.ts";
import { compile, fenceInfo, lex, safeExternal, type DiagramTarget, type Resolver, type Target } from "./compile.ts";

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

  it("keeps reference links as text", () => {
    const body = "See [the logs][logs].\n\n[logs]: https://docs.kaeso.app/console/request-logs";
    const resolver: Resolver = { anchors: new Set(), resolve: () => ({ kind: "problem", message: "no" }) };
    const page = compile(lex(body), resolver);
    expect(page.blocks[0]).toEqual({ t: "paragraph", c: [{ t: "text", v: "See [the logs][logs]." }] });
    expect(json(page.blocks[1])).toContain('"v":"[logs]: "');
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

describe("diagrams", () => {
  const drawn: DiagramTarget = {
    kind: "diagram",
    light: "diagrams/k.light.svg",
    dark: "diagrams/k.dark.svg",
    width: 364,
    height: 49,
    labels: ["App", "Kaeso gateway"],
  };
  const page = (body: string, target: DiagramTarget = drawn, offset = 4) => {
    const lexed = lex(body, { offset });
    const seen: { code: string; line: number }[] = [];
    const out = compile(lexed, {
      anchors: new Set(),
      resolve: () => ({ kind: "page", href: "/" }),
      diagram: (code, line) => (seen.push({ code, line }), target),
    });
    return { ...out, seen };
  };
  const FLOW = "```mermaid\nflowchart LR\n  accTitle: Route of a request\n  accDescr: An app calls Kaeso.\n  app[App] --> kaeso[Kaeso gateway]\n```";

  it("turns a mermaid block into a diagram with its drawings, title and description, and passes the fence's line in the file", () => {
    const out = page(`\nIntro.\n\n${FLOW}\n`);
    expect(out.problems).toEqual([]);
    expect(out.blocks[1]).toEqual({
      t: "diagram",
      light: "diagrams/k.light.svg",
      dark: "diagrams/k.dark.svg",
      width: 364,
      height: 49,
      title: "Route of a request",
      description: "An app calls Kaeso.",
    });
    expect(out.seen).toEqual([
      { code: "flowchart LR\n  accTitle: Route of a request\n  accDescr: An app calls Kaeso.\n  app[App] --> kaeso[Kaeso gateway]", line: 8 },
    ]);
    expect(out.words).toEqual(expect.arrayContaining(["route", "request", "app", "calls", "kaeso", "gateway"]));
    expect(out.words).not.toContain("flowchart");
  });

  it("finds the line of every diagram, in lists and after an identical one", () => {
    const out = page(`${FLOW}\n\n- Item\n\n  ${FLOW.replace(/\n/g, "\n  ")}\n\n${FLOW}\n`, drawn, 0);
    expect(out.seen.map((s) => s.line)).toEqual([1, 10, 17]);
  });

  it("keeps where a drawing's subject sits, so a wide diagram opens on it", () => {
    expect(page(FLOW, { ...drawn, focus: 0.18 }).blocks[0]).toMatchObject({ t: "diagram", focus: 0.18 });
    expect(page(FLOW).blocks[0]).not.toHaveProperty("focus");
  });

  it("names the file's line of a diagram without a title, with a comment, its own styles, or not drawn yet", () => {
    expect(page("```mermaid\nflowchart LR\n  a --> b\n```").problems).toEqual([
      ":5: the diagram has no `accTitle: …`: name it in a few words for screen readers and search",
    ]);
    expect(page("Text.\n\n```mermaid\n%%{init: {'theme': 'dark'}}%%\nflowchart LR\n  accTitle: A\n  a --> b\n```").problems).toEqual([
      ":7: the diagram has a `%%` line: diagrams take no comments and no `%%{init}`, the site sets their look",
    ]);
    expect(page("Text.\n\n```mermaid\nflowchart LR\n  accTitle: A\n  a:::accent --> b:::loud\n  style b fill:#f00\n```").problems).toEqual([
      ":7: the diagram styles itself: mark nodes with `:::accent` or `:::outside` alone, the site sets their look",
      ":7: the diagram marks a node `:::loud`: use `:::accent` or `:::outside`",
    ]);
    const missing = page(FLOW, { kind: "problem", message: "the diagram has changed since it was last drawn" });
    expect(missing.problems).toEqual([":5: the diagram has changed since it was last drawn"]);
    expect(missing.blocks).toEqual([]);
  });

  it("refuses a diagram where it can't be drawn, outside the pages in content/", () => {
    const out = compile(lex(FLOW), { anchors: new Set(), resolve: () => ({ kind: "page", href: "/" }) });
    expect(out.blocks).toEqual([]);
    expect(out.problems).toEqual([":1: diagrams are drawn only on the pages in content/; show this as text or a list"]);
  });

  it("fails a code block that draws a diagram with box-drawing characters or arrows, and keeps plain code", () => {
    const out = page(
      "Text.\n\n```text\napp ──▶ gateway\n         ▲\nconsole ─┘\n```\n\n```bash\ncurl -s https://kaeso.example.com/v1/models | jq '.data[] | .id'\n```\n\n```\nKPP → canonical\n```",
    );
    expect(out.problems).toEqual([
      ":7: the code block draws a diagram with box-drawing characters or arrows; write it as a `mermaid` block (development/documentation.md, Diagrams)",
      ":15: the code block draws a diagram with box-drawing characters or arrows; write it as a `mermaid` block (development/documentation.md, Diagrams)",
    ]);
    expect(out.blocks.map((b) => b.t)).toEqual(["paragraph", "code", "code", "code"]);
  });
});

describe("fenceInfo and safeExternal", () => {
  it("maps languages to the highlighted ones, the rest to text", () => {
    expect(fenceInfo("sh")).toEqual({ lang: "bash" });
    expect(fenceInfo("TypeScript title='a.ts'")).toEqual({ lang: "ts", title: "a.ts" });
    expect(fenceInfo("toml")).toEqual({ lang: "text" });
    expect(fenceInfo(undefined)).toEqual({ lang: "text" });
    expect(fenceInfo("markdown")).toEqual({ lang: "text", wrap: true });
    expect(fenceInfo("text title=policy")).toEqual({ lang: "text", title: "policy", wrap: true });
  });

  it("allows http, https and mailto only", () => {
    expect(safeExternal(" https://kaeso.app ")).toBe("https://kaeso.app/");
    for (const bad of ["javascript:alert(1)", "data:text/html,x", "/relative", "#top", "ftp://x"]) expect(safeExternal(bad)).toBeNull();
  });
});

describe("code groups", () => {
  const fence = (lang: string, tab: string, code: string) => `\`\`\`${lang} tab="${tab}"\n${code}\n\`\`\``;

  it("groups consecutive fences with a tab into one block, in their order", () => {
    const out = md(
      [
        "Call it:",
        fence("bash", "curl", "curl https://kaeso.example.com/v1/models"),
        fence("python", "Python", "print(1)"),
        fence("ts", "TypeScript", "console.log(1)"),
      ].join("\n\n"),
    );
    expect(out.problems).toEqual([]);
    expect(out.blocks).toEqual([
      { t: "paragraph", c: [{ t: "text", v: "Call it:" }] },
      {
        t: "codegroup",
        tabs: [
          { label: "curl", lang: "bash", code: "curl https://kaeso.example.com/v1/models" },
          { label: "Python", lang: "python", code: "print(1)" },
          { label: "TypeScript", lang: "ts", code: "console.log(1)" },
        ],
      },
    ]);
    expect(out.words).toEqual(expect.arrayContaining(["curl", "print", "console"]));
  });

  it("starts a new group after anything but a blank line, and leaves fences without a tab alone", () => {
    const out = md(
      [fence("bash", "curl", "a"), fence("python", "Python", "b"), "Or:", fence("bash", "curl", "c"), fence("python", "Python", "d"), "```json\n{}\n```"].join(
        "\n\n",
      ),
    );
    expect(out.problems).toEqual([]);
    expect(out.blocks.map((b) => b.t)).toEqual(["codegroup", "paragraph", "codegroup", "code"]);
  });

  it("names a lone tab, a repeated tab and a tab with a title, and still shows the code", () => {
    const lone = md(`Text.\n\n${fence("bash", "curl", "curl https://kaeso.example.com")}`);
    expect(lone.problems).toEqual([
      ':3: the fence with `tab="curl"` stands alone; a code group is two or more fences with `tab=` in a row, and a single block has no `tab=`',
    ]);
    expect(lone.blocks[1]).toEqual({ t: "code", lang: "bash", code: "curl https://kaeso.example.com" });
    const twice = md([fence("python", "Python", "a"), fence("python", "Python", "b")].join("\n\n"));
    expect(twice.problems).toEqual([':5: the code group has two tabs named "Python"; give each fence its own `tab=`']);
    const titled = md(['```ts tab="TypeScript" title="client.ts"\nx\n```', fence("python", "Python", "y")].join("\n\n"));
    expect(titled.problems).toEqual([':1: the fence has both `tab="TypeScript"` and `title=`; a fence in a code group is named by its tab alone']);
  });

  it("reads a tab like a title, quoted or not", () => {
    expect(fenceInfo('python tab="Python"')).toEqual({ lang: "python", tab: "Python" });
    expect(fenceInfo("bash tab=curl")).toEqual({ lang: "bash", tab: "curl" });
    expect(fenceInfo('ts tab=""')).toEqual({ lang: "ts" });
  });
});

describe("link cards", () => {
  const targets: Record<string, Target> = {
    "calling-models.md": { kind: "page", href: "/get-started/calling-models" },
    "../console/api-keys.md#create-a-key": { kind: "page", href: "/console/api-keys#create-a-key" },
  };

  it("turns a list whose every item starts with a page link into cards, without the separator", () => {
    const out = md(
      "- [Calling models](calling-models.md): Call models with the **OpenAI** SDK.\n- [API keys](../console/api-keys.md#create-a-key)：限制密钥。\n- [Calling models](calling-models.md)",
      targets,
    );
    expect(out.problems).toEqual([]);
    expect(out.blocks).toEqual([
      {
        t: "links",
        items: [
          {
            href: "/get-started/calling-models",
            kind: "page",
            title: [{ t: "text", v: "Calling models" }],
            description: [
              { t: "text", v: "Call models with the " },
              { t: "strong", c: [{ t: "text", v: "OpenAI" }] },
              { t: "text", v: " SDK." },
            ],
          },
          { href: "/console/api-keys#create-a-key", kind: "page", title: [{ t: "text", v: "API keys" }], description: [{ t: "text", v: "限制密钥。" }] },
          { href: "/get-started/calling-models", kind: "page", title: [{ t: "text", v: "Calling models" }], description: [] },
        ],
      },
    ]);
    expect(out.words).toEqual(expect.arrayContaining(["calling", "openai", "限制密钥"]));
  });

  it("keeps lists that mix in other items, number their items, link elsewhere or check tasks", () => {
    for (const body of [
      "- [Calling models](calling-models.md): Call models.\n- Ask the admin.",
      "1. [Calling models](calling-models.md): Call models.",
      "- [Kaeso](https://kaeso.app): The site.",
      "- [ ] [Calling models](calling-models.md)",
      "- Read [calling models](calling-models.md).",
    ])
      expect(md(body, targets).blocks[0]?.t).toBe("list");
  });
});

describe("headings and inline text", () => {
  it("names a heading by its plain text, through escapes, code, images and emphasis", () => {
    const { headings } = lex(
      "## Keys \\*and\\* `limits`\n\n## The ![console](images/console.png) view\n\n## **Strong** and ~~old~~ text\n\n> ## Quoted\n\n- ## Listed\n",
    );
    expect(headings.map((h) => [h.text, h.id, h.nested ?? false])).toEqual([
      ["Keys *and* limits", "keys-and-limits", false],
      ["The console view", "the-console-view", false],
      ["Strong and old text", "strong-and-old-text", false],
      ["Quoted", "quoted", true],
      ["Listed", "listed", true],
    ]);
  });

  it("keeps escapes as text and draws strikethrough, line breaks and a link's title", () => {
    const out = md('A \\*star\\*, ~~gone~~ and a break  \nthen [the logs](https://example.com/logs "Request logs").');
    expect(out.blocks).toEqual([
      {
        t: "paragraph",
        c: [
          { t: "text", v: "A *star*, " },
          { t: "del", c: [{ t: "text", v: "gone" }] },
          { t: "text", v: " and a break" },
          { t: "br" },
          { t: "text", v: "then " },
          { t: "link", kind: "external", href: "https://example.com/logs", title: "Request logs", c: [{ t: "text", v: "the logs" }] },
          { t: "text", v: "." },
        ],
      },
    ]);
  });
});
