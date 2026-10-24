import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Block, Inline } from "./ast";
import { TextProvider } from "~/i18n/text";
import { Markdown, diagramMinWidth } from "./render";
import { CODE_TAB_STORAGE_KEY, forgetCodeTab } from "./code-tab";

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

  it("lets long identifiers wrap only after their separators and between the words of a type name, and keeps their text", () => {
    const c = draw([
      {
        t: "paragraph",
        c: [
          { t: "code", v: "sampling.max_output_tokens" },
          { t: "code", v: "Role::SystemMessage" },
          { t: "code", v: "/v1/chat/completions" },
        ],
      },
    ]);
    const [field, role, path] = [...c.querySelectorAll("code")];
    expect(field).toHaveTextContent("sampling.max_output_tokens");
    expect([...(field?.childNodes ?? [])].map((n) => (n.nodeName === "WBR" ? "|" : n.textContent)).join("")).toBe("sampling.|max_|output_|tokens");
    expect([...(role?.childNodes ?? [])].map((n) => (n.nodeName === "WBR" ? "|" : n.textContent)).join("")).toBe("Role::|System|Message");
    expect([...(path?.childNodes ?? [])].map((n) => (n.nodeName === "WBR" ? "|" : n.textContent)).join("")).toBe("/v1/|chat/|completions");
  });

  it("wraps a prose block at every width, and code only on phones", () => {
    const c = draw([
      { t: "code", lang: "text", code: "Kaeso is an open-source AI gateway you run on your own servers.", wrap: true },
      { t: "code", lang: "bash", code: "curl https://kaeso.example.com/v1/models" },
    ]);
    const [prose, code] = [...c.querySelectorAll("code > *")];
    expect(prose).toHaveClass("whitespace-pre-wrap");
    expect(prose).not.toHaveClass("max-sm:whitespace-pre-wrap");
    expect(code).toHaveClass("max-sm:whitespace-pre-wrap");
    expect(code).not.toHaveClass("whitespace-pre-wrap");
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
    expect(screen.getByRole("region", { name: "Table: Level, Requests" })).toHaveAttribute("tabindex");
    expect(screen.getByRole("columnheader", { name: "Requests" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "60" })).toBeInTheDocument();
  });

  it("draws a table as a named region with its headers and cells, and lets only long identifiers break at their underscores", () => {
    const summary = "Start the gateway, the console and the databases with Docker Compose, then send the first request";
    draw([
      {
        t: "table",
        align: ["start", "start", "start"],
        head: [[{ t: "text", v: "Page" }], [{ t: "text", v: "Setting" }], [{ t: "text", v: "What to do" }]],
        rows: [
          [
            [{ t: "link", kind: "page", href: "/get-started/quick-start", c: [{ t: "text", v: "Quick start" }] }],
            [{ t: "code", v: "metrics_listen" }],
            [{ t: "text", v: summary }],
          ],
          [
            [{ t: "link", kind: "page", href: "/get-started/coding-tools", c: [{ t: "text", v: "Coding tools" }] }],
            [{ t: "code", v: "KAESO_METRICS_LISTEN" }],
            [{ t: "link", kind: "external", href: "https://kaeso.app/", c: [{ t: "text", v: "Point Claude Code, Codex and other tools at the install" }] }],
          ],
        ],
      },
    ]);
    const table = within(screen.getByRole("region", { name: "Table: Page, Setting, What to do" }));
    expect(table.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["Page", "Setting", "What to do"]);
    expect(table.getByRole("link", { name: "Quick start" })).toHaveAttribute("href", "/get-started/quick-start");
    expect(table.getByRole("cell", { name: summary })).toBeInTheDocument();
    expect(table.getByRole("link", { name: /Point Claude Code/ })).toHaveAttribute("href", "https://kaeso.app/");
    expect(table.getByText("metrics_listen").querySelector("wbr")).toBeNull();
    const long = table.getByText((_, el) => el?.tagName === "CODE" && el.textContent === "KAESO_METRICS_LISTEN");
    expect(long.querySelectorAll("wbr")).toHaveLength(2);
    expect(long).toHaveClass("inline");
    expect(long).not.toHaveClass("inline-block", "box-decoration-clone");
    expect(table.getByText("metrics_listen")).toHaveClass("inline", "whitespace-nowrap");
  });

  it("draws a table of three or more columns again for phones, each row its first cell over labelled values, a mostly short column's values beside their label", () => {
    const summary = "Start the gateway, the console and the databases with Docker Compose, then send the first request";
    const text = (v: string): Inline[] => [{ t: "text", v }];
    draw([
      {
        t: "table",
        align: ["start", "start", "start"],
        head: [text("Page"), text("Setting"), text("What to do")],
        rows: [
          [text("Quick start"), [{ t: "code", v: "metrics_listen" }], text(summary)],
          [text("Coding tools"), [], text(summary)],
          [text("Upgrades"), text("listen"), text(summary)],
          [text("Backups"), text(summary), text(summary)],
          [text("Email"), text("smtp_host"), text("Run the script")],
        ],
      },
      {
        t: "table",
        align: ["start", "start"],
        head: [text("Level"), text("Requests")],
        rows: [[text("Key"), text("60")]],
      },
    ]);
    const list = screen.getByRole("list", { name: "Table: Page, Setting, What to do" });
    expect(list).toHaveClass("sm:hidden");
    expect(screen.getByRole("region", { name: "Table: Page, Setting, What to do" }).closest(".max-sm\\:hidden")).not.toBeNull();
    const rows = within(list).getAllByRole("listitem");
    const pairs = (row: HTMLElement | undefined, which: string) => [...(row?.querySelectorAll(which) ?? [])].map((el) => el.textContent);
    const lists = (row: HTMLElement | undefined) =>
      [...(row?.querySelectorAll("dl") ?? [])].map((dl) => (dl.classList.contains("flex-col") ? "below" : "beside"));
    expect(rows[0]).toHaveTextContent(/^Quick start/);
    expect(pairs(rows[0], "dt, dd")).toEqual(["Setting", "metrics_listen", "What to do", summary]);
    expect(lists(rows[0])).toEqual(["beside", "below"]);
    expect(pairs(rows[1], "dt")).toEqual(["What to do"]);
    expect(lists(rows[1])).toEqual(["below"]);
    expect(pairs(rows[3], "dt")).toEqual(["Setting", "What to do"]);
    expect(lists(rows[3])).toEqual(["below"]);
    expect(pairs(rows[4], "dt")).toEqual(["Setting", "What to do"]);
    expect(lists(rows[4])).toEqual(["beside", "below"]);
    expect(screen.queryByRole("list", { name: "Table: Level, Requests" })).toBeNull();
  });

  it("lets long code in a table break after its separators and brackets, a Chinese list break after each comma, and a wide table's text columns narrow further", () => {
    const code = (v: string): Inline => ({ t: "code", v });
    const text = (v: string): Inline => ({ t: "text", v });
    const container = draw([
      {
        t: "table",
        align: ["start", "start", "start", "start", "start"],
        head: [[text("Route")], [text("Right")], [text("Fields")], [text("Default")], [text("Notes")]],
        rows: [
          [
            [code("GET orgs/{org}/projects/{project}")],
            [code("roles.manage")],
            [code("tools[].name"), text("、"), code(".description"), text("、"), code(".strict")],
            [code("Kaeso <kaeso@localhost>")],
            [text("Answers with the rights a project role holds, one row for each right in the catalogue")],
          ],
        ],
      },
    ]);
    const route = within(screen.getByRole("region", { name: "Table: Route, Right, Fields, Default, Notes" })).getByText(
      (_, el) => el?.tagName === "CODE" && el.textContent === "GET orgs/{org}/projects/{project}",
    );
    expect(route.querySelectorAll("wbr")).toHaveLength(3);
    const sender = within(screen.getByRole("region", { name: /^Table: Route/ })).getByText(
      (_, el) => el?.tagName === "CODE" && el.textContent === "Kaeso <kaeso@localhost>",
    );
    expect(sender.querySelectorAll("wbr")).toHaveLength(1);
    const fields = container.querySelector("table td:nth-child(3)");
    expect(fields?.querySelectorAll(":scope > wbr")).toHaveLength(2);
    expect(container.querySelector("table td:nth-child(5)")).toHaveClass("min-w-120");
  });

  it("breaks a long type name between its words, and narrows a table's gutters at five columns and its text columns from four", () => {
    const code = (v: string): Inline => ({ t: "code", v });
    const text = (v: string): Inline => ({ t: "text", v });
    const note = text("Dropped for other dialects when it only holds edits that every target can apply");
    const table = (columns: number): Block => ({
      t: "table",
      align: Array.from({ length: columns }, () => "start" as const),
      head: Array.from({ length: columns }, (_, i) => [text(`Column ${i + 1}`)]),
      rows: [Array.from({ length: columns }, (_, i) => (i === 0 ? [code("BetaContextManagementConfigParam")] : [note]))],
    });
    const container = draw([table(5), table(4), table(3)]);
    const [five, four, three] = [...container.querySelectorAll("table")];
    expect(five?.querySelector("code")?.querySelectorAll("wbr")).toHaveLength(4);
    expect(five?.querySelector("td:nth-child(2)")).toHaveClass("px-8", "min-w-120");
    expect(five?.querySelector("th")).toHaveClass("px-8");
    expect(four?.querySelector("td:nth-child(2)")).toHaveClass("px-12", "min-w-120");
    expect(three?.querySelector("td:nth-child(2)")).toHaveClass("px-12", "min-w-140");
  });

  it("keeps names whole in a cell broken over lines, unless one is too long to fit beside the rest", () => {
    const code = (v: string): Inline => ({ t: "code", v });
    const text = (v: string): Inline => ({ t: "text", v });
    const container = draw([
      {
        t: "table",
        align: ["start", "start", "start"],
        head: [[text("Name")], [text("Type")], [text("Sets")]],
        rows: [
          [[code("KAESO_SERVER_MAX_CONNECTIONS"), { t: "br" }, code("server.max_connections")], [text("text")], [text("How often")]],
          [
            [code("KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS"), { t: "br" }, code("instance.max_metadata_retention_days")],
            [text("integer")],
            [text("Longest")],
          ],
          [[code("streams.keepalive_interval")], [text("text")], [text("How often")]],
        ],
      },
    ]);
    const cells = [...container.querySelectorAll("table td:first-child")];
    const codes = (i: number) => [...(cells[i]?.querySelectorAll("code") ?? [])];
    expect(cells[0]?.querySelector("br")).not.toBeNull();
    expect(codes(0).every((c) => c.classList.contains("whitespace-nowrap") && !c.querySelector("wbr"))).toBe(true);
    expect(codes(1).every((c) => c.querySelector("wbr"))).toBe(true);
    expect(codes(2)[0]?.querySelector("wbr")).not.toBeNull();
  });

  it("draws a Chinese table in a region named in Chinese, with its words in their cells", () => {
    render(
      <TextProvider locale="zh-CN">
        <Markdown
          blocks={[
            {
              t: "table",
              align: ["start", "start"],
              head: [[{ t: "text", v: "页面" }], [{ t: "text", v: "内容" }]],
              rows: [
                [[{ t: "text", v: "快速开始" }], [{ t: "text", v: "启动网关控制台和数据库" }]],
                [[{ t: "text", v: "调用模型" }], [{ t: "text", v: "在本机调用模型并查看日志" }]],
              ],
            },
          ]}
        />
      </TextProvider>,
    );
    expect(screen.getByRole("region", { name: "表格：页面、内容" })).toBeInTheDocument();
    expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["页面", "内容"]);
    expect(screen.getByRole("cell", { name: "快速开始" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "启动网关控制台和数据库" })).toBeInTheDocument();
  });

  it("lets long code break with its sentence and keeps short code in one piece beside its punctuation", () => {
    const c = draw([
      {
        t: "paragraph",
        c: [
          { t: "code", v: "docker compose exec gateway kaeso openapi" },
          { t: "text", v: ". Then " },
          { t: "code", v: "kaeso serve" },
        ],
      },
    ]);
    const [long, short] = [...c.querySelectorAll("code")];
    expect(long).toHaveClass("inline", "box-decoration-clone");
    expect(long).not.toHaveClass("inline-block");
    expect(short).toHaveClass("inline", "whitespace-nowrap");
    expect(short).not.toHaveClass("inline-block");
    expect(short?.querySelector("wbr")).toBeNull();
  });

  it("draws a diagram as one labelled region with its light and dark image and its description for screen readers", () => {
    draw([
      {
        t: "diagram",
        light: "/assets/route.light.svg",
        dark: "/assets/route.dark.svg",
        width: 640,
        height: 180,
        title: "Route of a request",
        description: "An app sends a request to Kaeso, which forwards it to the provider.",
      },
    ]);
    const region = screen.getByRole("region", { name: "Diagram: Route of a request" });
    const [light, dark] = screen.getAllByRole("img", { name: "Route of a request" });
    expect(region).toContainElement(light ?? null);
    expect(light).toHaveAttribute("src", "/assets/route.light.svg");
    expect(dark).toHaveAttribute("src", "/assets/route.dark.svg");
    expect(light).toHaveAttribute("width", "640");
    expect(light).toHaveAccessibleDescription("An app sends a request to Kaeso, which forwards it to the provider.");
    expect(light?.parentElement).toHaveClass("min-w-480");
  });

  it("names a diagram in the page's language and leaves out a description it doesn't have", () => {
    render(
      <TextProvider locale="de">
        <Markdown blocks={[{ t: "diagram", light: "/l.svg", dark: "/d.svg", width: 300, height: 120, title: "Ablauf" }]} />
      </TextProvider>,
    );
    expect(screen.getByRole("region", { name: "Diagramm: Ablauf" })).toBeInTheDocument();
    expect(screen.getAllByRole("img", { name: "Ablauf" })[0]).not.toHaveAttribute("aria-describedby");
  });

  it("lets a diagram shrink by a quarter at most, never needing more than the framed column, and scroll in its region beyond that", () => {
    expect(diagramMinWidth(300)).toBeUndefined();
    expect(diagramMinWidth(420)).toBeUndefined();
    expect(diagramMinWidth(430)).toBe("min-w-360");
    expect(diagramMinWidth(479)).toBe("min-w-360");
    expect(diagramMinWidth(630)).toBe("min-w-480");
    expect(diagramMinWidth(640)).toBe("min-w-480");
    expect(diagramMinWidth(775)).toBe("min-w-560");
    expect(diagramMinWidth(2000)).toBe("min-w-560");
  });
});

