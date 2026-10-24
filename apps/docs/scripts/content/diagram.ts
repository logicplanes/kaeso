import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { font, neutral, radius } from "@logicplanes/kaeso-tokens";

export const DIAGRAM_THEMES = ["light", "dark"] as const;
export type DiagramTheme = (typeof DIAGRAM_THEMES)[number];

export const DIAGRAM_LANG = "mermaid";
export const DIAGRAM_ROLES: readonly string[] = ["accent", "outside"];
export const DIAGRAM_COMMAND = "pnpm --filter kaeso-docs diagrams";
const REVISION = 6;

export interface DiagramText {
  title?: string;
  description?: string;
}

export interface DiagramJob {
  key: string;
  code: string;
  file: string;
  line: number;
}

const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };

const decode = (s: string) =>
  s.replace(/&(#\d+|#x[\da-f]+|\w+);/gi, (whole, name: string) => {
    if (name[0] !== "#") return ENTITIES[name.toLowerCase()] ?? whole;
    return String.fromCodePoint(name[1] === "x" || name[1] === "X" ? parseInt(name.slice(2), 16) : Number(name.slice(1)));
  });

export function diagramText(code: string): DiagramText {
  const title = /^[ \t]*accTitle[ \t]*:[ \t]*(.+?)[ \t]*$/m.exec(code)?.[1];
  const block = /^[ \t]*accDescr[ \t]*\{([\s\S]*?)\}/m.exec(code)?.[1];
  const line = /^[ \t]*accDescr[ \t]*:[ \t]*(.+?)[ \t]*$/m.exec(code)?.[1];
  const description = (block ?? line ?? "")
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .join(" ");
  return { ...(title ? { title } : {}), ...(description ? { description } : {}) };
}

export function mermaidVersion(docsRoot: string): string {
  const pkg = join(docsRoot, "node_modules", "mermaid", "package.json");
  return existsSync(pkg) ? (JSON.parse(readFileSync(pkg, "utf8")) as { version: string }).version : "unknown";
}

