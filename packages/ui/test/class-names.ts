import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repository = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const SOURCES = ["packages/ui/src", "apps/console/src", "apps/docs/src", "apps/showcase/src"];
const SOURCE_FILE = /\.(?:tsx?|astro|mdx?)$/;
const MERGER_CONFIGURATION = join(repository, "packages/ui/src/utils/cn.ts");
const CLASS_TOKEN = /^!?-?[a-z*@[(][\w\-:/.[\]()%=&>*~+'"!@,#]*!?$/i;

function files(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === "generated" || name.startsWith(".")) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) files(path, out);
    else if (SOURCE_FILE.test(name) && path !== MERGER_CONFIGURATION) out.push(path);
  }
  return out;
}

function literals(source: string): string[] {
  return [...source.matchAll(/"((?:[^"\\\n]|\\.)*)"|'((?:[^'\\\n]|\\.)*)'|`([^`]*)`/g)].map((m) => (m[1] ?? m[2] ?? m[3] ?? "").replace(/\$\{[^}]*\}/g, " "));
}

export interface SourceClasses {
  lists: string[];
  tokens: string[];
}

export function sourceClasses(): SourceClasses {
  const lists = new Set<string>();
  const tokens = new Set<string>();
  for (const root of SOURCES) {
    for (const file of files(join(repository, root))) {
      for (const text of literals(readFileSync(file, "utf8"))) {
        const parts = text.split(/\s+/).filter((t) => t && CLASS_TOKEN.test(t));
        if (!parts.length) continue;
        parts.forEach((t) => tokens.add(t));
        if (parts.length > 1) lists.add(parts.join(" "));
      }
    }
  }
  return { lists: [...lists].sort(), tokens: [...tokens].sort() };
}

const styles = join(repository, "packages/ui/src/styles/styles.css");

async function loadStylesheet(id: string, base: string) {
  const { createRequire } = await import("node:module");
  const require = createRequire(join(base, "noop.js"));
  const path = id.startsWith(".") ? join(base, id) : require.resolve(id === "tailwindcss" ? "tailwindcss/index.css" : id);
  return { path, base: dirname(path), content: readFileSync(path, "utf8") };
}

export async function kaesoUtilities(candidates: string[]): Promise<string[]> {
  const { __unstable__loadDesignSystem } = await import("tailwindcss");
  const design = await __unstable__loadDesignSystem(readFileSync(styles, "utf8"), { base: dirname(styles), loadStylesheet });
  const css = design.candidatesToCss(candidates);
  return candidates.filter((_, i) => css[i]);
}