describe("Heading links", () => {
  it("links a heading to itself, and leaves a heading that holds a link as it is", () => {
    draw([
      { t: "heading", level: 2, id: "streaming", c: [{ t: "text", v: "Streaming" }] },
      { t: "heading", level: 3, id: "see-limits", c: [{ t: "link", kind: "page", href: "/console/limits", c: [{ t: "text", v: "See limits" }] }] },
    ]);
    const own = screen.getByRole("link", { name: "Streaming" });
    expect(own).toHaveAttribute("href", "#streaming");
    expect(screen.getByRole("heading", { level: 2, name: "Streaming" })).toContainElement(own);
    expect(screen.getByRole("link", { name: "See limits" })).toHaveAttribute("href", "/console/limits");
    expect(screen.getAllByRole("link")).toHaveLength(2);
  });
});

describe("Code groups", () => {
  const request: Block = {
    t: "codegroup",
    tabs: [
      { label: "curl", lang: "bash", code: "curl https://kaeso.example.com/v1/models" },
      { label: "Python", lang: "python", code: "client.models.list()" },
      { label: "TypeScript", lang: "ts", code: "await client.models.list();" },
    ],
  };
  const reply: Block = {
    t: "codegroup",
    tabs: [
      { label: "Python", lang: "python", code: "print(response.output_text)" },
      { label: "TypeScript", lang: "ts", code: "console.log(response.output_text);" },
    ],
  };

  const tabs = (name: string) => screen.getAllByRole("tab", { name });
  const first = (name: string) => tabs(name)[0] as HTMLElement;

  beforeEach(() => {
    localStorage.clear();
    forgetCodeTab();
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("shows the first tab until the reader chooses, then every group with that tab follows and the choice is kept", async () => {
    draw([request, reply]);
    expect(first("curl")).toHaveAttribute("aria-selected", "true");
    expect(tabs("Python")[1]).toHaveAttribute("aria-selected", "true");
    await userEvent.click(first("TypeScript"));
    for (const tab of tabs("TypeScript")) expect(tab).toHaveAttribute("aria-selected", "true");
    expect(screen.getAllByRole("tabpanel").map((panel) => panel.textContent)).toEqual(["await client.models.list();", "console.log(response.output_text);"]);
    expect(localStorage.getItem(CODE_TAB_STORAGE_KEY)).toBe("TypeScript");
  });

  it("opens on the tab chosen on an earlier page, and keeps its own where it lacks that tab", () => {
    localStorage.setItem(CODE_TAB_STORAGE_KEY, "curl");
    draw([request, reply]);
    expect(first("curl")).toHaveAttribute("aria-selected", "true");
    expect(tabs("Python")[1]).toHaveAttribute("aria-selected", "true");
  });

  it("still opens and switches when the browser keeps nothing", async () => {
    const refuse = () => {
      throw new DOMException("blocked", "SecurityError");
    };
    vi.stubGlobal("localStorage", { getItem: refuse, setItem: refuse });
    draw([request, reply]);
    expect(first("curl")).toHaveAttribute("aria-selected", "true");
    await userEvent.click(first("Python"));
    for (const tab of tabs("Python")) expect(tab).toHaveAttribute("aria-selected", "true");
  });

  it("keeps the group the reader clicked in place when groups above it change height", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() =>
      DOMRect.fromRect({ y: localStorage.getItem(CODE_TAB_STORAGE_KEY) ? 340 : 400, height: 120 }),
    );
    const page = document.scrollingElement ?? document.documentElement;
    const scrollBy = vi.fn();
    page.scrollBy = scrollBy;
    draw([request, reply]);
    await userEvent.click(first("TypeScript"));
    expect(scrollBy).toHaveBeenCalledWith(0, -60);
  });
});

describe("Link cards", () => {
  it("draws each next page as a link with its name and sentence, and keeps links inside the sentence as text", () => {
    const c = draw([
      {
        t: "links",
        items: [
          {
            href: "/get-started/calling-models",
            kind: "page",
            title: [{ t: "text", v: "Calling models" }],
            description: [
              { t: "text", v: "Call models with the " },
              { t: "link", kind: "page", href: "/api/model-api", c: [{ t: "text", v: "official SDKs" }] },
              { t: "text", v: "." },
            ],
          },
          { href: "/console/api-keys#create-a-key", kind: "page", title: [{ t: "text", v: "API keys" }], description: [] },
        ],
      },
    ]);
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("link", { name: "Calling models Call models with the official SDKs." })).toHaveAttribute("href", "/get-started/calling-models");
    expect(screen.getByRole("link", { name: "API keys" })).toHaveAttribute("href", "/console/api-keys#create-a-key");
    expect(c.querySelectorAll("a a")).toHaveLength(0);
    expect(screen.getAllByRole("link")).toHaveLength(2);
  });
});
