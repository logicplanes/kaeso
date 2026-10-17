import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { DIAGRAM_THEMES, diagramFile, type DiagramJob } from "./diagram.ts";
import { buildDocs, kaesoVersion, staleDiagrams } from "./docs.ts";
import { interFonts, renderMermaid, type MermaidResult, type MermaidSource } from "./mermaid.ts";

export interface SyncResult {
  drawn: number;
  removed: string[];
  problems: string[];
}

export type Render = (sources: MermaidSource[]) => Promise<MermaidResult[]>;

const posix = (p: string) => p.split(sep).join("/");

export const missingDiagrams = (dir: string, jobs: DiagramJob[]) =>
  jobs.filter((j, i) => jobs.findIndex((k) => k.key === j.key) === i && DIAGRAM_THEMES.some((t) => !existsSync(join(dir, diagramFile(j.key, t)))));

export async function syncDiagrams(dir: string, jobs: DiagramJob[], render: Render, options: { prune?: boolean } = {}): Promise<SyncResult> {
  const missing = missingDiagrams(dir, jobs);
  const results = missing.length ? await render(missing.map(({ key, code }) => ({ key, code }))) : [];
  const problems: string[] = [];
  let drawn = 0;
  for (const result of results) {
    if (result.ok) {
      mkdirSync(dir, { recursive: true });
      for (const theme of DIAGRAM_THEMES) writeFileSync(join(dir, diagramFile(result.key, theme)), `${result.svg[theme]}\n`);
      drawn++;
      continue;
    }
    for (const job of jobs.filter((j) => j.key === result.key)) {
      const line = job.line + (result.line ?? 1);
      problems.push(`${job.file}:${line}: the diagram doesn't parse: ${result.message}`);
    }
  }
  const removed = options.prune === false ? [] : staleDiagrams(dir, jobs);
  for (const file of removed) rmSync(join(dir, file));
  return { drawn, removed, problems };
}

export const browserRender =
  (docsRoot: string): Render =>
  (sources) =>
    renderMermaid(sources, { fonts: interFonts(docsRoot) });

export async function drawDiagrams(root: string, contentDir: string, render: Render, version = kaesoVersion(root)): Promise<{ ok: boolean; lines: string[] }> {
  const docs = buildDocs(join(root, contentDir), root, version);
  const result = await syncDiagrams(docs.diagramDir, docs.diagrams, render);
  const where = posix(relative(root, docs.diagramDir));
  if (result.problems.length) return { ok: false, lines: [`diagrams: ${result.problems.length} to fix`, ...result.problems.map((p) => `  ${p}`)] };
  const unique = new Set(docs.diagrams.map((d) => d.key)).size;
  return { ok: true, lines: [`diagrams: ${unique} in ${where}/, ${result.drawn} drawn, ${result.removed.length} removed`] };
}

if (import.meta.main) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
  const result = await drawDiagrams(root, process.env.KAESO_DOCS_CONTENT ?? "content", browserRender(root));
  for (const line of result.lines) (result.ok ? console.log : console.error)(line);
  process.exitCode = result.ok ? 0 : 1;
}
