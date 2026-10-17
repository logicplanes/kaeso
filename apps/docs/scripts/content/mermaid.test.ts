import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { Browser } from "@playwright/test";
import { neutral } from "@logicplanes/kaeso-tokens";
import { svgLabels, svgSize } from "./diagram.ts";
import { interFonts, launchRenderer, renderMermaid } from "./mermaid.ts";

let browser: Browser;
beforeAll(async () => {
  browser = await launchRenderer();
}, 60_000);
afterAll(() => browser?.close());

const rgb = (hex: string) => `rgb(${[1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).join(", ")})`;
const render = (code: string) => renderMermaid([{ key: "k1", code }], { fonts: interFonts(process.cwd()), browser });

describe("renderMermaid", () => {
  it("draws a light and a dark SVG that need nothing from the page: no stylesheet, classes, styles, scripts or links", async () => {
    const [result] = await render(
      'flowchart LR\n  accTitle: Route of a request\n  accDescr: An app calls Kaeso, which calls the provider.\n  app[App] -- /v1 --> kaeso[Kaeso]\n  kaeso --> provider[(Provider)]\n  click app "https://example.com"',
    );
    if (!result?.ok) throw new Error(JSON.stringify(result));
    for (const theme of ["light", "dark"] as const) {
      const svg = result.svg[theme];
      expect(svg).toMatch(/^<svg [^>]*role="img"/);
      expect(svg).not.toMatch(/<style|<script|<foreignObject|\sclass=|\sstyle=|\son\w+=|href="http|<!--|\sfilter=/);
      expect(svg).toContain(">Route of a request</title>");
      expect(svg).toContain(">An app calls Kaeso, which calls the provider.</desc>");
      expect(svgLabels(svg)).toEqual(expect.arrayContaining(["App", "Kaeso", "Provider", "/v1"]));
      expect(svgSize(svg)?.width).toBeGreaterThan(200);
      expect(svg).toContain(`fill="${rgb(neutral["bg-raised"][theme])}"`);
      expect(svg).toContain(`fill="${rgb(neutral.text[theme])}"`);
      expect(svg).toMatch(/<rect [^>]*rx="10"/);
      expect(svg).toContain("Inter Variable");
    }
    expect(svgSize(result.svg.light)).toEqual(svgSize(result.svg.dark));
  }, 60_000);

  it("draws sequence and state diagrams with every label as text", async () => {
    const [sequence] = await render(
      "sequenceDiagram\n  accTitle: Reply\n  participant App\n  participant Gateway\n  App->>Gateway: Request\n  Gateway-->>App: Reply",
    );
    const [state] = await render(
      "stateDiagram-v2\n  accTitle: Stream\n  [*] --> ResponseStart\n  ResponseStart --> ResponseStop: no blocks\n  ResponseStop --> [*]",
    );
    if (!sequence?.ok || !state?.ok) throw new Error("not drawn");
    expect(svgLabels(sequence.svg.light)).toEqual(expect.arrayContaining(["App", "Gateway", "Request", "Reply"]));
    expect(svgLabels(state.svg.dark)).toEqual(expect.arrayContaining(["ResponseStart", "ResponseStop", "no blocks"]));
  }, 60_000);

  it("says on which line of the diagram it stopped parsing", async () => {
    const [result] = await render("flowchart LR\n  accTitle: Broken\n  a --> b\n  b -->> \n  c");
    expect(result).toMatchObject({ key: "k1", ok: false, line: 4 });
    expect(result?.ok === false && result.message).toMatch(/^Parse error on line 4/);
    const [unknown] = await render("flowchat LR\n  a --> b");
    expect(unknown).toMatchObject({ ok: false });
    expect(unknown?.ok === false && unknown.message).toMatch(/No diagram type detected/);
  }, 60_000);

  it("starts and closes a browser of its own when it isn't given one, and asks for none without diagrams", async () => {
    expect(await renderMermaid([], { fonts: [] })).toEqual([]);
    const [result] = await renderMermaid([{ key: "own", code: "flowchart LR\n  accTitle: Own\n  a --> b" }], { fonts: interFonts(process.cwd()) });
    expect(result).toMatchObject({ key: "own", ok: true });
  }, 60_000);
});
