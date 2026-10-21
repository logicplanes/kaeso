import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as t from "../src/tokens.ts";
import { tokensCss } from "./tokens-css.ts";

const dist = join(dirname(fileURLToPath(import.meta.url)), "..", "dist");
mkdirSync(dist, { recursive: true });

type Mode = { light: string; dark: string };
const light: string[] = [];
const dark: string[] = [];
const fixed: string[] = [];
const both = (name: string, m: Mode) => {
  light.push(`  --k-${name}: ${m.light};`);
  dark.push(`  --k-${name}: ${m.dark};`);
};

for (const [name, m] of Object.entries(t.neutral)) both(name, m);
for (const [role, s] of Object.entries(t.semantic)) {
  both(role, s.solid);
  both(`${role}-text`, s.text);
  both(`${role}-tint`, s.tint);
}
both("danger-fill", t.dangerFill);
both("on-danger", t.onDanger);
both("viz-1-soft", t.vizSoft);
for (const [id, m] of Object.entries(t.providerMark)) both(`mark-${id}`, m);
t.viz.forEach((v, i) => {
  both(`viz-${i + 1}`, { light: v.light, dark: v.dark });
  both(`viz-thin-${i + 1}`, { light: v.thin, dark: v.dark });
});
for (const [name, m] of Object.entries(t.weight)) both(`weight-${name}`, m);
for (const [name, m] of Object.entries(t.shadow)) both(`shadow-${name}`, m);
for (const [name, m] of Object.entries(t.material)) both(name, m);
for (const [name, m] of Object.entries(t.themeDisplay)) both(name, m);
light.push("  color-scheme: light;");
dark.push("  color-scheme: dark;");

fixed.push(`  --k-font-sans: ${t.font.sans};`, `  --k-font-mono: ${t.font.mono};`, `  --k-font-features: ${t.font.features};`);
for (const [name, s] of Object.entries(t.type)) {
  fixed.push(`  --k-text-${name}: ${s.size}px;`, `  --k-leading-${name}: ${s.lineHeight}px;`, `  --k-tracking-${name}: ${s.tracking}em;`);
}
for (const [size, c] of Object.entries(t.control)) {
  fixed.push(
    `  --k-control-${size}: ${c.height}px;`,
    `  --k-control-${size}-padding: ${c.padding}px;`,
    `  --k-control-${size}-icon: ${c.icon}px;`,
    `  --k-control-${size}-gap: ${c.gap}px;`,
  );
}
for (const [name, v] of Object.entries(t.row)) fixed.push(`  --k-row-${kebab(name)}: ${v}px;`);
for (const [name, v] of Object.entries(t.chrome)) fixed.push(`  --k-chrome-${kebab(name)}: ${v}px;`);
for (const [name, v] of Object.entries(t.radius)) fixed.push(`  --k-radius-${name}: ${v};`);
for (const [name, v] of Object.entries(t.motion.duration)) fixed.push(`  --k-duration-${name}: ${v}ms;`);
for (const [name, v] of Object.entries(t.motion.ease)) fixed.push(`  --k-ease-${name}: ${v};`);
fixed.push(
  `  --k-skeleton-duration: ${t.motion.skeleton.duration}ms;`,
  `  --k-skeleton-low: ${t.motion.skeleton.low};`,
  `  --k-spinner-turn: ${t.motion.spinner.turn}ms;`,
  `  --k-spinner-delay: ${t.motion.spinner.delay}ms;`,
);
for (const [name, v] of Object.entries(t.optical)) fixed.push(`  --k-optical-${name}: ${v}px;`);
for (const [name, v] of Object.entries(t.z)) fixed.push(`  --k-z-${name}: ${v};`);
fixed.push(
  `  --k-width-prose: ${t.width.prose};`,
  `  --k-width-settings: ${t.width.settings}px;`,
  `  --k-width-auth: ${t.width.auth}px;`,
  `  --k-width-tooltip: ${t.width.tooltipMax}px;`,
  `  --k-width-field-label: ${t.width.fieldLabel}px;`,
  `  --k-width-aside: ${t.width.aside}px;`,
  `  --k-width-menu-min: ${t.width.menuMin}px;`,
  `  --k-width-menu-max: ${t.width.menuMax}px;`,
);

function kebab(s: string) {
  return s.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}

writeFileSync(join(dist, "tokens.css"), tokensCss({ fixed, light, dark }));

const colorRoles: Record<string, string> = {
  panel: "bg-panel",
  chrome: "bg-chrome",
  raised: "bg-raised",
  tooltip: "bg-tooltip",
  toast: "toast-bg",
  scan: "scan",
  "on-scan": "on-scan",
  "toast-fg": "toast-fg",
  "toast-fg-secondary": "toast-fg-secondary",
  "toast-action": "toast-action",
  "toast-action-hover": "toast-action-hover",
  "toast-danger": "toast-danger",
  "fill-1": "fill-1",
  "item-hover": "item-hover",
  "item-pressed": "item-pressed",
  "fill-2": "fill-2",
  "fill-3": "fill-3",
  "line-subtle": "line-subtle",
  line: "line",
  "line-strong": "line-strong",
  "panel-ring": "panel-ring",
  fg: "text",
  "fg-secondary": "text-secondary",
  "fg-tertiary": "text-tertiary",
  "fg-disabled": "text-disabled",
  ink: "ink",
  "on-ink": "on-ink",
  focus: "focus",
  overlay: "overlay",
  "danger-fill": "danger-fill",
  "on-danger": "on-danger",
};
for (const role of Object.keys(t.semantic)) {
  colorRoles[role] = role;
  colorRoles[`${role}-text`] = `${role}-text`;
  colorRoles[`${role}-tint`] = `${role}-tint`;
}
for (const id of Object.keys(t.providerMark)) colorRoles[`mark-${id}`] = `mark-${id}`;
t.viz.forEach((_, i) => {
  colorRoles[`viz-${i + 1}`] = `viz-${i + 1}`;
  colorRoles[`viz-thin-${i + 1}`] = `viz-thin-${i + 1}`;
});

