import { cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { compile, optimize } from "@tailwindcss/node";
import { Scanner } from "@tailwindcss/oxide";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const styles = join(root, "src/styles");
const dist = join(root, "dist");
mkdirSync(join(dist, "fonts"), { recursive: true });

cpSync(join(styles, "fonts"), join(dist, "fonts"), { recursive: true });
for (const f of ["fonts.css", "kaeso.css"]) cpSync(join(styles, f), join(dist, f));

const SOURCE_LINE = /^@source (not )?"([^"]+)";\r?(?:\n|$)/gm;
const SOURCES_IN_SRC = ["../components", "../blocks", "../theme", "../utils", "not ../utils/cn.ts", "not ../**/*.test.{ts,tsx}", "not ../**/docs.ts"];
const SOURCES_IN_DIST = ["./components", "./blocks", "./chunks", "./index.js", "not ./chunks/cn-*"];
const sourceLine = (s) => (s.startsWith("not ") ? `@source not "${s.slice(4)}";` : `@source "${s}";`);

const src = readFileSync(join(styles, "tailwind.css"), "utf8");
const found = [...src.matchAll(SOURCE_LINE)].map((m) => `${m[1] ?? ""}${m[2]}`);
if (found.join("\n") !== SOURCES_IN_SRC.join("\n")) {
  throw new Error(
    `src/styles/tailwind.css scans ${JSON.stringify(found)}, but scripts/build-css.mjs maps ${JSON.stringify(SOURCES_IN_SRC)} to dist. Update both together.`,
  );
}
let placed = false;
const tw = src.replace(SOURCE_LINE, () => {
  if (placed) return "";
  placed = true;
  return `${SOURCES_IN_DIST.map(sourceLine).join("\n")}\n`;
});
if (/@source (not )?"\.\.\//.test(tw)) throw new Error("dist/tailwind.css would still scan ../ sources");
writeFileSync(join(dist, "tailwind.css"), tw);

const stylesheet = readFileSync(join(styles, "styles.css"), "utf8");

async function scanned(base) {
  const compiler = await compile(stylesheet, { base, from: join(base, "styles.css"), onDependency: () => {} });
  if (compiler.root !== "none")
    throw new Error('src/styles/styles.css must import "tailwindcss" with source(none): only the @source entries of tailwind.css ship classes');
  return { compiler, candidates: new Scanner({ sources: compiler.sources }).scan() };
}

async function additions(known, extra) {
  const { compiler } = await scanned(dist);
  let before = compiler.build(known);
  return extra.filter((c) => {
    const after = compiler.build([c]);
    const adds = after !== before;
    before = after;
    return adds;
  });
}

function documentedUtilities() {
  const own = new Set([...readFileSync(join(styles, "kaeso.css"), "utf8").matchAll(/^@utility ([a-z][a-z0-9-]*) \{/gm)].map((m) => m[1]));
  const foundations = join(root, "foundations");
  const spans = readdirSync(foundations)
    .filter((f) => f.endsWith(".md"))
    .flatMap((f) => [...readFileSync(join(foundations, f), "utf8").matchAll(/`([^`\n]+)`/g)].flatMap((m) => m[1].split(/\s+/)));
  return [...new Set(spans)].filter((c) => own.has(c)).sort();
}

const published = await scanned(dist);
const inSource = new Set((await scanned(styles)).candidates);
const shared = published.candidates.filter((c) => inSource.has(c));
const rest = published.candidates.filter((c) => !inSource.has(c));
const covered = published.compiler.build(shared);
const css = published.compiler.build(rest);
if (css !== covered) {
  const missing = await additions(shared, rest);
  throw new Error(`dist ships classes that src/styles/tailwind.css does not scan: ${missing.join(" ")}. Add their folder to both source lists.`);
}
const documented = documentedUtilities();
if (published.compiler.build(documented) !== css) {
  const missing = await additions(published.candidates, documented);
  throw new Error(
    `foundations/*.md documents utilities that dist/styles.css lacks: ${missing.join(" ")}. Add them to src/styles/tailwind.css with @source inline(...).`,
  );
}
writeFileSync(join(dist, "styles.css"), optimize(css, { file: join(dist, "styles.css"), minify: true }).code);
console.log("@logicplanes/kaeso-ui: wrote styles.css, tailwind.css and fonts");
