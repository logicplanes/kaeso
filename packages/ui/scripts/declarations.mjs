import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const types = join(dirname(fileURLToPath(import.meta.url)), "..", "dist", "types");

function declarations(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((d) =>
    d.isDirectory() ? declarations(join(dir, d.name)) : d.name.endsWith(".d.ts") ? [join(dir, d.name)] : [],
  );
}

function nodeSpecifier(file, specifier) {
  const target = resolve(dirname(file), specifier);
  if (existsSync(`${target}.d.ts`)) return `${specifier}.js`;
  if (existsSync(join(target, "index.d.ts"))) return `${specifier.replace(/\/$/, "")}/index.js`;
  throw new Error(`${relative(types, file)}: ${specifier} resolves to no declaration`);
}

let rewritten = 0;
const files = declarations(types);
for (const file of files) {
  const code = readFileSync(file, "utf8");
  const out = code.replace(/(\bfrom\s*|\bimport\(\s*)(["'])(\.{1,2}(?:\/[^"']*)?)\2/g, (whole, head, quote, specifier) => {
    if (/\.(js|json)$/.test(specifier)) return whole;
    rewritten += 1;
    return `${head}${quote}${nodeSpecifier(file, specifier)}${quote}`;
  });
  if (out !== code) writeFileSync(file, out);
}
console.log(`declarations: ${rewritten} relative specifiers in ${files.length} files resolve under Node's ESM rules`);
