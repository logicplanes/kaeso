import type { AstroComponentFactory } from "astro/runtime/server/index.js";
import type { DocsLang } from "~/lib/locale";

export interface GuideVersion {
  lang: DocsLang;
  title: string;
  description: string;
  headings: { depth: number; slug: string; text: string }[];
  Content: AstroComponentFactory;
}

export interface GuideModule {
  frontmatter: { title: string; description: string };
  getHeadings: () => { depth: number; slug: string; text: string }[];
  Content: AstroComponentFactory;
}

export function guideVersions(modules: Record<DocsLang, GuideModule>): GuideVersion[] {
  return (Object.entries(modules) as [DocsLang, GuideModule][]).map(([lang, m]) => ({
    lang,
    title: m.frontmatter.title,
    description: m.frontmatter.description,
    headings: m.getHeadings(),
    Content: m.Content,
  }));
}
