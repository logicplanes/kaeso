import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { lengths, motion } from "../../tokens/src/tokens.ts";
import { componentsWithoutRef } from "./component-refs.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const LENGTHS = new Set(lengths.map(String));
const DURATIONS = new Set(Object.values(motion.duration).map(String));
const SPACING_UTIL =
  /(?<![\w-])-?(?:p|px|py|pt|pr|pb|pl|ps|pe|m|mx|my|mt|mr|mb|ml|ms|me|gap|gap-x|gap-y|space-x|space-y|h|w|size|min-h|min-w|max-h|max-w|inset|inset-x|inset-y|top|right|bottom|left|start|end|translate-x|translate-y|scroll-mt|scroll-mb|scroll-m|basis)-(\d+(?:\.\d+)?)(?![\w./-])/g;

function allowsUndefined(type) {
  if (type.kind === ts.SyntaxKind.UndefinedKeyword) return true;
  if (ts.isParenthesizedTypeNode(type)) return allowsUndefined(type.type);
  return ts.isUnionTypeNode(type) && type.types.some(allowsUndefined);
}

function optionalWithoutUndefined(file, src) {
  const source = ts.createSourceFile(file, src, ts.ScriptTarget.Latest, true, file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const found = [];
  const visit = (node) => {
    if (ts.isPropertySignature(node) && node.questionToken && node.type && !allowsUndefined(node.type)) {
      found.push({ line: source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1, text: node.getText(source).replace(/\s+/g, " ") });
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return found;
}

const WITHOUT_REF = new Map([
  ["src/theme/theme-provider.tsx#ThemeProvider", "a context provider around its children"],
  ["src/i18n/provider.tsx#LocaleProvider", "a context provider around its children"],
  ["src/utils/link-provider.tsx#LinkProvider", "a context provider around its children"],
  ["src/utils/inset.tsx#InsetContent", "a context provider around its children"],
  ["src/utils/inset.tsx#GutterContent", "a context provider around its children"],
  ["src/utils/portal-context.tsx#PortalContainer", "a context provider; its host element is a mount point after the children, not a root around them"],
  ["src/components/tooltip/tooltip-provider.tsx#TooltipProvider", "a context provider around its children"],
  ["src/components/sidebar/sidebar.tsx#Provider", "a context provider around its children"],
  ["src/components/dialog/scope.tsx#DialogScope", "a context provider that remounts its children"],
  ["src/components/tooltip/tooltip.tsx#Tooltip", "its child is the trigger and takes its own ref"],
  ["src/components/popover/popover-root.tsx#PopoverRoot", "Base UI's root renders no element"],
  ["src/components/popover/popover-root.tsx#PopoverTrigger", "its child is the trigger and takes its own ref"],
  ["src/components/popover/popover-root.tsx#PopoverClose", "its child is the button and takes its own ref"],
  ["src/components/popover/hover-card.tsx#HoverRoot", "Base UI's root renders no element"],
  ["src/components/popover/hover-card.tsx#HoverTrigger", "its child is the trigger and takes its own ref"],
  ["src/components/menu/menu.tsx#Root", "Base UI's root renders no element"],
  ["src/components/menu/menu.tsx#Trigger", "its child is the trigger and takes its own ref"],
  ["src/components/menu/menu.tsx#Sub", "Base UI's submenu root renders no element"],
  ["src/components/menu/menu.tsx#ContextRoot", "Base UI's root renders no element"],
  ["src/components/dialog/dialog.tsx#Close", "its child is the button and takes its own ref"],
  ["src/blocks/pages/pages.tsx#ListPage", "a page header and body side by side, with no element around them"],
  ["src/blocks/pages/pages.tsx#OverviewPage", "a page header and body side by side, with no element around them"],
  ["src/blocks/pages/pages.tsx#EmptyPage", "a page header and body side by side, with no element around them"],
  ["src/components/date-picker/date-popup.tsx#DatePopup", "internal, loaded lazily by DatePicker"],
  ["src/components/date-picker/range-popup.tsx#RangePopup", "internal, loaded lazily by DateRangePicker"],
  ["src/components/date-picker/iso-field.tsx#IsoField", "internal to the date pickers' popups"],
  ["src/components/list/row-checkbox.tsx#RowCheckbox", "internal to List"],
  ["src/components/list/virtual-rows.tsx#VirtualRows", "internal, loaded lazily by List"],
  ["src/components/select/option-checkbox.tsx#OptionCheckbox", "internal to Select"],
  ["src/components/sidebar/sidebar-menu.tsx#SidebarMenuRoot", "internal, loaded lazily by the sidebar's menus"],
  ["src/components/tooltip/tooltip-root.tsx#TooltipRoot", "internal, loaded lazily by Tooltip"],
]);

const errors = [];
const report = (file, line, msg) => errors.push(`${relative(root, file)}:${line}  ${msg}`);

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (name === "node_modules" || name === "dist" || name.startsWith(".")) continue;
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

const componentDirs = [join(root, "src/components"), join(root, "src/blocks")];
const files = componentDirs
  .flatMap((d) => {
    try {
      return walk(d);
    } catch {
      return [];
    }
  })
  .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f));

for (const file of files) {
  const src = readFileSync(file, "utf8");
  const lines = src.split("\n");
  lines.forEach((line, i) => {
    const n = i + 1;
    if (/^\s*(\/\/|\*|\/\*)/.test(line)) return;
    const strings = [...line.matchAll(/"([^"\\]*(?:\\.[^"\\]*)*)"|`([^`]*)`|'([^'\\]*)'/g)].map((m) => m[1] ?? m[2] ?? m[3] ?? "");
    for (const s of strings) {
      if (/#[0-9a-fA-F]{3,8}\b/.test(s) && !/^#[\w-]+$/.test(s) && !/href|#ex-|#api/.test(s))
        report(file, n, `hex color literal in "${s.slice(0, 60)}"; use a token`);
      if (/\b(?:rgb|rgba|hsl|hsla|oklch|oklab)\(/.test(s)) report(file, n, `color function literal; use a token`);
      for (const m of s.matchAll(/(?<![\w-])((?:[a-z][\w-]*:)*)([a-z][\w-]*)-\[[^\]]+\]/g)) {
        const name = m[2];
        if (!/^(?:data|aria|supports|group-data|peer-data|has|group-has|not|group-aria)$/.test(name))
          report(file, n, `arbitrary Tailwind value in "${m[0]}"; add a token or a utility`);
      }
      if (/(?:^|\s)\[&[^\]]*\]:/.test(s)) report(file, n, `arbitrary variant; add a named variant or utility`);
      if (/(?:^|\s)dark:/.test(s)) report(file, n, `dark: variant; tokens switch themselves`);
      if (/(?:^|\s)transition-all\b|transition:\s*all/.test(s)) report(file, n, `transition-all`);
      if (/(?:^|\s)(?:[a-z-]+:)*border(?:-[xytrblse])?(?:-\d+)?(?:\s|$)/.test(s) || /(?:^|\s)(?:[a-z-]+:)*border-(?:line|fg|panel|raised|fill)/.test(s))
        report(file, n, `border utility; draw lines as rings (shadow) instead`);
      for (const m of s.matchAll(SPACING_UTIL)) {
        if (!LENGTHS.has(m[1])) report(file, n, `"${m[0]}" is off the length scale (${[...LENGTHS].join(", ")})`);
      }
      for (const m of s.matchAll(/(?<![\w-])duration-(\d+)/g)) if (!DURATIONS.has(m[1])) report(file, n, `duration-${m[1]} is not in the motion table`);
    }
  });
  if (/aria-current["']?\s*[:=]\s*\{?\s*["']step["']/.test(src))
    report(file, 1, `a numbered steps list (aria-current="step"); the library has no setup checklist, use StepDialog for a task in several steps`);
}

const sourceFiles = walk(join(root, "src")).filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f));
for (const file of sourceFiles) {
  const src = readFileSync(file, "utf8");
  for (const { line, text } of optionalWithoutUndefined(file, src)) report(file, line, `optional prop without "| undefined": ${text.slice(0, 60)}`);
  for (const { name, line } of componentsWithoutRef(file, src)) {
    const key = `${relative(root, file)}#${name}`;
    if (WITHOUT_REF.has(key)) WITHOUT_REF.delete(key);
    else report(file, line, `${name} renders an element but isn't a forwardRef; forward the ref to its root element`);
  }
  if (/\bforwardRef\b/.test(src)) {
    const names = [...src.matchAll(/(?:const|let)\s+(\w+)\s*=\s*forwardRef/g)].map((m) => m[1]);
    for (const name of names) if (!new RegExp(`${name}\\.displayName\\s*=`).test(src)) report(file, 1, `${name} has no displayName`);
  }
}

for (const key of WITHOUT_REF.keys())
  errors.push(`${key}  is listed as a component without a ref, but no such exported plain function component exists; remove it from WITHOUT_REF`);

for (const dir of componentDirs) {
  let entries = [];
  try {
    entries = readdirSync(dir);
  } catch {
    continue;
  }
  for (const name of entries) {
    const d = join(dir, name);
    if (!statSync(d).isDirectory() || name.startsWith("_")) continue;
    if (/^(?:steps?|stepper|checklist|setup(?:-checklist)?|onboarding)$/.test(name))
      report(d, 1, `a setup checklist or numbered steps component; the library has none, use StepDialog for a task in several steps`);
    try {
      statSync(join(d, "index.ts"));
    } catch {
      continue;
    }
    const all = readdirSync(d)
      .filter((f) => f.endsWith(".tsx") && !f.includes(".test."))
      .map((f) => readFileSync(join(d, f), "utf8"))
      .join("\n");
    if (!/export const KAESO_\w+_VARIANTS\s*=/.test(all)) report(d, 1, `missing a KAESO_*_VARIANTS export`);
    if (!/export const KAESO_\w+_DEFAULT_VARIANTS\s*=/.test(all)) report(d, 1, `missing a KAESO_*_DEFAULT_VARIANTS export`);
  }
}

if (errors.length) {
  console.error(errors.join("\n"));
  console.error(`\nlint: ${errors.length} problem${errors.length === 1 ? "" : "s"}`);
  process.exit(1);
}
console.log(`lint: ${files.length} files clean`);
