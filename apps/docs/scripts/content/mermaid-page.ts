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
  const corner = (svg: SVGSVGElement, ground: string) => {
    type Box = { x: number; y: number; width: number; height: number };
    const inRoot = (el: SVGGraphicsElement): Box => {
      const b = el.getBBox();
      const at = new DOMPoint(b.x, b.y).matrixTransform(el.getCTM() ?? new DOMMatrix());
      return { x: at.x, y: at.y, width: b.width, height: b.height };
    };
    const meets = (a: Box, b: Box, gap: number) =>
      a.x - gap < b.x + b.width && b.x - gap < a.x + a.width && a.y - gap < b.y + b.height && b.y - gap < a.y + a.height;
    const along = (p: SVGGeometryElement) => {
      const length = p.getTotalLength();
      const m = p.getCTM() ?? new DOMMatrix();
      return Array.from({ length: Math.ceil(length / 4) + 1 }, (_, i) => p.getPointAtLength(Math.min(length, i * 4)).matrixTransform(m));
    };
    const edges = [...svg.querySelectorAll("g.edgePaths path, path.flowchart-link")].flatMap((p) => along(p as SVGGeometryElement));
    const blocks = [...svg.querySelectorAll("g.node, g.edgeLabel")].map((el) => inRoot(el as SVGGraphicsElement)).filter((b) => b.width > 0);
    const clusters = [...svg.querySelectorAll("g.cluster")].flatMap((cluster) => {
      const frame = cluster.querySelector(":scope > rect") as SVGRectElement | null;
      const label = cluster.querySelector(":scope > g.cluster-label") as SVGGElement | null;
      return frame && label ? [{ frame, label, border: along(frame) }] : [];
    });
    const placed: Box[] = [];
    for (const { frame, label } of clusters) {
      const box = frame.getBBox();
      const inner = label.getBBox();
      const outer = inRoot(frame);
      const width = inner.width + 8;
      const height = inner.height + 2;
      const left = outer.x + 8;
      const right = outer.x + outer.width - 8 - width;
      const top = outer.y + 7;
      const bottom = outer.y + outer.height - 7 - height;
      const others = clusters.filter((c) => c.frame !== frame).flatMap((c) => c.border);
      const free = (b: Box) =>
        ![...edges, ...others].some((p) => meets(b, { x: p.x, y: p.y, width: 0, height: 0 }, 6)) &&
        !blocks.some((n) => meets(b, n, 4)) &&
        !placed.some((n) => meets(b, n, 4));
      const steps = Math.max(0, Math.floor((right - left) / 4));
      const spots = [top, bottom].flatMap((y) => Array.from({ length: steps + 1 }, (_, i) => ({ x: left + i * 4, y, width, height })));
      const spot = spots.find(free) ?? { x: left, y: top, width, height };
      placed.push(spot);
      const mask = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      for (const [name, value] of Object.entries({ x: inner.x - 4, y: inner.y - 1, width, height, rx: 4 })) mask.setAttribute(name, String(value));
      mask.setAttribute("style", `fill:${ground};stroke:none`);
      label.prepend(mask);
      label.setAttribute("transform", `translate(${box.x + 12 - inner.x + spot.x - left}, ${box.y + 8 - inner.y + spot.y - top})`);
      const topGroup = svg.querySelector(":scope > g") as SVGGElement | null;
      const from = topGroup?.getCTM();
      const to = label.getCTM();
      if (!topGroup || !from || !to) continue;
      const m = from.inverse().multiply(to);
      label.setAttribute("transform", `matrix(${m.a} ${m.b} ${m.c} ${m.d} ${m.e} ${m.f})`);
      topGroup.append(label);
    }
  };
  const flatten = (svg: SVGSVGElement, ground: string) => {
    centre(svg);
    corner(svg, ground);
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
        if (name === "stroke-dasharray") {
          const lengths = value.split(/[\s,]+/).map(Number.parseFloat);
          const gaps = lengths.length === 1 ? lengths : lengths.filter((_, i) => i % 2 === 1);
          if (!gaps.some((gap) => gap > 0)) continue;
        }
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
    const comments = document.createTreeWalker(svg, NodeFilter.SHOW_COMMENT);
    const found: Comment[] = [];
    while (comments.nextNode()) found.push(comments.currentNode as Comment);
    for (const comment of found) comment.remove();
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
        flatten(root, String((configs[theme].themeVariables as Record<string, unknown> | undefined)?.background ?? "none"));
        svg[theme] = new XMLSerializer().serializeToString(root);
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
