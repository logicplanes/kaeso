import type { DiagramTheme } from "./diagram.ts";

interface PageJob {
  key: string;
  code: string;
}

interface PageInput {
  jobs: PageJob[];
  configs: Record<DiagramTheme, Record<string, unknown>>;
  radius: number;
}

type PageResult = { key: string; ok: true; svg: Record<string, string> } | { key: string; ok: false; line?: number; message: string };

interface MermaidApi {
  initialize(config: Record<string, unknown>): void;
  parse(code: string): Promise<unknown>;
  render(id: string, code: string, container?: Element): Promise<{ svg: string }>;
}

export async function loadFonts(): Promise<void> {
  await Promise.all(["400", "500", "600"].map((w) => document.fonts.load(`${w} 14px "Inter Variable"`)));
}

export async function renderInPage({ jobs, configs, radius }: PageInput): Promise<PageResult[]> {
  const mermaid = (window as unknown as { mermaid: MermaidApi }).mermaid;
  const SHAPES = new Set(["rect", "circle", "ellipse", "polygon", "polyline", "path", "line"]);
  const TEXT = new Set(["text", "tspan", "textPath"]);
  const PAINT = ["fill", "fill-opacity", "stroke", "stroke-width", "stroke-dasharray", "stroke-opacity", "stroke-linecap", "stroke-linejoin"];
  const TYPE = ["font-family", "font-size", "font-weight", "font-style", "text-anchor", "dominant-baseline"];
  const DEFAULTS: Record<string, string> = { opacity: "1", "fill-opacity": "1", "stroke-opacity": "1" };
  const GEOMETRY = new Set([
    "d",
    "points",
    "transform",
    "viewBox",
    "x",
    "y",
    "x1",
    "x2",
    "y1",
    "y2",
    "width",
    "height",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "dx",
    "dy",
    "refX",
    "refY",
  ]);
  const ROUNDED = /\b(label-container|actor|note|cluster|basic|stateGroup|composit)/;
  const lineOf = (error: unknown): number | undefined => {
    const hash = (error as { hash?: { loc?: { first_line?: number }; line?: number } }).hash;
    const fromHash = hash?.loc?.first_line ?? (typeof hash?.line === "number" ? hash.line + 1 : undefined);
    if (fromHash) return fromHash;
    const match = /line[:\s]+(\d+)/i.exec(String((error as { message?: string }).message ?? error));
    return match ? Number(match[1]) : undefined;
  };
  const messageOf = (error: unknown) =>
    String((error as { message?: string }).message ?? error)
      .split("\n")
      .filter((l) => l.trim() && !/^-*\^$/.test(l.trim()))
      .slice(0, 3)
      .join(" ")
      .replace(/\s+/g, " ")
      .trim();
  const centre = (svg: SVGSVGElement) => {
    for (const node of svg.querySelectorAll("g.node")) {
      const label = [...node.children].find((c) => c.tagName === "g" && c.classList.contains("label")) as SVGGElement | undefined;
      const shape = [...node.children].find((c) => c !== label && c.tagName !== "g") as SVGGraphicsElement | undefined;
      const text = label?.querySelector("text");
      const move = /translate\(\s*(-?[\d.]+)[,\s]+(-?[\d.]+)\s*\)/.exec(label?.getAttribute("transform") ?? "");
      if (!label || !shape || !text || !move || getComputedStyle(text).textAnchor === "middle") continue;
      const box = shape.getBBox();
      const inner = text.getBBox();
      const x = box.x + box.width / 2 - (inner.x + inner.width / 2);
      if (Math.abs(x - Number(move[1])) > 0.5) label.setAttribute("transform", `translate(${x}, ${move[2]})`);
    }
  };
  const flatten = (svg: SVGSVGElement) => {
    centre(svg);
    const plan: [Element, Record<string, string> | null][] = [];
    for (const el of [svg, ...svg.querySelectorAll("*")]) {
      const tag = el.tagName;
      if (tag === "style" || tag === "title" || tag === "desc") continue;
      const cs = getComputedStyle(el);
      if (cs.display === "none" || el.tagName === "foreignObject") {
        plan.push([el, null]);
        continue;
      }
      const parent = el === svg ? null : el.parentElement && getComputedStyle(el.parentElement);
      const names = [
        ...(el === svg || tag === "g" || TEXT.has(tag) ? TYPE : []),
        ...(el === svg || tag === "g" || SHAPES.has(tag) || TEXT.has(tag) || tag === "marker" ? PAINT : []),
      ];
      const attrs: Record<string, string> = {};
      for (const name of names) {
        const value = cs.getPropertyValue(name);
        if (name === "stroke-dasharray" && value.split(",").filter((v) => Number.parseFloat(v) > 0).length < 2) continue;
        if (!value || (parent ? parent.getPropertyValue(name) === value : DEFAULTS[name] === value)) continue;
        attrs[name] = value;
      }
      const opacity = cs.getPropertyValue("opacity");
      if (opacity && opacity !== "1") attrs.opacity = opacity;
      if (tag === "rect" && el.closest("g") && ROUNDED.test(`${el.getAttribute("class") ?? ""} ${el.parentElement?.getAttribute("class") ?? ""}`)) {
        const width = Number(el.getAttribute("width") ?? 0);
        const height = Number(el.getAttribute("height") ?? 0);
        const r = Math.min(radius, width / 2, height / 2);
        if (r > 0) {
          attrs.rx = String(r);
          attrs.ry = String(r);
        }
      }
      plan.push([el, attrs]);
    }
    for (const [el, attrs] of plan) {
      if (!attrs) {
        el.remove();
        continue;
      }
      for (const [name, value] of Object.entries(attrs)) el.setAttribute(name, value);
      el.removeAttribute("style");
      el.removeAttribute("class");
    }
    for (const el of svg.querySelectorAll("style, filter, script, foreignObject, iframe, image, use[href^='http']")) el.remove();
    for (const el of [svg, ...svg.querySelectorAll("*")]) {
      for (const attr of [...el.attributes]) {
        const external = (attr.name === "href" || attr.name === "xlink:href") && !attr.value.startsWith("#");
        if (attr.name.startsWith("data-") || attr.name.startsWith("on") || attr.name === "filter" || external) el.removeAttribute(attr.name);
        else if (GEOMETRY.has(attr.name))
          el.setAttribute(
            attr.name,
            attr.value.replace(/-?\d*\.\d{3,}/g, (n) => String(Math.round(Number(n) * 100) / 100)),
          );
      }
    }
    const box = svg.viewBox.baseVal;
    svg.setAttribute("width", String(Math.ceil(box.width)));
    svg.setAttribute("height", String(Math.ceil(box.height)));
    svg.setAttribute("role", "img");
    svg.removeAttribute("aria-roledescription");
    svg.removeAttribute("class");
    svg.removeAttribute("style");
  };
  const results: PageResult[] = [];
  for (const job of jobs) {
    const svg: Record<string, string> = {};
    let failure: PageResult | null = null;
    for (const theme of Object.keys(configs) as DiagramTheme[]) {
      mermaid.initialize(configs[theme]);
      try {
        await mermaid.parse(job.code);
        const host = document.createElement("div");
        document.body.append(host);
        const { svg: markup } = await mermaid.render(`d${job.key}`, job.code, host);
        host.innerHTML = markup;
        const root = host.querySelector("svg");
        if (!root) throw new Error("mermaid drew nothing");
        flatten(root);
        svg[theme] = new XMLSerializer().serializeToString(root).replace(/<!--[\s\S]*?-->/g, "");
        host.remove();
      } catch (error) {
        const line = lineOf(error);
        failure = { key: job.key, ok: false, ...(line ? { line } : {}), message: messageOf(error) };
        break;
      }
    }
    results.push(failure ?? { key: job.key, ok: true, svg });
  }
  return results;
}
