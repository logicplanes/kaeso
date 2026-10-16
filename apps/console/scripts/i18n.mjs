import { readFileSync, readdirSync, writeFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = join(fileURLToPath(import.meta.url), "../..");
const SRC = join(root, "src");
const LOCALES = join(SRC, "i18n/locales");
export const TRANSLATED = ["de", "zh-CN"];
export const AREAS = [
  "common",
  "shell",
  "auth",
  "errors",
  "rights",
  "format",
  "kaeso",
  "signIn",
  "public",
  "account",
  "org",
  "orgHome",
  "instance",
  "project",
  "keys",
  "limits",
  "logs",
  "models",
  "playground",
];
const KEY = /^[a-z][a-zA-Z0-9]*(\.[a-z][a-zA-Z0-9]*)+$/;

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "generated" || name === "locales" ? [] : files(path);
    return /\.(ts|tsx)$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : [];
  });
}

export function extract() {
  const keys = new Map();
  const problems = [];
  for (const file of files(SRC).filter((f) => !f.endsWith(join("i18n", "rich.tsx")))) {
    const text = readFileSync(file, "utf8");
    if (!/\bt\(/.test(text)) continue;
    const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, file.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
    const imported = source.statements.some(
      (s) =>
        ts.isImportDeclaration(s) &&
        /(^~\/i18n(\/rich)?$|^\.\.?(\/\.\.)*\/i18n(\/rich)?$|^\.\/index$)/.test(s.moduleSpecifier.text) &&
        s.importClause?.namedBindings &&
        ts.isNamedImports(s.importClause.namedBindings) &&
        s.importClause.namedBindings.elements.some((e) => e.name.text === "t" || e.name.text === "tRich"),
    );
    if (!imported) continue;
    const where = (node) => `${relative(root, file)}:${source.getLineAndCharacterOfPosition(node.getStart()).line + 1}`;
    const visit = (node) => {
      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && (node.expression.text === "t" || node.expression.text === "tRich")) {
        const [key, english] = node.arguments;
        const literal = (n) => n && (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n));
        if (!literal(key) || !literal(english)) problems.push(`${where(node)}: t() takes a literal key and literal English`);
        else if (!KEY.test(key.text) || !AREAS.includes(key.text.split(".")[0]))
          problems.push(`${where(node)}: "${key.text}" is not <area>.<words> with an area of ${AREAS.join(", ")}`);
        else {
          const seen = keys.get(key.text);
          if (seen && seen.english !== english.text) problems.push(`${where(node)}: "${key.text}" is "${seen.english}" at ${seen.where}; one key, one English`);
          else if (!seen) keys.set(key.text, { english: english.text, where: where(node) });
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return { keys, problems };
}

const read = (path) => (existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : {});
const json = (o) => `${JSON.stringify(o, null, 2)}\n`;
const sorted = (entries) => Object.fromEntries([...entries].sort(([a], [b]) => (a < b ? -1 : 1)));

export function placeholders(message) {
  const found = new Set();
  const body = (i) => {
    while (i < message.length && message[i] !== "}") {
      if (message[i] !== "{") {
        i++;
        continue;
      }
      const m = /^\{\s*([a-zA-Z0-9_]+)\s*(?:,\s*(plural|select)\s*,)?/.exec(message.slice(i));
      if (!m) {
        i++;
        continue;
      }
      i += m[0].length;
      if (!m[2]) {
        found.add(m[1]);
        i = message.indexOf("}", i) + 1 || message.length;
        continue;
      }
      found.add(`${m[1]}:${m[2]}`);
      while (i < message.length) {
        while (/\s/.test(message[i] ?? "")) i++;
        if (message[i] === "}") {
          i++;
          break;
        }
        while (i < message.length && message[i] !== "{" && message[i] !== "}") i++;
        if (message[i] !== "{") continue;
        i = body(i + 1) + 1;
      }
    }
    return i;
  };
  body(0);
  return [...found].sort().join(" ");
}

export function check({ write = false } = {}) {
  const { keys, problems } = extract();
  const byArea = new Map(AREAS.map((a) => [a, new Map()]));
  for (const [key, { english }] of keys) byArea.get(key.split(".")[0]).set(key, english);
  for (const area of AREAS) {
    const path = join(LOCALES, "en", `${area}.json`);
    const want = json(sorted(byArea.get(area)));
    const have = existsSync(path) ? readFileSync(path, "utf8") : "";
    if (write && want !== have) {
      mkdirSync(join(LOCALES, "en"), { recursive: true });
      writeFileSync(path, want);
    } else if (want !== have) problems.push(`src/i18n/locales/en/${area}.json is not the source's English: run pnpm i18n --write`);
  }
  for (const locale of TRANSLATED) {
    for (const area of AREAS) {
      const file = `src/i18n/locales/${locale}/${area}.json`;
      const catalog = read(join(root, file));
      const english = byArea.get(area);
      for (const [key, text] of english) {
        const translated = catalog[key];
        if (typeof translated !== "string" || !translated.trim()) problems.push(`${file}: "${key}" is missing (${JSON.stringify(text)})`);
        else if (placeholders(translated) !== placeholders(text))
          problems.push(`${file}: "${key}" has placeholders {${placeholders(translated)}}, the English {${placeholders(text)}}`);
      }
      for (const key of Object.keys(catalog)) if (!english.has(key)) problems.push(`${file}: "${key}" is not used in the source`);
    }
  }
  return { keys: keys.size, problems };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const { keys, problems } = check({ write: process.argv.includes("--write") });
  for (const p of problems) console.error(p);
  if (problems.length) {
    console.error(`i18n: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
    process.exit(1);
  }
  console.log(`i18n: ${keys} keys in English, German and Chinese`);
}
