import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { font, neutral, radius } from "@logicplanes/kaeso-tokens";

export const DIAGRAM_THEMES = ["light", "dark"] as const;
export type DiagramTheme = (typeof DIAGRAM_THEMES)[number];

export const DIAGRAM_LANG = "mermaid";
export const DIAGRAM_COMMAND = "pnpm --filter kaeso-docs diagrams";
const REVISION = 2;

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
  const raised = c("bg-raised");
  const well = c("fill-1");
  const ring = c("line-strong");
  const line = c("line");
  const size = 14;
  return {
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    darkMode: theme === "dark",
    fontFamily: font.sans,
    fontSize: size,
    htmlLabels: false,
    markdownAutoWrap: false,
    flowchart: { htmlLabels: false, useMaxWidth: false, wrappingWidth: 360, curve: "basis", padding: 12, nodeSpacing: 36, rankSpacing: 44, diagramPadding: 4 },
    sequence: {
      useMaxWidth: false,
      mirrorActors: false,
      diagramMarginX: 4,
      diagramMarginY: 4,
      actorMargin: 32,
      width: 120,
      boxMargin: 8,
      noteMargin: 10,
      messageMargin: 32,
      actorFontFamily: font.sans,
      noteFontFamily: font.sans,
      messageFontFamily: font.sans,
      actorFontSize: size,
      noteFontSize: size,
      messageFontSize: size,
      wrap: false,
    },
    state: { useMaxWidth: false, padding: 8 },
    themeVariables: {
      darkMode: theme === "dark",
      background: c("bg-panel"),
      fontFamily: font.sans,
      fontSize: `${size}px`,
      textColor: text,
      primaryColor: raised,
      primaryBorderColor: ring,
      primaryTextColor: text,
      secondaryColor: well,
      secondaryBorderColor: line,
      secondaryTextColor: text,
      tertiaryColor: well,
      tertiaryBorderColor: line,
      tertiaryTextColor: text,
      mainBkg: raised,
      nodeBorder: ring,
      nodeTextColor: text,
      lineColor: secondary,
      clusterBkg: well,
      clusterBorder: well,
      titleColor: secondary,
      edgeLabelBackground: c("bg-panel"),
      actorBkg: raised,
      actorBorder: ring,
      actorTextColor: text,
      actorLineColor: ring,
      signalColor: secondary,
      signalTextColor: text,
      labelBoxBkgColor: well,
      labelBoxBorderColor: line,
      labelTextColor: text,
      loopTextColor: secondary,
      noteBkgColor: well,
      noteBorderColor: well,
      noteTextColor: text,
      activationBkgColor: well,
      activationBorderColor: ring,
      sequenceNumberColor: c("on-ink"),
      stateBkg: raised,
      stateBorder: ring,
      stateLabelColor: text,
      compositeBackground: well,
      compositeTitleBackground: well,
      altBackground: well,
      transitionColor: secondary,
      transitionLabelColor: text,
      specialStateColor: c("ink"),
      innerEndBackground: c("ink"),
      labelColor: text,
      errorBkgColor: well,
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