export function mermaidConfig(theme: DiagramTheme): Record<string, unknown> {
  const c = (name: keyof typeof neutral) => neutral[name][theme];
  const text = c("text");
  const secondary = c("text-secondary");
  const ground = c("fill-1");
  const raised = c("bg-raised");
  const ring = c("line-strong");
  const edge = c("line-field");
  const ink = c("ink");
  const onInk = c("on-ink");
  const size = 14;
  const small = 12;
  const shapes = (selector: string) => ["rect", "path", "polygon", "circle", "ellipse"].map((s) => `${selector} ${s}`).join(",");
  const css = [
    `${shapes(".node.accent")}{fill:${ink} !important;stroke:${ink} !important;}`,
    `.node.accent text,.node.accent tspan{fill:${onInk} !important;font-weight:600;}`,
    `${shapes(".node.outside")}{fill:${ground} !important;stroke:${ring} !important;stroke-dasharray:4 3;}`,
    `.node.outside text,.node.outside tspan{fill:${secondary} !important;}`,
    `.edgeLabel text,.edgeLabel tspan,.edgeLabels text,.edgeLabels tspan{fill:${secondary};font-size:${small}px;font-weight:500 !important;}`,
    `.edgeLabel rect,.edgeLabel .label rect,.edgeLabels rect,.labelBkg{fill:${ground} !important;opacity:1 !important;}`,
    `.cluster>rect{fill:none;stroke:${ring};stroke-dasharray:4 4;}`,
    `.cluster-label text,.cluster-label tspan,.cluster text{fill:${secondary};font-size:${small}px;font-weight:500;}`,
    `.messageText{fill:${text};}`,
    `.sequenceNumber{fill:${onInk};font-size:11px;font-weight:600;}`,
    `marker[id$="sequencenumber"] circle{fill:${ink};stroke:none;}`,
    `.noteText,.noteText tspan{fill:${text};font-size:13px;}`,
    `.actor{stroke:${ring};}`,
  ].join("");
  return {
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    layout: "dagre",
    look: "classic",
    darkMode: theme === "dark",
    fontFamily: font.sans,
    fontSize: size,
    htmlLabels: false,
    markdownAutoWrap: false,
    themeCSS: css,
    flowchart: {
      htmlLabels: false,
      useMaxWidth: false,
      wrappingWidth: 240,
      minNodeWidth: 0,
      curve: "basis",
      padding: 16,
      nodeSpacing: 40,
      rankSpacing: 48,
      diagramPadding: 4,
      subGraphTitleMargin: { top: 4, bottom: 12 },
    },
    sequence: {
      useMaxWidth: false,
      mirrorActors: false,
      diagramMarginX: 4,
      diagramMarginY: 12,
      actorMargin: 40,
      width: 120,
      height: 44,
      boxMargin: 8,
      boxTextMargin: 6,
      noteMargin: 10,
      messageMargin: 36,
      actorFontFamily: font.sans,
      noteFontFamily: font.sans,
      messageFontFamily: font.sans,
      actorFontSize: size,
      actorFontWeight: 600,
      noteFontSize: 13,
      messageFontSize: size,
      wrap: false,
    },
    state: { useMaxWidth: false, minNodeWidth: 0, padding: 12 },
    themeVariables: {
      darkMode: theme === "dark",
      background: ground,
      fontFamily: font.sans,
      fontSize: `${size}px`,
      textColor: text,
      primaryColor: raised,
      primaryBorderColor: ring,
      primaryTextColor: text,
      secondaryColor: ground,
      secondaryBorderColor: ring,
      secondaryTextColor: text,
      tertiaryColor: ground,
      tertiaryBorderColor: ring,
      tertiaryTextColor: text,
      mainBkg: raised,
      nodeBorder: ring,
      nodeTextColor: text,
      lineColor: edge,
      clusterBkg: "none",
      clusterBorder: ring,
      titleColor: secondary,
      edgeLabelBackground: ground,
      actorBkg: raised,
      actorBorder: ring,
      actorTextColor: text,
      actorLineColor: ring,
      signalColor: edge,
      signalTextColor: text,
      labelBoxBkgColor: raised,
      labelBoxBorderColor: ring,
      labelTextColor: secondary,
      loopTextColor: secondary,
      noteBkgColor: raised,
      noteBorderColor: ring,
      noteTextColor: secondary,
      activationBkgColor: raised,
      activationBorderColor: ring,
      sequenceNumberColor: onInk,
      stateBkg: raised,
      stateBorder: ring,
      stateLabelColor: text,
      compositeBackground: ground,
      compositeTitleBackground: ground,
      altBackground: ground,
      transitionColor: edge,
      transitionLabelColor: secondary,
      specialStateColor: ink,
      innerEndBackground: ink,
      labelColor: text,
      errorBkgColor: ground,
      errorTextColor: text,
    },
  };
}

export const DIAGRAM_RADIUS = Number.parseInt(radius.item, 10);

export function diagramKey(code: string, version: string): string {
  const configs = DIAGRAM_THEMES.map(mermaidConfig);
  return createHash("sha256")
    .update(JSON.stringify([REVISION, version, DIAGRAM_RADIUS, configs, code.replace(/\r\n?/g, "\n").trim()]))
    .digest("hex")
    .slice(0, 16);
}

export const diagramFile = (key: string, theme: DiagramTheme) => `${key}.${theme}.svg`;

export function diagramFiles(dir: string): string[] {
  return existsSync(dir)
    ? readdirSync(dir)
        .filter((f) => f.endsWith(".svg"))
        .sort()
    : [];
}

export function svgLabels(svg: string): string[] {
  return [...svg.matchAll(/<text\b[^>]*>([\s\S]*?)<\/text>/g)]
    .map((m) =>
      decode((m[1] ?? "").replace(/<[^>]*>/g, " "))
        .replace(/\s+/g, " ")
        .trim(),
    )
    .filter(Boolean);
}

export function svgSize(svg: string): { width: number; height: number } | null {
  const root = /<svg\b[^>]*>/.exec(svg)?.[0] ?? "";
  const width = Number(/\swidth="([\d.]+)"/.exec(root)?.[1]);
  const height = Number(/\sheight="([\d.]+)"/.exec(root)?.[1]);
  return width > 0 && height > 0 ? { width, height } : null;
}
