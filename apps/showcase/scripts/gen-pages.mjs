import { readdirSync, readFileSync, writeFileSync, writeSync, mkdirSync, rmSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import ts from "typescript";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const demosDir = join(root, "src/demos");
const genDir = join(root, "src/generated");
const pagesDir = join(root, "src/pages/components");
mkdirSync(genDir, { recursive: true });

const registry = join(root, "../../packages/ui/ai/component-registry.json");
if (!existsSync(registry)) execFileSync("node", [join(root, "../../packages/ui/scripts/registry.mjs")], { stdio: "inherit" });

function literal(node) {
  if (!node) return undefined;
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
  if (ts.isNumericLiteral(node)) return Number(node.text);
  if (node.kind === ts.SyntaxKind.TrueKeyword) return true;
  if (node.kind === ts.SyntaxKind.FalseKeyword) return false;
  if (ts.isArrayLiteralExpression(node)) return node.elements.map(literal);
  if (ts.isObjectLiteralExpression(node)) {
    const o = {};
    for (const p of node.properties) if (ts.isPropertyAssignment(p)) o[p.name.getText().replace(/^["']|["']$/g, "")] = literal(p.initializer);
    return o;
  }
  if (ts.isAsExpression(node) || ts.isSatisfiesExpression?.(node)) return literal(node.expression);
  return undefined;
}

function dedent(text) {
  const lines = text.replace(/^\n+|\s+$/g, "").split("\n");
  const indent = Math.min(...lines.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length));
  return lines.map((l) => l.slice(indent)).join("\n");
}

function codeOf(fn, src) {
  const body = fn.body;
  if (!body) return "";
  const stmts = body.statements;
  if (stmts.length === 1 && ts.isReturnStatement(stmts[0]) && stmts[0].expression) {
    let e = stmts[0].expression;
    if (ts.isParenthesizedExpression(e)) e = e.expression;
    return dedent(src.slice(e.getStart(), e.getEnd()));
  }
  return dedent(src.slice(fn.getStart(), fn.getEnd()).replace(/^export\s+/, ""));
}

const out = {};
const files = readdirSync(demosDir).filter((f) => f.endsWith(".tsx") && !f.startsWith("_"));
for (const file of files) {
  const slug = file.replace(/\.tsx$/, "");
  const src = readFileSync(join(demosDir, file), "utf8");
  const sf = ts.createSourceFile(file, src, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let meta = {};
  let examples = [];
  const fns = {};
  const exportsList = [];
  for (const st of sf.statements) {
    const exported = st.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword);
    if (!exported) continue;
    if (ts.isVariableStatement(st)) {
      for (const d of st.declarationList.declarations) {
        const name = d.name.getText();
        if (name === "meta") meta = literal(d.initializer);
        else if (name === "examples") examples = literal(d.initializer);
        else if (d.initializer && (ts.isArrowFunction(d.initializer) || ts.isFunctionExpression(d.initializer))) fns[name] = d.initializer;
      }
    } else if (ts.isFunctionDeclaration(st) && st.name) {
      fns[st.name.text] = st;
      exportsList.push(st.name.text);
    }
  }
  if (!meta?.title) throw new Error(`${file}: export const meta = { title, description, group, … } is required`);
  for (const ex of examples) {
    if (!fns[ex.fn]) throw new Error(`${file}: example "${ex.id}" names missing function ${ex.fn}`);
    ex.code = codeOf(fns[ex.fn], src);
  }
  const dos = exportsList.filter((n) => /^Do[A-Z]/.test(n)).map((n) => ({ fn: n, code: codeOf(fns[n], src) }));
  const donts = exportsList.filter((n) => /^Dont[A-Z]/.test(n)).map((n) => ({ fn: n, code: codeOf(fns[n], src) }));
  out[slug] = {
    slug,
    ...meta,
    examples,
    dos,
    donts,
    hasStates: exportsList.includes("States"),
    hasAnatomy: exportsList.includes("Anatomy"),
    hasPlayground: exportsList.includes("Playground"),
  };
}
writeFileSync(join(genDir, "demos.json"), `${JSON.stringify(out, null, 2)}\n`);

const wrapDir = join(genDir, "islands");
rmSync(wrapDir, { recursive: true, force: true });
mkdirSync(wrapDir, { recursive: true });
for (const [slug, page] of Object.entries(out)) {
  const names = [...new Set([...page.examples.map((e) => e.fn), ...(page.hasStates ? ["States"] : [])])];
  writeFileSync(
    join(wrapDir, `${slug}.tsx`),
    `import { PortalContainer, TooltipProvider } from "@logicplanes/kaeso-ui";\nimport { DocsLocale } from "~/lib/locale";\nimport * as D from "~/demos/${slug}";\n\n${names.map((n) => `export function ${n}() {\n  return (\n    <DocsLocale>\n      <TooltipProvider>\n        <PortalContainer>\n          <D.${n} />\n        </PortalContainer>\n      </TooltipProvider>\n    </DocsLocale>\n  );\n}`).join("\n\n")}\n`,
  );
}

const blocksDir = join(root, "src/pages/blocks");
rmSync(pagesDir, { recursive: true, force: true });
rmSync(blocksDir, { recursive: true, force: true });
mkdirSync(pagesDir, { recursive: true });
const ident = (s) => s.replace(/-([a-z])/g, (_, c) => c.toUpperCase()).replace(/^[a-z]/, (c) => c.toUpperCase());
for (const [slug, page] of Object.entries(out)) {
  const D = `D${ident(slug)}`;
  const previews = `    <Preview slot="primary" example={page.examples[0]}><${D}.${page.examples[0].fn} client:visible /></Preview>`;
  if (page.group !== "Blocks")
    writeFileSync(
      join(pagesDir, `${slug}.astro`),
      `---
import DocPage from "~/layouts/DocPage.astro";
import Preview from "~/components/Preview.astro";
import * as ${D} from "~/generated/islands/${slug}";
import demos from "~/generated/demos.json";
const page = demos["${slug}"];
---
<DocPage page={page}>
${previews}
</DocPage>
`,
    );
  mkdirSync(join(pagesDir, slug), { recursive: true });
  writeFileSync(
    join(pagesDir, slug, "examples.astro"),
    `---
import ExamplesPage from "~/layouts/ExamplesPage.astro";
import Preview from "~/components/Preview.astro";
import * as ${D} from "~/generated/islands/${slug}";
import demos from "~/generated/demos.json";
const page = demos["${slug}"];
---
<ExamplesPage page={page}>
${page.examples.map((ex, i) => `  <Preview example={page.examples[${i}]} heading><${D}.${ex.fn} client:visible /></Preview>`).join("\n")}
</ExamplesPage>
`,
  );
  if (page.hasStates) {
    mkdirSync(join(pagesDir, slug), { recursive: true });
    writeFileSync(
      join(pagesDir, slug, "states.astro"),
      `---
import StatesPage from "~/layouts/StatesPage.astro";
import * as ${D} from "~/generated/islands/${slug}";
import demos from "~/generated/demos.json";
const page = demos["${slug}"];
---
<StatesPage page={page}>
  <${D}.States slot="light" client:load />
  <${D}.States slot="dark" client:load />
</StatesPage>
`,
    );
  }
}
const pkg = JSON.parse(readFileSync(join(root, "../../packages/ui/package.json"), "utf8"));
const guide = [
  "# @logicplanes/kaeso-ui component guides",
  "",
  `Version ${pkg.version}. Generated from the showcase's demo modules (apps/showcase/src/demos); do not edit. For each component: what it is for, its import, keyboard, accessibility, do and don't, and every example's code. Props and variants: \`component-registry.md\`.`,
  "",
];
const tidy = (code) => {
  const [first, ...rest] = code.split("\n");
  const indent = Math.min(...rest.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length));
  return [first, ...rest.map((l) => l.slice(Number.isFinite(indent) ? indent : 0))].join("\n");
};
const fence = (code) => ["```tsx", tidy(code), "```", ""];
for (const page of Object.values(out).sort((a, b) => a.title.localeCompare(b.title))) {
  const names = page.import ?? page.title.replace(/\s+/g, "");
  const from = page.barrel === false ? `@logicplanes/kaeso-ui/${page.entry ?? page.slug}` : "@logicplanes/kaeso-ui";
  guide.push(`## ${page.title}`, "", page.description ?? "", "", `\`import { ${names} } from "${from}";\``, "");
  if (page.keyboard?.length) guide.push("### Keyboard", "", "| Key | Action |", "|---|---|", ...page.keyboard.map(([k, a]) => `| ${k} | ${a} |`), "");
  if (page.a11y?.length) guide.push("### Accessibility", "", ...page.a11y.map((n) => `- ${n}`), "");
  if (page.dos.length || page.donts.length) {
    guide.push("### Do and don't", "");
    for (const d of page.dos)
      guide.push(
        `Do (${d.fn
          .replace(/^Do/, "")
          .replace(/([a-z])([A-Z])/g, "$1 $2")
          .toLowerCase()}):`,
        "",
        ...fence(d.code),
      );
    for (const d of page.donts)
      guide.push(
        `Don't (${d.fn
          .replace(/^Dont/, "")
          .replace(/([a-z])([A-Z])/g, "$1 $2")
          .toLowerCase()}):`,
        "",
        ...fence(d.code),
      );
  }
  guide.push("### Examples", "");
  for (const ex of page.examples) guide.push(`#### ${ex.title}`, "", ...(ex.description ? [ex.description, ""] : []), ...fence(ex.code));
}
writeFileSync(join(root, "../../packages/ui/ai/component-guides.md"), `${guide.join("\n").replace(/\n{3,}/g, "\n\n")}\n`);
const registryJson = JSON.parse(readFileSync(registry, "utf8"));
const norm = (t) => t.replace(/\s+/g, " ").trim();
const HIDDEN_PROPS = new Set(["className", "data-force", "style"]);
const shown = new Set();
for (const page of Object.values(out)) {
  if (page.group === "Blocks") continue;
  shown.add(norm(page.description ?? ""));
  for (const ex of page.examples) {
    shown.add(norm(ex.title));
    if (ex.description) shown.add(norm(ex.description));
  }
  const main = (page.import ?? page.title.replace(/\s+/g, "")).split(",")[0].trim();
  const own = registryJson.components.find((c) => c.entry === (page.entry ?? page.slug))?.components.find((c) => c.name === main);
  for (const p of own?.props ?? []) if (!HIDDEN_PROPS.has(p.name) && !p.deprecated) shown.add(norm(p.description));
}
const untranslated = [];
for (const lang of ["de", "zh-CN"]) {
  const dict = JSON.parse(readFileSync(join(root, `src/i18n/${lang}.json`), "utf8"));
  for (const key of shown) if (key && !(key in dict)) untranslated.push(`${lang}: ${key}`);
}
if (untranslated.length) {
  writeSync(2, `docs: ${untranslated.length} texts missing from src/i18n:\n  ${untranslated.join("\n  ")}\n`);
  process.exit(1);
}
console.log(`docs: generated ${Object.keys(out).length} component pages`);
