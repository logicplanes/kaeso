import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { DEFAULT_LOCALE, LANGUAGE_NAMES, LOCALES, localeOfPath, localePrefix, type Locale } from "../../src/i18n/locales.ts";
import { MESSAGES } from "../../src/i18n/messages/index.ts";
import { buildChangelog, changelogClash, withChangelog } from "./changelog.ts";
import { DIAGRAM_COMMAND, DIAGRAM_THEMES, diagramFile, diagramFiles, type DiagramJob } from "./diagram.ts";
import { buildRedirects, noRedirects, redirectsFileOf, type Redirect } from "./redirects.ts";
import { buildSite, type Site, type SitePage } from "./site.ts";

export interface Docs {
  contentDir: string;
  diagramDir: string;
  sites: Record<Locale, Site>;
  redirects: Record<Locale, Redirect[]>;
  diagrams: DiagramJob[];
  problems: string[];
}

export const diagramDirOf = (contentDir: string) => join(dirname(resolve(contentDir)), "diagrams");

export function staleDiagrams(diagramDir: string, jobs: DiagramJob[]): string[] {
  const used = new Set(jobs.flatMap((j) => DIAGRAM_THEMES.map((t) => diagramFile(j.key, t))));
  return diagramFiles(diagramDir).filter((f) => !used.has(f));
}

const posix = (p: string) => p.split(sep).join("/");

function strayEntries(contentDir: string, repoRoot: string): string[] {
  return readdirSync(contentDir)
    .filter((name) => !name.startsWith(".") && !(LOCALES as readonly string[]).includes(name))
    .map((name) => `${posix(relative(repoRoot, join(contentDir, name)))}: put every page in a language folder (${LOCALES.join(", ")})`);
}

function translationProblems(sites: Record<Locale, Site>, repoRoot: string): string[] {
  const source = sites[DEFAULT_LOCALE];
  const english = new Map(source.pages.map((p) => [p.local, p]));
  const fileIn = (site: Site, local: string) => posix(relative(repoRoot, join(site.contentDir, local)));
  const problems: string[] = [];
  for (const locale of LOCALES.filter((l) => l !== DEFAULT_LOCALE)) {
    const site = sites[locale];
    const translated = new Map(site.pages.map((p) => [p.local, p]));
    for (const [local, page] of english) {
      const other = translated.get(local);
      if (!other) problems.push(`${fileIn(site, local)}: missing; translate ${page.rel}`);
      else if (other.meta.order !== page.meta.order)
        problems.push(`${other.rel}: its order is ${orderOf(other)}, but ${page.rel} has ${orderOf(page)}; a translation keeps the English order`);
    }
    for (const [local, page] of translated) {
      if (!english.has(local)) problems.push(`${page.rel}: has no English page; add ${fileIn(source, local)} or remove it`);
    }
  }
  return problems;
}

function reservedAddresses(site: Site): string[] {
  return site.pages.flatMap((page) => {
    const locale = localeOfPath(page.path);
    return locale === DEFAULT_LOCALE ? [] : [`${page.rel}: its address ${page.path} is where the ${LANGUAGE_NAMES[locale]} pages live; rename it`];
  });
}

const orderOf = (page: SitePage) => (page.meta.order === undefined ? "not set" : String(page.meta.order));

export function kaesoVersion(docsRoot: string): string {
  return (JSON.parse(readFileSync(join(docsRoot, "..", "..", "package.json"), "utf8")) as { version: string }).version;
}

const siteOptions = (locale: Locale, version: string, diagrams: string) => {
  const messages = MESSAGES[locale];
  return { prefix: localePrefix(locale), labels: { home: messages["nav.introduction"], overview: messages["nav.overview"] }, version, diagrams };
};

export function buildDocs(contentDir: string, repoRoot: string, version: string, changelogFile?: string, diagramDir = diagramDirOf(contentDir)): Docs {
  const rel = posix(relative(repoRoot, contentDir));
  if (!existsSync(contentDir)) {
    const empty = buildSite(join(contentDir, DEFAULT_LOCALE), repoRoot, siteOptions(DEFAULT_LOCALE, version, diagramDir));
    const sites = Object.fromEntries(LOCALES.map((l) => [l, empty])) as Record<Locale, Site>;
    return { contentDir, diagramDir, sites, redirects: noRedirects(), diagrams: [], problems: [`${rel} does not exist`] };
  }
  const sites = Object.fromEntries(
    LOCALES.map((locale) => [locale, buildSite(join(contentDir, locale), repoRoot, siteOptions(locale, version, diagramDir))]),
  ) as Record<Locale, Site>;
  const diagrams = LOCALES.flatMap((l) => sites[l].diagrams);
  const stale = staleDiagrams(diagramDir, diagrams).map(
    (f) => `${posix(relative(repoRoot, join(diagramDir, f)))}: no page draws this diagram any more; run \`${DIAGRAM_COMMAND}\` to remove it`,
  );
  const redirectsFile = redirectsFileOf(contentDir);
  const { byLocale: redirects, problems: redirectProblems } = buildRedirects(redirectsFile, posix(relative(repoRoot, redirectsFile)), sites);
  const problems = [
    ...strayEntries(contentDir, repoRoot),
    ...LOCALES.flatMap((l) => sites[l].problems),
    ...stale,
    ...reservedAddresses(sites[DEFAULT_LOCALE]),
    ...translationProblems(sites, repoRoot),
    ...redirectProblems,
  ];
  if (!changelogFile) return { contentDir, diagramDir, sites, redirects, diagrams, problems };
  const changelog = buildChangelog(changelogFile, repoRoot);
  const clashes = LOCALES.flatMap((l) => changelogClash(sites[l], localePrefix(l)));
  const logged = Object.fromEntries(LOCALES.map((l) => [l, withChangelog(sites[l], changelog, localePrefix(l), MESSAGES[l])])) as Record<Locale, Site>;
  return { contentDir, diagramDir, sites: logged, redirects, diagrams, problems: [...problems, ...changelog.problems, ...clashes] };
}
