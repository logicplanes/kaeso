import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { LOCALES, type Locale } from "../src/i18n/locales.ts";

export const FRAME_CEILING_KB = 224;
export const PAGE_CEILING_KB = 24;

interface ManifestEntry {
  file: string;
  isEntry?: boolean;
  imports?: string[];
  dynamicImports?: string[];
  css?: string[];
}

type Manifest = Record<string, ManifestEntry>;

export interface Budget {
  name: string;
  kb: number;
  ceiling: number;
}

export interface BudgetReport {
  frames: Budget[];
  pages: Budget[];
  searchKb: number;
  failed: boolean;
  lines: string[];
}

const kb = (bytes: number) => Math.round((bytes / 1024) * 10) / 10;

export function measure(dist: string): BudgetReport {
  const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8")) as Manifest;
  const gz = (file: string) => gzipSync(readFileSync(join(dist, file))).length;
  const closure = (key: string, into = new Set<string>()): Set<string> => {
    const entry = manifest[key];
    if (!entry || into.has(key)) return into;
    into.add(key);
    for (const dep of entry.imports ?? []) closure(dep, into);
    return into;
  };
  const size = (keys: Set<string>) =>
    [...new Set([...keys].flatMap((k) => [manifest[k]?.file ?? "", ...(manifest[k]?.css ?? [])]))]
      .filter((f) => f.endsWith(".js") || f.endsWith(".css"))
      .reduce((sum, f) => sum + gz(f), 0);
  const keyOf = (test: (key: string) => boolean) => Object.keys(manifest).find(test);
  const entryKey = keyOf((k) => manifest[k]?.isEntry === true);
  if (!entryKey) throw new Error("budget: the build has no entry");
  const entry = closure(entryKey);

  const frame = (locale: Locale): Budget => {
    const keys = new Set(entry);
    const site = keyOf((k) => k === `kaeso-docs:site/${locale}`);
    if (site) closure(site, keys);
    const ui = keyOf((k) => new RegExp(`(^|/)packages/ui/dist/chunks/${locale}-[^/]+\\.js$`).test(k));
    if (ui) closure(ui, keys);
    return { name: `the frame in ${locale}`, kb: kb(size(keys)), ceiling: FRAME_CEILING_KB };
  };
  const frames = LOCALES.map(frame);
  const pages = Object.keys(manifest)
    .filter((k) => k.startsWith("kaeso-docs:page/"))
    .map((k) => ({ name: k.slice("kaeso-docs:page/".length), kb: kb(gz(manifest[k]?.file ?? "")), ceiling: PAGE_CEILING_KB }))
    .sort((a, b) => b.kb - a.kb);
  const searchKey = keyOf((k) => k === "src/shell/search.tsx");
  const searchKb = searchKey ? kb(size(new Set([...closure(searchKey)].filter((k) => !entry.has(k))))) : 0;

  const over = [...frames, ...pages].filter((b) => b.kb > b.ceiling);
  const largest = pages[0];
  const lines = [
    ...frames.map((b) => `budget: ${b.name}: ${b.kb} kB gzipped; ceiling ${b.ceiling} kB ${b.kb > b.ceiling ? "EXCEEDED" : "ok"}`),
    ...(largest ? [`budget: the largest page is ${largest.name}: ${largest.kb} kB gzipped; ceiling ${PAGE_CEILING_KB} kB a page`] : []),
    ...pages.filter((p) => p.kb > p.ceiling).map((p) => `budget: the page ${p.name} is ${p.kb} kB gzipped, over ${PAGE_CEILING_KB} kB: split it into pages`),
    `budget: the search, loaded after the first paint: ${searchKb} kB gzipped without its index`,
  ];
  return { frames, pages, searchKb, failed: over.length > 0, lines };
}

if (import.meta.main) {
  const dist = join(dirname(fileURLToPath(import.meta.url)), "../dist");
  const report = measure(dist);
  for (const line of report.lines) console.log(line);
  process.exitCode = report.failed ? 1 : 0;
}
