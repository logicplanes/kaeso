import { notFound } from "@tanstack/react-router";
import type { PageContent, PageMeta } from "~/markdown/ast";
import type { LocaleSite } from "~/site/data";

export interface LoadedPage {
  meta: PageMeta;
  content: PageContent;
  editBase: string;
}

export function pagePath(prefix: string, splat: string | undefined): string {
  const rest = (splat ?? "").replace(/^\/+|\/+$/g, "");
  return rest ? `${prefix}/${rest}` : prefix || "/";
}

export async function loadPage(site: LocaleSite, path: string): Promise<LoadedPage> {
  const meta = site.pages[path];
  const chunk = site.load[path];
  if (!meta || !chunk) throw notFound();
  return { meta, content: (await chunk()).default, editBase: site.editBase };
}
