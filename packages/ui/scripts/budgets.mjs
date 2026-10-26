import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { gzipSync } from "node:zlib";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(packageRoot, "dist");
const budgets = { "components/button.js": 3.8, "components/menu.js": 9, "components/data-table.js": 22 };
const cssBudget = 28;

function closure(file, seen = new Set()) {
  if (seen.has(file)) return seen;
  seen.add(file);
  const src = readFileSync(file, "utf8");
  for (const m of src.matchAll(/(?:import|export)\s[^"']*?from\s*["'](\.{1,2}\/[^"']+)["']|import\s*["'](\.{1,2}\/[^"']+)["']/g)) {
    const next = resolve(dirname(file), m[1] ?? m[2]);
    if (existsSync(next) && next.endsWith(".js")) closure(next, seen);
  }
  return seen;
}
const kb = (n) => Math.round((n / 1024) * 10) / 10;
const gz = (files) => gzipSync(Buffer.concat([...files].map((f) => readFileSync(f)))).length;
const entries = readdirSync(join(dist, "components"))
  .filter((f) => f.endsWith(".js"))
  .map((f) => join(dist, "components", f));
const uses = new Map();
for (const e of entries) for (const f of closure(e)) if (f.includes("/chunks/")) uses.set(f, (uses.get(f) ?? 0) + 1);
const foundation = new Set([...uses].filter(([, n]) => n >= entries.length * 0.4).map(([f]) => f));
const foundationBudget = 8;
const failures = [];
const lines = [];
const fsize = gz(foundation);
lines.push(`foundation: ${kb(fsize)} kB gzipped (budget ${foundationBudget} kB, ${foundation.size} chunks shared by most entries)`);
if (kb(fsize) > foundationBudget) failures.push(`the shared foundation is ${kb(fsize)} kB gzipped, over ${foundationBudget} kB`);
const entryNames = new Set(entries.map((e) => e.split("/").pop().replace(/\.js$/, "")));
const chunkName = (f) =>
  f
    .split("/")
    .pop()
    .replace(/-[\w-]{8}\.js$/, "");
for (const [entry, limit] of Object.entries(budgets)) {
  const own = entry.split("/").pop().replace(/\.js$/, "");
  const files = [...closure(join(dist, entry))].filter(
    (f) => !foundation.has(f) && !(f.includes("/chunks/") && entryNames.has(chunkName(f)) && chunkName(f) !== own),
  );
  const size = gz(files);
  lines.push(`${entry}: ${kb(size)} kB gzipped of its own (budget ${limit} kB, ${files.length} files)`);
  if (kb(size) > limit) failures.push(`${entry} is ${kb(size)} kB gzipped, over its ${limit} kB budget`);
}
const { parseAst } = await import(createRequire(join(packageRoot, "package.json")).resolve("vite"));
const PURE = /\/\*\s*[#@]__PURE__\s*\*\/\s*$/;
const DEV_ONLY = /^process\.env\.NODE_ENV !== "production"$/;
function pure(n, code) {
  if (!n) return true;
  switch (n.type) {
    case "Literal":
    case "Identifier":
    case "FunctionExpression":
    case "ArrowFunctionExpression":
      return true;
    case "TemplateLiteral":
      return n.expressions.every((e) => pure(e, code));
    case "ArrayExpression":
      return n.elements.every((e) => e && pure(e.type === "SpreadElement" ? e.argument : e, code));
    case "ObjectExpression":
      return n.properties.every((p) => p.type === "Property" && (!p.computed || pure(p.key, code)) && pure(p.value, code));
    case "MemberExpression":
      return !n.computed && (n.object.type === "Identifier" || (n.object.type === "MemberExpression" && pure(n.object, code)));
    case "UnaryExpression":
      return pure(n.argument, code);
    case "BinaryExpression":
    case "LogicalExpression":
      return pure(n.left, code) && pure(n.right, code);
    case "ConditionalExpression":
      return pure(n.test, code) && pure(n.consequent, code) && pure(n.alternate, code);
    case "CallExpression":
    case "NewExpression":
      return PURE.test(code.slice(Math.max(0, n.start - 40), n.start)) && n.arguments.every((a) => pure(a.type === "SpreadElement" ? a.argument : a, code));
    default:
      return false;
  }
}
const kept = [];
const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : f.endsWith(".js") ? [join(d, f)] : []));
for (const file of walk(dist)) {
  const code = readFileSync(file, "utf8");
  for (const s of parseAst(code).body) {
    const d = s.type === "ExportNamedDeclaration" || s.type === "ExportDefaultDeclaration" ? s.declaration : s;
    if (!d || ["ImportDeclaration", "ExportAllDeclaration", "FunctionDeclaration"].includes(d.type)) continue;
    const ok =
      d.type === "VariableDeclaration"
        ? d.declarations.every((x) => pure(x.init, code))
        : d.type === "ExpressionStatement"
          ? d.expression.type === "Literal" ||
            (d.expression.type === "LogicalExpression" && DEV_ONLY.test(code.slice(d.expression.left.start, d.expression.left.end)))
          : false;
    if (!ok) kept.push(`${file.slice(dist.length + 1)}: ${code.slice(s.start, s.end).replace(/\s+/g, " ").slice(0, 100)}`);
  }
}
lines.push(`tree-shaking: ${kept.length} module-level statements a bundler must keep`);
const guarded = walk(dist).filter((f) => /typeof process\b/.test(readFileSync(f, "utf8")));
lines.push(`development checks: ${guarded.length} files guard NODE_ENV with typeof process`);
for (const f of guarded)
  failures.push(`${f.slice(dist.length + 1)} guards process.env.NODE_ENV with typeof process: browser bundles keep its development checks`);
for (const k of kept) failures.push(`kept at module level (mark it pure or move it into a function): ${k}`);

const i18nEntry = join(dist, "i18n.js");
const i18nSrc = readFileSync(i18nEntry, "utf8");
const from = (name) => {
  const local = new RegExp(`(\\w+) as ${name}\\b`).exec(i18nSrc.slice(i18nSrc.lastIndexOf("export {")))?.[1];
  const m = local && new RegExp(`import \\{[^}]*(?:\\b\\w+ as ${local}\\b|\\b${local}\\b(?! as))[^}]*\\} from "(\\./[^"]+)"`).exec(i18nSrc);
  return m ? resolve(dist, m[1]) : null;
};
for (const name of ["formatMessage", "useUiText", "useLocale", "matchUiLocale"]) {
  const chunk = from(name);
  if (!chunk) failures.push(`i18n.js: can't tell which chunk ${name} comes from`);
  else if ([...closure(chunk)].some((f) => f === join(dist, "format.js")))
    failures.push(`${name}'s chunk (${chunk.slice(dist.length + 1)}) imports format.js: an app that only formats messages would load the formatters`);
  else if (name === "formatMessage") lines.push(`i18n: formatMessage alone is ${kb(gz(closure(chunk)))} kB gzipped, without the formatters`);
}

const css = gzipSync(readFileSync(join(dist, "styles.css"))).length;
lines.push(`styles.css: ${kb(css)} kB gzipped (budget ${cssBudget} kB)`);
if (kb(css) > cssBudget) failures.push(`styles.css is ${kb(css)} kB gzipped, over ${cssBudget} kB`);
for (const l of lines) console.log(`  ${l}`);
for (const f of failures) console.error(`  ✗ ${f}`);
console.log(failures.length ? `bundle budgets: ${failures.length} failures` : "bundle budgets: within budget");
process.exit(failures.length ? 1 : 0);