const theme: string[] = [];
theme.push("@theme inline {");
theme.push("  --color-*: initial;", "  --color-transparent: transparent;", "  --color-current: currentColor;", "  --color-inherit: inherit;");
for (const [name, v] of Object.entries(colorRoles)) theme.push(`  --color-${name}: var(--k-${v});`);
theme.push(
  "  --font-*: initial;",
  "  --font-sans: var(--k-font-sans);",
  "  --font-mono: var(--k-font-mono);",
  "  --default-font-family: var(--k-font-sans);",
  "  --default-mono-font-family: var(--k-font-mono);",
  "  --default-font-feature-settings: var(--k-font-features);",
);
theme.push(
  "  --font-weight-*: initial;",
  "  --font-weight-regular: var(--k-weight-regular);",
  "  --font-weight-medium: var(--k-weight-medium);",
  "  --font-weight-semibold: var(--k-weight-semibold);",
);
theme.push("  --shadow-*: initial;", "  --inset-shadow-*: initial;", "  --drop-shadow-*: initial;", "  --text-shadow-*: initial;");
for (const name of Object.keys(t.shadow)) theme.push(`  --shadow-${name}: var(--k-shadow-${name});`);
theme.push("}");
theme.push("");
theme.push("@theme {");
theme.push("  --spacing-*: initial;", "  --spacing: initial;");
for (const v of t.lengths) theme.push(`  --spacing-${v}: ${v}px;`);
theme.push("  --text-*: initial;");
for (const [name, s] of Object.entries(t.type)) {
  theme.push(`  --text-${name}: ${s.size}px;`, `  --text-${name}--line-height: ${s.lineHeight}px;`, `  --text-${name}--letter-spacing: ${s.tracking}em;`);
}
theme.push("  --tracking-*: initial;", "  --leading-*: initial;", "  --leading-none: 1;", "  --leading-code: 20px;");
theme.push("  --radius-*: initial;");
for (const [name, v] of Object.entries(t.radius)) theme.push(`  --radius-${name}: ${v};`);
theme.push("  --ease-*: initial;");
for (const [name, v] of Object.entries(t.motion.ease)) theme.push(`  --ease-${name}: ${v};`);
theme.push("  --animate-*: initial;");
theme.push("  --blur-*: initial;", "  --perspective-*: initial;", "  --aspect-*: initial;", "  --container-*: initial;");
theme.push(
  `  --container-prose: ${t.width.prose};`,
  `  --container-settings: ${t.width.settings}px;`,
  `  --container-auth: ${t.width.auth}px;`,
  `  --container-tooltip: ${t.width.tooltipMax}px;`,
  `  --container-palette: ${t.width.palette}px;`,
  `  --container-banner: ${t.width.banner}px;`,
  `  --container-kv: ${t.width.kv}px;`,
  `  --container-cell-sm: ${t.width.cellSm}px;`,
  `  --container-cell-md: ${t.width.cellMd}px;`,
  `  --container-cell-lg: ${t.width.cellLg}px;`,
);
theme.push(
  "  --breakpoint-*: initial;",
  "  --breakpoint-sm: 640px;",
  "  --breakpoint-md: 768px;",
  `  --breakpoint-lg: ${t.breakpoint.sidebar}px;`,
  "  --breakpoint-xl: 1280px;",
);
theme.push("  --default-transition-duration: 120ms;", "  --default-transition-timing-function: var(--k-ease-standard);");
theme.push("}");
writeFileSync(join(dist, "theme.css"), `\n${theme.join("\n")}\n`);

const resolve = <T extends Record<string, Mode>>(o: T) => Object.fromEntries(Object.entries(o).map(([k, m]) => [k, { light: m.light, dark: m.dark }]));
writeFileSync(
  join(dist, "tokens.json"),
  `${JSON.stringify(
    {
      brandVersion: t.brandVersion,
      neutral: resolve(t.neutral),
      semantic: t.semantic,
      dangerFill: t.dangerFill,
      viz: t.viz,
      providerMark: resolve(t.providerMark),
      tokenKindSeries: t.tokenKindSeries,
      tokenKindTint: t.tokenKindTint,
      type: t.type,
      weight: t.weight,
      font: t.font,
      control: t.control,
      row: t.row,
      chrome: t.chrome,
      space: t.space,
      lengths: t.lengths,
      radius: t.radius,
      shadow: resolve(t.shadow),
      material: resolve(t.material),
      motion: t.motion,
      z: t.z,
      optical: t.optical,
      width: t.width,
      breakpoint: t.breakpoint,
    },
    null,
    2,
  )}\n`,
);
console.log(`@logicplanes/kaeso-tokens: wrote tokens.css, theme.css, tokens.json (brand ${t.brandVersion})`);
