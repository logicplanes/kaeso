import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const t = JSON.parse(readFileSync(join(root, "../tokens/dist/tokens.json"), "utf8"));
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const kebab = (k) => k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
const table = (head, rows) => [
  `| ${head.join(" | ")} |`,
  `|${head.map(() => "---").join("|")}|`,
  ...rows.map((r) => `| ${r.map((c) => String(c ?? "").replace(/\|/g, "\\|")).join(" | ")} |`),
  "",
];
const code = (s) => (s ? `\`${s}\`` : "");

const md = [
  `# ${pkg.name} tokens`,
  "",
  `Version ${pkg.version}. Generated from \`@logicplanes/kaeso-tokens\`; do not edit. Components use these through role names (\`bg-panel\`, \`text-fg-secondary\`, \`h-36\`, \`rounded-pill\`); a raw value never appears in a component. How to use them: \`foundations/*.md\`.`,
  "",
  "## Neutrals",
  "",
  ...table(
    ["Variable", "Light", "Dark"],
    Object.entries(t.neutral).map(([k, m]) => [code(`--k-${k}`), m.light, m.dark]),
  ),
  "## Status",
  "",
  ...table(
    ["Variable", "Light", "Dark"],
    Object.entries(t.semantic).flatMap(([k, s]) => [
      [code(`--k-${k}`), s.solid.light, s.solid.dark],
      [code(`--k-${k}-text`), s.text.light, s.text.dark],
      [code(`--k-${k}-tint`), s.tint.light, s.tint.dark],
    ]),
  ),
  "## Data",
  "",
  ...table(
    ["Variable", "Series", "Light", "Dark"],
    t.viz.map((v, i) => [code(`--k-viz-${i + 1}`), v.name, v.light, v.dark]),
  ),
  "## Type",
  "",
  ...table(
    ["Utility", "Size / line height", "Weight", "Tracking"],
    Object.entries(t.type).map(([k, s]) => [code(`text-${k}`), `${s.size}/${s.lineHeight}px`, s.weight, s.tracking ? `${s.tracking}em` : ""]),
  ),
  "## Controls",
  "",
  ...table(
    ["Variable", "Height", "Padding", "Icon", "Gap"],
    Object.entries(t.control).map(([k, c]) => [code(`--k-control-${k}`), `${c.height}px`, c.padding, c.icon, c.gap]),
  ),
  "## Rows",
  "",
  ...table(
    ["Variable", "Height"],
    Object.entries(t.row).map(([k, v]) => [code(`--k-row-${kebab(k)}`), `${v}px`]),
  ),
  "## Radii",
  "",
  ...table(
    ["Utility", "Value"],
    Object.entries(t.radius).map(([k, v]) => [code(`rounded-${k}`), v === "9999px" ? "pill" : v]),
  ),
  "## Elevation",
  "",
  ...table(
    ["Utility", "Light", "Dark"],
    Object.entries(t.shadow).map(([k, v]) => [code(`shadow-${k}`), code(v.light ?? v), code(v.dark ?? v)]),
  ),
  "## Motion",
  "",
  ...table(
    ["Variable", "Value"],
    [
      ...Object.entries(t.motion.duration).map(([k, v]) => [code(`--k-duration-${k}`), `${v}ms`]),
      ...Object.entries(t.motion.ease).map(([k, v]) => [code(`--k-ease-${k}`), v]),
      [code("--k-spinner-turn"), `${t.motion.spinner.turn}ms`],
      [code("--k-skeleton-duration"), `${t.motion.skeleton.duration}ms`],
      ["spinner delay (before it shows)", `${t.motion.spinner.delay}ms`],
      ["spinner minimum (once shown)", `${t.motion.spinner.minimum}ms`],
    ],
  ),
  "## Layers",
  "",
  ...table(
    ["Variable", "Value"],
    Object.entries(t.z).map(([k, v]) => [code(`--k-z-${k}`), v]),
  ),
  "## Lengths",
  "",
  `Every length a component may use. The class suffix is the pixel value: \`h-36\` is 36px. Spacing scale: ${t.space.join(", ")}.`,
  "",
  t.lengths.join(", "),
  "",
];
mkdirSync(join(root, "ai"), { recursive: true });
writeFileSync(join(root, "ai/tokens.md"), `${md.join("\n")}\n`);

const groupsScript = `import(${JSON.stringify(join(root, "src/icons/groups.ts"))}).then((m) => process.stdout.write(JSON.stringify(m.ICON_GROUPS)));`;
const groups = JSON.parse(execFileSync(join(root, "node_modules/.bin/tsx"), ["--eval", groupsScript], { cwd: root, encoding: "utf8" }));
const icons = [
  "# Icon vocabulary",
  "",
  `Version ${pkg.version}. Generated from \`src/icons/vocabulary.ts\`; do not edit. Use the name, never an icon picked by look: \`icons.key\` always means a key. Hugeicons, Stroke Rounded. Rules: \`foundations/icons.md\`.`,
  "",
  ...Object.entries(groups).flatMap(([group, names]) => [`## ${group}`, "", names.map((n) => `\`${n}\``).join(", "), ""]),
];
writeFileSync(join(root, "ai/icons.md"), `${icons.join("\n")}\n`);
console.log("references: ai/tokens.md, ai/icons.md");
