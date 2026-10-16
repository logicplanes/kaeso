import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { lengths, motion } from "../../tokens/src/tokens.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const LENGTHS = new Set(lengths.map(String));
const DURATIONS = new Set(Object.values(motion.duration).map(String));
const SPACING_UTIL =
  /(?<![\w-])-?(?:p|px|py|pt|pr|pb|pl|ps|pe|m|mx|my|mt|mr|mb|ml|ms|me|gap|gap-x|gap-y|space-x|space-y|h|w|size|min-h|min-w|max-h|max-w|inset|inset-x|inset-y|top|right|bottom|left|start|end|translate-x|translate-y|scroll-mt|scroll-mb|scroll-m|basis)-(\d+(?:\.\d+)?)(?![\w./-])/g;

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
  let inProps = false;
  let depth = 0;
  lines.forEach((line, i) => {
    if (/^(export )?interface \w+(Props)?\b[^{]*\{/.test(line)) {
      inProps = true;
      depth = 0;
    }
    if (!inProps) return;
    depth += (line.match(/\{/g) ?? []).length - (line.match(/\}/g) ?? []).length;
    const m = line.match(/^ {2}["\w-]+\?: (.+);\s*$/);
    if (m && !/\| undefined\)?$/.test(m[1])) report(file, i + 1, `optional prop without "| undefined": ${line.trim().slice(0, 60)}`);
    if (depth <= 0 && line.includes("}")) inProps = false;
  });
  if (/forwardRef\(/.test(src)) {
    const names = [...src.matchAll(/(?:const|let)\s+(\w+)\s*=\s*forwardRef/g)].map((m) => m[1]);
    for (const name of names) if (!new RegExp(`${name}\\.displayName\\s*=`).test(src)) report(file, 1, `${name} has no displayName`);
  }
}

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
