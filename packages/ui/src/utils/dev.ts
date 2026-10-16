const isDev = process.env.NODE_ENV !== "production";
const warned = new Set<string>();

export function devWarn(message: string): void {
  if (!isDev || warned.has(message)) return;
  warned.add(message);
  console.warn(`[@logicplanes/kaeso-ui] ${message}`);
}

const FORBIDDEN = [
  /(?:^|:)(?:bg|text|ring|border|shadow|outline|fill|stroke|decoration|accent|caret|placeholder)-(?!left|right|center|justify|start|end|wrap|nowrap|balance|pretty|clip|ellipsis)/,
  /(?:^|:)rounded(?:-|$)/,
  /(?:^|:)p[xytrblse]?-\S+/,
  /(?:^|:)(?:h|min-h|max-h|size)-(?!full|auto|screen|dvh|svh|fit|min|max)/,
  /(?:^|:)font-/,
  /(?:^|:)(?:leading|tracking)-/,
  /(?:^|:)material-/,
  /(?:^|:)opacity-/,
];

export function warnStylingClassName(component: string, className: string | undefined): void {
  if (!isDev || !className) return;
  const bad = className.split(/\s+/).filter((c) => FORBIDDEN.some((re) => re.test(c)));
  if (bad.length > 0) {
    devWarn(
      `${component}: className is for layout only (margin, width, grid placement). Remove ${bad.join(", ")}; change the component or add a variant instead.`,
    );
  }
}

export function warnMissingName(component: string, hasName: boolean): void {
  if (!hasName) devWarn(`${component}: an icon-only control needs an aria-label or a visible label.`);
}

export function warnControlled(component: string, prop: string, value: unknown, handler: unknown): void {
  if (value !== undefined && handler === undefined) {
    devWarn(
      `${component}: \`${prop}\` is controlled but no change handler was passed; the control will not respond. Use \`default${prop[0]?.toUpperCase()}${prop.slice(1)}\` for an uncontrolled value.`,
    );
  }
}
