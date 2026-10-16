import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { LOCALES } from "../../src/i18n/locales.ts";
import { buildDocs } from "./docs.ts";

export interface CheckResult {
  ok: boolean;
  lines: string[];
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

export function checkContent(root: string, dir: string): CheckResult {
  const docs = buildDocs(join(root, dir), root);
  if (docs.problems.length) return { ok: false, lines: [`content: ${docs.problems.length} to fix in ${dir}/`, ...docs.problems.map((p) => `  ${p}`)] };
  const pages = docs.sites.en.metas.length;
  return { ok: true, lines: [`content: ${plural(pages, "page", "pages")} in ${plural(LOCALES.length, "language", "languages")}, every link checked`] };
}

if (import.meta.main) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
  const result = checkContent(root, process.env.KAESO_DOCS_CONTENT ?? "content");
  for (const line of result.lines) (result.ok ? console.log : console.error)(line);
  process.exitCode = result.ok ? 0 : 1;
}
