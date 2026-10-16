import { demos } from "./demos";
import { localizedPath, type DocsLang } from "./languages";

export interface NavItem {
  title: string;
  href: string;
  icon?: string;
}
export interface NavSection {
  title: string;
  items: NavItem[];
}

export const overview: NavItem[] = [
  { icon: "home", title: "Overview", href: "/" },
  { icon: "download", title: "Installation", href: "/installation" },
  { icon: "themeLight", title: "Theming", href: "/theming" },
  { icon: "serviceAccount", title: "For agents", href: "/for-agents" },
  { icon: "time", title: "Changelog", href: "/changelog" },
];

const pages = Object.values(demos);

export const components: NavItem[] = pages
  .filter((p) => p.group !== "Blocks")
  .sort((a, b) => a.title.localeCompare(b.title))
  .map((p) => ({ title: p.title, href: `/components/${p.slug}` }));

export function sections(lang: DocsLang): NavSection[] {
  const local = (items: NavItem[]) => items.map((it) => ({ ...it, href: localizedPath(lang, it.href) }));
  return [
    { title: "Overview", items: local(overview) },
    { title: "Components", items: local(components) },
  ];
}

export const GITHUB = "https://github.com/logicplanes/kaeso";
